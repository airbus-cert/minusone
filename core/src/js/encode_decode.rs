use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::*;
use crate::js::Value::*;
use crate::js::utils::*;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use log::trace;

/// Centralized dispatcher for static encode/decode builtins
///
/// This includes:
/// - `escape(s)`
/// - `unescape(s)`
/// - `encodeURI(s)`
/// - `decodeURI(s)`
/// - `encodeURIComponent(s)`
/// - `decodeURIComponent(s)`
type EncodeDecode = fn(&[JavaScript]) -> Option<JavaScript>;
const ENCODE_BUILTINS: &[(&str, EncodeDecode)] = &[
    ("escape", encode_builtin_escape),
    ("unescape", encode_builtin_unescape),
    ("encodeURI", encode_builtin_encode_uri),
    ("decodeURI", encode_builtin_decode_uri),
    ("encodeURIComponent", encode_builtin_encode_uri_component),
    ("decodeURIComponent", encode_builtin_decode_uri_component),
];

#[derive(Default)]
pub struct EncodeDecodeBuiltins;

impl<'a> RuleMut<'a> for EncodeDecodeBuiltins {
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

        let Ok(method) = callee.text() else {
            return Ok(());
        };

        let args = view.named_child("arguments");
        let positional_args = get_positional_arguments(args);
        let mut arg_values = Vec::with_capacity(positional_args.len());
        for arg in positional_args {
            let Some(value) = arg.data().cloned() else {
                return Ok(());
            };
            arg_values.push(value);
        }

        let Some(result) = dispatch_encode_builtin(method, &arg_values) else {
            return Ok(());
        };

        let result = match result {
            Raw(Num(n)) if n.is_nan() => NaN,
            any => any,
        };

        trace!("EncodeBuiltins: reducing {}(...) to {}", method, result);
        node.reduce(result);
        Ok(())
    }
}

fn dispatch_encode_builtin(method: &str, args: &[JavaScript]) -> Option<JavaScript> {
    ENCODE_BUILTINS
        .iter()
        .find_map(|(name, handler)| (*name == method).then(|| handler(args)))
        .flatten()
}

// Escape/Unescape
const ESCAPE_ALLOWED_CHARS: &[u8] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789@\\*_+-./";
fn encode_builtin_escape(args: &[JavaScript]) -> Option<JavaScript> {
    if args.len() < 1 {
        return Some(Raw(Str("undefined".to_string())));
    }
    let s = match &args[0] {
        Raw(Str(s)) => s.clone(),
        any => any.to_string(),
    };

    let mut encoded = String::new();
    for (_, char) in s.char_indices() {
        if char as usize > u8::MAX as usize {
            encoded.push_str(&format!("%u{:04X}", char as u32));
        } else {
            if ESCAPE_ALLOWED_CHARS.contains(&(char as u8)) {
                encoded.push(char);
            } else {
                encoded.push_str(&format!("%{:02X}", char as u32));
            }
        }
    }

    Some(Raw(Str(encoded)))
}

fn encode_builtin_unescape(args: &[JavaScript]) -> Option<JavaScript> {
    if args.len() < 1 {
        return Some(Raw(Str("undefined".to_string())));
    }
    let s = match &args[0] {
        Raw(Str(s)) => s.clone(),
        any => any.to_string(),
    };

    let mut decoded = String::new();
    let mut i = 0;
    while i < s.len() {
        if &s[i..i + 1] == "%" {
            if i + 1 < s.len() && &s[i + 1..i + 2] == "u" {
                // %uXXXX
                if i + 6 <= s.len() {
                    if let Ok(code_point) = u32::from_str_radix(&s[i + 2..i + 6], 16) {
                        if let Some(char) = std::char::from_u32(code_point) {
                            decoded.push(char);
                            i += 6;
                            continue;
                        }
                    }
                }
            } else {
                // %XX
                if i + 3 <= s.len() {
                    if let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                        decoded.push(byte as char);
                        i += 3;
                        continue;
                    }
                }
            }
        }
        decoded.push(s.chars().nth(i).unwrap());
        i += 1;
    }

    Some(Raw(Str(decoded)))
}

