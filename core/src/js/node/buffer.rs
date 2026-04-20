use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::{Array, Buffer, Raw};
use crate::js::Value::{Bool, Num, Str};
use crate::js::utils::{get_positional_arguments, method_name};
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use base64::{Engine, alphabet};
use log::trace;

/// Infers `Buffer.from(...)` static calls.
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::integer::ParseInt;
/// use minusone::js::array::ParseArray;
/// use minusone::js::node::buffer::BufferFrom;
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = Buffer.from([65, 66, 67]);").unwrap();
/// tree.apply_mut(&mut (ParseInt::default(), ParseArray::default(), BufferFrom::default())).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
/// assert_eq!(linter.output, "var x = Buffer.from('414243', 'hex');");
/// ```
#[derive(Default)]
pub struct BufferFrom;

impl<'a> RuleMut<'a> for BufferFrom {
    type Language = JavaScript;

    fn enter(
        &mut self,
        _node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        Ok(())
    }

    fn leave(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        let view = node.view();
        if view.kind() != "call_expression" {
            return Ok(());
        }

        let Some(callee) = view.named_child("function").or_else(|| view.child(0)) else {
            return Ok(());
        };

        let Some(method) = method_name(&callee) else {
            return Ok(());
        };
        if method != "from" {
            return Ok(());
        }

        let Some(object) = callee.child(0).or_else(|| callee.named_child("object")) else {
            return Ok(());
        };
        let Ok(object_name) = object.text() else {
            return Ok(());
        };
        if object_name != "Buffer" {
            return Ok(());
        }

        let args = get_positional_arguments(view.named_child("arguments"));
        let Some(first_arg) = args.first() else {
            return Ok(());
        };

        let bytes = match first_arg.data() {
            Some(Raw(Str(input))) => {
                let encoding = args
                    .get(1)
                    .and_then(|arg| arg.data())
                    .and_then(|value| match value {
                        Raw(Str(s)) => Some(normalize_encoding(s)),
                        _ => None,
                    })
                    .unwrap_or_else(|| "utf8".to_string());
                decode_string_for_encoding(input, &encoding)
            }
            Some(Array(array)) => Some(array_node_to_bytes(array)),
            _ => None,
        };

        if let Some(bytes) = bytes {
            trace!(
                "BufferFrom: reducing Buffer.from(...) to Buffer of {} bytes",
                bytes.len()
            );
            node.reduce(Buffer(bytes));
        }

        Ok(())
    }
}

/// Infers toString calls on Buffer objects
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::integer::ParseInt;
/// use minusone::js::array::ParseArray;
/// use minusone::js::node::buffer::{BufferFrom, BufferToString};
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = Buffer.from([65, 66, 67]).toString();").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(), ParseArray::default(), BufferFrom::default(), BufferToString::default()
/// )).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
/// assert_eq!(linter.output, "var x = 'ABC';");
/// ```
#[derive(Default)]
pub struct BufferToString;

impl<'a> RuleMut<'a> for BufferToString {
    type Language = JavaScript;

    fn enter(
        &mut self,
        _node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        Ok(())
    }

    fn leave(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        let view = node.view();
        if view.kind() != "call_expression" {
            return Ok(());
        }

        let Some(callee) = view.named_child("function").or_else(|| view.child(0)) else {
            return Ok(());
        };
        let Some(method) = method_name(&callee) else {
            return Ok(());
        };
        if method != "toString" {
            return Ok(());
        }

        let Some(object) = callee.child(0).or_else(|| callee.named_child("object")) else {
            return Ok(());
        };
        let Some(Buffer(bytes)) = object.data() else {
            return Ok(());
        };

        let args = get_positional_arguments(view.named_child("arguments"));
        let encoding = match args.first().and_then(|arg| arg.data()) {
            None | Some(JavaScript::Undefined) => "utf8".to_string(),
            Some(Raw(Str(s))) => normalize_encoding(s),
            _ => return Ok(()),
        };

        let len = bytes.len();
        let start = args
            .get(1)
            .and_then(|arg| arg.data())
            .and_then(js_to_positive_index)
            .unwrap_or(0)
            .min(len);
        let end = args
            .get(2)
            .and_then(|arg| arg.data())
            .and_then(js_to_positive_index)
            .unwrap_or(len)
            .min(len);
        let slice = if end < start {
            &bytes[0..0]
        } else {
            &bytes[start..end]
        };

        let Some(decoded) = encode_buffer_slice(slice, &encoding) else {
            return Ok(());
        };

        trace!(
            "BufferToString: reducing Buffer.toString('{}', {}, {}) to '{}'",
            encoding,
            start,
            end,
            decoded
        );
        node.reduce(Raw(Str(decoded)));
        Ok(())
    }
}

