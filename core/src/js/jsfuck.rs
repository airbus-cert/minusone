use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::{Raw, Regex};
use crate::js::Value::Str;
use crate::js::regex::ParseRegex;
use crate::js::utils::{get_positional_arguments, method_name};
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use log::trace;

/// Resolves the JSFuck ["level 9"](https://stackoverflow.com/a/63713987) universal builder.
///
/// When a character cannot be assembled from primitive value coercions, JSFuck
/// (and JSFuck-style encoders such as <https://stackoverflow.com/a/63713987>)
/// falls back to the `Function` constructor with a body that simply returns a
/// string literal, e.g.
///
/// ```text
/// []["flat"]["constructor"]("return 'A'")()   // => "A"
/// Function("return '😀'")()              // => "😀"
/// ```
///
/// This rule detects that exact shape — a zero-argument call whose callee is a
/// call of the `Function` constructor (`X["constructor"](...)` or the global
/// `Function(...)`) whose argument is the literal `return '<string literal>'` —
/// decodes the returned string literal (interpreting `\uXXXX`, `\u{…}`, `\xXX`
/// and the standard escapes, including surrogate pairs) and reduces the whole
/// expression to that string. Only a returned *string literal* is evaluated, so
/// no arbitrary code is executed.
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::string::ParseString;
/// use minusone::js::jsfuck::JsFuckLevelNine;
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree(r#"Function("return 'AB'")()"#).unwrap();
/// tree.apply_mut(&mut (ParseString::default(), JsFuckLevelNine::default())).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
/// assert_eq!(linter.output, "'AB'");
/// ```
#[derive(Default)]
pub struct JsFuckLevelNine;

impl JsFuckLevelNine {
    /// decodes a `Function` body of the form `return '<string literal>'`.
    fn decode_return_string(body: &str) -> Option<String> {
        let rest = body.trim().strip_prefix("return")?.trim_start();
        let mut chars = rest.chars();
        let quote = chars.next()?;
        if quote != '\'' && quote != '"' {
            return None;
        }

        let inner = rest
            .strip_prefix(quote)
            .and_then(|s| s.strip_suffix(quote))?;
        decode_js_string_literal(inner)
    }
}

/// emulates `Function("try{ <stmt> }catch(f){return f}")()`
fn decode_thrown_error(body: &str) -> Option<String> {
    let stmt = body
        .trim()
        .strip_prefix("try{")?
        .rsplit_once("}catch(")?
        .0
        .trim();
    let message = match stmt {
        "String().normalize(false)" => {
            "RangeError: The normalization form should be one of NFC, NFD, NFKC, NFKD."
        }
        "Function([]+[[]].concat([[]]))()" => "SyntaxError: Unexpected token ','",
        _ => return None,
    };
    Some(message.to_string())
}

fn decode_js_string_literal(inner: &str) -> Option<String> {
    let chars: Vec<char> = inner.chars().collect();
    let mut units: Vec<u16> = Vec::new();
    let mut i = 0;

    let push_char = |units: &mut Vec<u16>, c: char| {
        let mut buf = [0u16; 2];
        for u in c.encode_utf16(&mut buf) {
            units.push(*u);
        }
    };

    while i < chars.len() {
        let c = chars[i];
        if c != '\\' {
            if c == '\'' || c == '"' {
                return None;
            }
            push_char(&mut units, c);
            i += 1;
            continue;
        }

        i += 1;
        let esc = *chars.get(i)?;
        match esc {
            'u' => {
                i += 1;
                if chars.get(i) == Some(&'{') {
                    i += 1;
                    let start = i;
                    while i < chars.len() && chars[i] != '}' {
                        i += 1;
                    }
                    if i >= chars.len() {
                        return None;
                    }
                    let hex: String = chars[start..i].iter().collect();
                    i += 1; // consume '}'
                    let cp = u32::from_str_radix(&hex, 16).ok()?;
                    push_char(&mut units, char::from_u32(cp)?);
                } else {
                    if i + 4 > chars.len() {
                        return None;
                    }
                    let hex: String = chars[i..i + 4].iter().collect();
                    i += 4;
                    units.push(u16::from_str_radix(&hex, 16).ok()?);
                }
            }
            'x' => {
                i += 1;
                if i + 2 > chars.len() {
                    return None;
                }
                let hex: String = chars[i..i + 2].iter().collect();
                i += 2;
                units.push(u16::from_str_radix(&hex, 16).ok()?);
            }
            'n' => {
                units.push(0x0A);
                i += 1;
            }
            't' => {
                units.push(0x09);
                i += 1;
            }
            'r' => {
                units.push(0x0D);
                i += 1;
            }
            'b' => {
                units.push(0x08);
                i += 1;
            }
            'f' => {
                units.push(0x0C);
                i += 1;
            }
            'v' => {
                units.push(0x0B);
                i += 1;
            }
            // legacy octal escape `\ooo`
            '0'..='7' => {
                let max_len = if esc <= '3' { 3 } else { 2 };
                let start = i;
                i += 1;
                while i - start < max_len && i < chars.len() && ('0'..='7').contains(&chars[i]) {
                    i += 1;
                }
                let octal: String = chars[start..i].iter().collect();
                units.push(u16::from_str_radix(&octal, 8).ok()?);
            }
            // \\, \', \" and any other escaped character: take it literally
            other => {
                push_char(&mut units, other);
                i += 1;
            }
        }
    }

    String::from_utf16(&units).ok()
}

impl<'a> RuleMut<'a> for JsFuckLevelNine {
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

        if !get_positional_arguments(view.named_child("arguments")).is_empty() {
            return Ok(());
        }

        let Some(inner) = view.named_child("function").or_else(|| view.child(0)) else {
            return Ok(());
        };
        if inner.kind() != "call_expression" {
            return Ok(());
        }

        let Some(inner_callee) = inner.named_child("function").or_else(|| inner.child(0)) else {
            return Ok(());
        };
        let is_function_constructor = method_name(&inner_callee).as_deref() == Some("constructor")
            || inner_callee
                .text()
                .map(|t| t == "Function")
                .unwrap_or(false);
        if !is_function_constructor {
            return Ok(());
        }

        let inner_args = get_positional_arguments(inner.named_child("arguments"));
        if inner_args.len() != 1 {
            return Ok(());
        }
        let Some(Raw(Str(body))) = inner_args[0].data() else {
            return Ok(());
        };

        if let Some(decoded) = JsFuckLevelNine::decode_return_string(body) {
            trace!(
                "JsFuckLevelNine: reducing Function(\"{}\")() to {:?}",
                body, decoded
            );
            node.reduce(Raw(Str(decoded)));
        } else if let Some(error) = decode_thrown_error(body) {
            trace!(
                "JsFuckLevelNine: reducing Function(\"{}\")() (thrown) to {:?}",
                body, error
            );
            node.reduce(Raw(Str(error)));
        } else if let Some((pattern, flags)) = body
            .trim()
            .strip_prefix("return")
            .and_then(|r| ParseRegex::parse_regex_literal(r.trim_start()))
        {
            trace!(
                "JsFuckLevelNine: reducing Function(\"{}\")() to /{}/{}",
                body, pattern, flags
            );
            node.reduce(Regex { pattern, flags });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_bmp_escape() {
        assert_eq!(
            JsFuckLevelNine::decode_return_string(r"return 'A'").as_deref(),
            Some("A")
        );
    }

    #[test]
    fn decode_surrogate_pair() {
        assert_eq!(
            JsFuckLevelNine::decode_return_string(r"return '😀'").as_deref(),
            Some("😀")
        );
    }

    #[test]
    fn decode_code_point_and_hex() {
        assert_eq!(
            JsFuckLevelNine::decode_return_string(r"return '\u{1F600}'").as_deref(),
            Some("😀")
        );
        assert_eq!(
            JsFuckLevelNine::decode_return_string(r#"return "\x41\x42""#).as_deref(),
            Some("AB")
        );
    }

    #[test]
    fn rejects_non_string_return() {
        assert_eq!(JsFuckLevelNine::decode_return_string("return 1+1"), None);
        assert_eq!(JsFuckLevelNine::decode_return_string("alert(1)"), None);
    }
}