// URIs
const URI_ALLOWED_CHARS: &[u8] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!#$&'()*+,-./:;=?@_~";
const URI_COMPONENT_ALLOWED_CHARS: &[u8] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!'()*-._~";
const DECODE_URI_RESERVED: &[u8] = b";,/?:@&=+$#"; // URI_ALLOWED_CHARS - URI_COMPONENT_ALLOWED_CHARS

fn encode_builtin_encode_uri(args: &[JavaScript]) -> Option<JavaScript> {
    encode_uri(args, false)
}

fn encode_builtin_encode_uri_component(args: &[JavaScript]) -> Option<JavaScript> {
    encode_uri(args, true)
}

fn encode_uri(args: &[JavaScript], component: bool) -> Option<JavaScript> {
    if args.len() < 1 {
        return Some(Raw(Str("undefined".to_string())));
    }
    let s = match &args[0] {
        Raw(Str(s)) => s.clone(),
        any => any.to_string(),
    };

    let allowed_chars = if component {
        URI_COMPONENT_ALLOWED_CHARS
    } else {
        URI_ALLOWED_CHARS
    };
    let mut encoded = String::new();
    for char in s.chars() {
        // ASCII
        if char.is_ascii() && allowed_chars.contains(&(char as u8)) {
            encoded.push(char);
        } else {
            // UTF-8
            let mut buf = [0u8; 4];
            let utf8_bytes = char.encode_utf8(&mut buf);
            for byte in utf8_bytes.as_bytes() {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    Some(Raw(Str(encoded)))
}

fn utf8_char_width(first_byte: u8) -> usize {
    match first_byte {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => 1, // invalid UTF-8
    }
}

fn encode_builtin_decode_uri(args: &[JavaScript]) -> Option<JavaScript> {
    decode_uri(args, false)
}

fn encode_builtin_decode_uri_component(args: &[JavaScript]) -> Option<JavaScript> {
    decode_uri(args, true)
}

fn decode_uri(args: &[JavaScript], component: bool) -> Option<JavaScript> {
    if args.len() < 1 {
        return Some(Raw(Str("undefined".to_string())));
    }
    let s = match &args[0] {
        Raw(Str(s)) => s.clone(),
        any => any.to_string(),
    };

    let bytes = s.as_bytes();
    let mut decoded = String::new();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'%' && i + 3 <= bytes.len() {
            let mut buf: Vec<u8> = Vec::new();
            let mut j = i;
            while j + 3 <= bytes.len() && bytes[j] == b'%' {
                let hex = match std::str::from_utf8(&bytes[j + 1..j + 3]) {
                    Ok(h) => h,
                    Err(_) => break,
                };
                match u8::from_str_radix(hex, 16) {
                    Ok(byte) => {
                        buf.push(byte);
                        j += 3;
                    }
                    Err(_) => break,
                }
            }
            if !buf.is_empty() {
                let mut k = 0;
                while k < buf.len() {
                    let width = utf8_char_width(buf[k]).min(buf.len() - k);
                    if width == 1 && DECODE_URI_RESERVED.contains(&buf[k]) && !component {
                        decoded.push_str(&s[i + k * 3..i + k * 3 + 3]);
                    } else {
                        match std::str::from_utf8(&buf[k..k + width]) {
                            Ok(cs) => decoded.push_str(cs),
                            Err(_) => decoded.push(char::REPLACEMENT_CHARACTER),
                        }
                    }
                    k += width;
                }
                i = j;
                continue;
            }
        }
        let c = s[i..].chars().next().unwrap();
        decoded.push(c);
        i += c.len_utf8();
    }

    Some(Raw(Str(decoded)))
}

#[cfg(test)]
mod test_encode_decode {
    use crate::js::build_javascript_tree;
    use crate::js::encode_decode::EncodeDecodeBuiltins;
    use crate::js::linter::Linter;
    use crate::js::specials::ParseSpecials;
    use crate::js::string::ParseString;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            ParseSpecials::default(),
            EncodeDecodeBuiltins::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_escape() {
        assert_eq!(deobfuscate("escape('abc123');"), "'abc123';");
        assert_eq!(deobfuscate("escape('äöü');"), "'%E4%F6%FC';");
        assert_eq!(deobfuscate("escape('ć');"), "'%u0107';");
        assert_eq!(deobfuscate("escape('@*_+-./');"), "'@*_+-./';");
        assert_eq!(deobfuscate("escape();"), "'undefined';");
        assert_eq!(deobfuscate("escape(null);"), "'null';");
    }

    #[test]
    fn test_unescape() {
        assert_eq!(deobfuscate("unescape('%E4%F6%FC');"), "'äöü';");
        assert_eq!(deobfuscate("unescape('%u0107');"), "'ć';");
        assert_eq!(deobfuscate("unescape('@*_+-./');"), "'@*_+-./';");
        assert_eq!(deobfuscate("unescape();"), "'undefined';");
        assert_eq!(deobfuscate("unescape(null);"), "'null';");
    }

    #[test]
    fn test_encode_uri() {
        assert_eq!(deobfuscate("encodeURI('abc123');"), "'abc123';");
        assert_eq!(deobfuscate("encodeURI('&');"), "'&';");
        assert_eq!(deobfuscate("encodeURI('ć');"), "'%C4%87';");
        assert_eq!(
            deobfuscate("encodeURI('https://mozilla.org/?x=шеллы');"),
            "'https://mozilla.org/?x=%D1%88%D0%B5%D0%BB%D0%BB%D1%8B';"
        );
        assert_eq!(deobfuscate("encodeURI(';,/?:@&=+$#');"), "';,/?:@&=+$#';");
        assert_eq!(deobfuscate("encodeURI('-_.!~*\\'()');"), "'-_.!~*\\'()';");
        assert_eq!(
            deobfuscate("encodeURI('ABC abc 123');"),
            "'ABC%20abc%20123';"
        );
        assert_eq!(deobfuscate("encodeURI();"), "'undefined';");
        assert_eq!(deobfuscate("encodeURI(null);"), "'null';");
    }

    #[test]
    fn test_decode_uri() {
        assert_eq!(deobfuscate("decodeURI('%C4%87');"), "'ć';");
        assert_eq!(deobfuscate("decodeURI('%26');"), "'%26';");
        assert_eq!(
            deobfuscate("decodeURI('https://mozilla.org/?x=%D1%88%D0%B5%D0%BB%D0%BB%D1%8B');"),
            "'https://mozilla.org/?x=шеллы';"
        );
        assert_eq!(deobfuscate("decodeURI();"), "'undefined';");
        assert_eq!(deobfuscate("decodeURI(null);"), "'null';");
    }

    #[test]
    fn test_encode_uri_component() {
        assert_eq!(deobfuscate("encodeURIComponent('&');"), "'%26';");
        assert_eq!(
            deobfuscate("encodeURIComponent('https://mozilla.org/?x=шеллы');"),
            "'https%3A%2F%2Fmozilla.org%2F%3Fx%3D%D1%88%D0%B5%D0%BB%D0%BB%D1%8B';"
        );
    }

    #[test]
    fn test_decode_uri_component() {
        assert_eq!(deobfuscate("decodeURIComponent('%26');"), "'&';");
        assert_eq!(
            deobfuscate(
                "decodeURIComponent('https%3A%2F%2Fmozilla.org%2F%3Fx%3D%D1%88%D0%B5%D0%BB%D0%BB%D1%8B');"
            ),
            "'https://mozilla.org/?x=шеллы';"
        );
    }
}