fn normalize_encoding(input: &str) -> String {
    input.trim().to_ascii_lowercase().replace(['-', '_'], "")
}

fn decode_string_for_encoding(input: &str, encoding: &str) -> Option<Vec<u8>> {
    match encoding {
        "utf8" => Some(input.as_bytes().to_vec()),
        "utf16le" | "ucs2" => Some(
            input
                .encode_utf16()
                .flat_map(|unit| [(unit & 0xff) as u8, (unit >> 8) as u8])
                .collect(),
        ),
        "ascii" | "latin1" | "binary" => {
            Some(input.chars().map(|ch| (ch as u32 & 0xff) as u8).collect())
        }
        "base64" => decode_base64(input, &alphabet::STANDARD),
        "base64url" => decode_base64(input, &alphabet::URL_SAFE),
        "hex" => Some(decode_hex_node_style(input)),
        _ => None,
    }
}

fn decode_base64(input: &str, alphabet: &alphabet::Alphabet) -> Option<Vec<u8>> {
    let config = GeneralPurposeConfig::new()
        .with_decode_padding_mode(DecodePaddingMode::Indifferent)
        .with_decode_allow_trailing_bits(true);
    GeneralPurpose::new(alphabet, config).decode(input).ok()
}

fn encode_buffer_slice(bytes: &[u8], encoding: &str) -> Option<String> {
    match encoding {
        "utf8" => Some(String::from_utf8_lossy(bytes).to_string()),
        "hex" => Some(bytes.iter().map(|b| format!("{:02x}", b)).collect()),
        "ascii" => Some(bytes.iter().map(|b| (b & 0x7f) as char).collect()),
        "latin1" | "binary" => Some(bytes.iter().map(|b| *b as char).collect()),
        "base64" => Some(GeneralPurpose::new(&alphabet::STANDARD, GeneralPurposeConfig::new()).encode(bytes)),
        "base64url" => Some(
            GeneralPurpose::new(
                &alphabet::URL_SAFE,
                GeneralPurposeConfig::new().with_encode_padding(false),
            )
                .encode(bytes),
        ),
        "utf16le" | "ucs2" => Some(String::from_utf16le_lossy(bytes)),
        _ => None,
    }
}

fn js_to_positive_index(value: &JavaScript) -> Option<usize> {
    let number = match

    value.as_js_num() {
        Raw(Num(n)) => n,
        _ => f64::NAN,
    };

    if !number.is_finite() {
        return None;
    }
    if number <= 0.0 {
        return None
    }

    Some(number as usize)
}

fn decode_hex_node_style(input: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(input.len() / 2);
    let mut chars = input.chars();

    while let (Some(hi), Some(lo)) = (chars.next(), chars.next()) {
        let Some(hi) = hi.to_digit(16) else {
            break;
        };
        let Some(lo) = lo.to_digit(16) else {
            break;
        };
        bytes.push(((hi << 4) | lo) as u8);
    }

    bytes
}

fn array_node_to_bytes(array_node: &Vec<JavaScript>) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(array_node.len());

    for item in array_node {
        bytes.push(array_item_to_u8(&item));
    }

    bytes
}

fn array_item_to_u8(value: &JavaScript) -> u8 {
    let number = match value {
        Raw(Num(n)) => *n,
        Raw(Str(s)) => match Raw(Str(s.clone())).as_js_num() {
            Raw(Num(n)) => n,
            _ => f64::NAN,
        }
        Raw(Bool(b)) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        JavaScript::Null => 0.0,
        JavaScript::Undefined => f64::NAN,
        JavaScript::NaN => f64::NAN,
        JavaScript::Bytes(bytes) | Buffer(bytes) => {
            let as_string = String::from_utf8_lossy(bytes).to_string();
            as_string.trim().parse::<f64>().ok().unwrap_or(f64::NAN)
        }
        Array(_)
        | JavaScript::Regex { .. }
        | JavaScript::Function { .. }
        | JavaScript::Object { .. } => f64::NAN,
        Raw(_) => f64::NAN,
    };

    to_uint8(number)
}

fn to_uint8(n: f64) -> u8 {
    if !n.is_finite() || n == 0.0 {
        return 0;
    }

    // Node docs: values are truncated then masked with 0xFF.
    (n.trunc() as i64).rem_euclid(256) as u8
}

