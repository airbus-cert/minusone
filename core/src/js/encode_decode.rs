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
    /*("encodeURI", encode_builtin_encode_uri),
    ("decodeURI", encode_builtin_decode_uri),
    ("encodeURIComponent", encode_builtin_encode_uri_component),
    ("decodeURIComponent", encode_builtin_decode_uri_component),*/
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
}