#[cfg(test)]
mod tests {
    use crate::js::array::ParseArray;
    use crate::js::build_javascript_tree;
    use crate::js::forward::Forward;
    use crate::js::integer::{ParseInt, PosNeg};
    use crate::js::linter::Linter;
    use crate::js::node::buffer::{BufferFrom, BufferToString};
    use crate::js::string::ParseString;
    use crate::js::var::Var;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseString::default(),
            ParseArray::default(),
            BufferFrom::default(),
            PosNeg::default(),
            Forward::default(),
            Var::default(),
            BufferToString::default(),
        ))
            .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_buffer_from_array() {
        assert_eq!(
            deobfuscate("const buf4 = Buffer.from([1, 2, 3]);"),
            "const buf4 = Buffer.from('010203', 'hex');"
        );
    }

    #[test]
    fn test_buffer_from_array_truncates_with_and_255() {
        assert_eq!(
            deobfuscate("const buf5 = Buffer.from([257, 257.5, -255, '1']);"),
            "const buf5 = Buffer.from('01010101', 'hex');"
        );
    }

    #[test]
    fn test_buffer_from_utf8() {
        assert_eq!(
            deobfuscate("const buf6 = Buffer.from('tést');"),
            "const buf6 = Buffer.from('74c3a97374', 'hex');"
        );
    }

    #[test]
    fn test_buffer_from_latin1_alias_binary() {
        assert_eq!(
            deobfuscate("const buf7 = Buffer.from('tést', 'latin1');"),
            "const buf7 = Buffer.from('74e97374', 'hex');"
        );
        assert_eq!(
            deobfuscate("const buf8 = Buffer.from('tést', 'binary');"),
            "const buf8 = Buffer.from('74e97374', 'hex');"
        );
    }

    #[test]
    fn test_buffer_from_encodings() {
        assert_eq!(
            deobfuscate("const b = Buffer.from('QQ==', 'base64');"),
            "const b = Buffer.from('41', 'hex');"
        );
        assert_eq!(
            deobfuscate("const b = Buffer.from('QQ', 'base64url');"),
            "const b = Buffer.from('41', 'hex');"
        );
        assert_eq!(
            deobfuscate("const b = Buffer.from('A', 'hex');"),
            "const b = Buffer.from('', 'hex');"
        );
        assert_eq!(
            deobfuscate("const b = Buffer.from('4142ZZ', 'hex');"),
            "const b = Buffer.from('4142', 'hex');"
        );
        assert_eq!(
            deobfuscate("const b = Buffer.from('A', 'ucs2');"),
            "const b = Buffer.from('4100', 'hex');"
        );
        assert_eq!(
            deobfuscate("const b = Buffer.from('A', 'utf16le');"),
            "const b = Buffer.from('4100', 'hex');"
        );
    }

    #[test]
    fn test_buffer_to_string_utf8_and_range() {
        assert_eq!(
            deobfuscate(
                "const buf1 = Buffer.from('abcdefghijklmnopqrstuvwxyz'); console.log(buf1.toString('utf8'));"
            ),
            "const buf1 = Buffer.from('6162636465666768696a6b6c6d6e6f707172737475767778797a', 'hex'); console.log('abcdefghijklmnopqrstuvwxyz');"
        );

        assert_eq!(
            deobfuscate(
                "const buf1 = Buffer.from('abcdefghijklmnopqrstuvwxyz'); console.log(buf1.toString('utf8', 0, 5));"
            ),
            "const buf1 = Buffer.from('6162636465666768696a6b6c6d6e6f707172737475767778797a', 'hex'); console.log('abcde');"
        );
    }

    #[test]
    fn test_buffer_to_string_hex_and_undefined_encoding() {
        assert_eq!(
            deobfuscate("const buf2 = Buffer.from('tést'); console.log(buf2.toString('hex'));"),
            "const buf2 = Buffer.from('74c3a97374', 'hex'); console.log('74c3a97374');"
        );

        assert_eq!(
            deobfuscate("const buf2 = Buffer.from('tést'); console.log(buf2.toString('utf8', 0, 3));"),
            "const buf2 = Buffer.from('74c3a97374', 'hex'); console.log('té');"
        );

        assert_eq!(
            deobfuscate("const buf2 = Buffer.from('tést'); console.log(buf2.toString(undefined, 0, 3));"),
            "const buf2 = Buffer.from('74c3a97374', 'hex'); console.log('té');"
        );
    }
}
