use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::{Array, Raw};
use crate::js::Value::Bool;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use js::Value;
use js::Value::{Num, Str};
use log::{debug, trace, warn};
use js::array::flatten_array;
use js::JavaScript::Undefined;

/// Parses JavaScript string literals into `Raw(Str(_))`.
#[derive(Default)]
pub struct ParseString;

impl<'a> RuleMut<'a> for ParseString {
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
        if view.kind() != "string" {
            return Ok(());
        }

        let value = view.text();
        if let Err(e) = value {
            warn!("ParseString: error getting text for node: {}", e);
            return Ok(());
        }
        let value = unescaped_js_string(&value?);

        trace!("ParseString (L): string literal with value '{}'", value);
        node.reduce(Raw(Str(value)));

        Ok(())
    }
}

fn unescaped_js_string(s: &str) -> String {
    if s.len() < 2 {
        return s.to_string();
    }

    let mut result = String::new();
    let mut chars = s[1..s.len() - 1].chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                match next {
                    'n' => result.push('\n'),
                    't' => result.push('\t'),
                    'r' => result.push('\r'),
                    '\\' => result.push('\\'),
                    '"' => result.push('"'),
                    '\'' => result.push('\''),
                    'u' => {
                        let mut hex = String::new();
                        for _ in 0..4 {
                            if let Some(h) = chars.next() {
                                hex.push(h);
                            } else {
                                warn!("ParseString: incomplete unicode escape sequence");
                                break;
                            }
                        }
                        if let Ok(code_point) = u16::from_str_radix(&hex, 16) {
                            if let Some(ch) = std::char::from_u32(code_point as u32) {
                                result.push(ch);
                            } else {
                                warn!("ParseString: invalid unicode code point: {}", hex);
                            }
                        } else {
                            warn!("ParseString: invalid unicode escape sequence: {}", hex);
                        }
                    }
                    _ => {
                        warn!("ParseString: unrecognized escape sequence: \\{}", next);
                        result.push(next);
                    }
                }
            } else {
                warn!("ParseString: trailing backslash in string literal");
                result.push('\\');
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Infers charAt calls on string literals and reduces them to single-character string literals using arrays indexes
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::string::{ParseString, CharAt};
/// use minusone::js::integer::ParseInt;
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = 'test'[1];").unwrap();
/// tree.apply_mut(&mut (ParseString::default(), ParseInt::default(), CharAt::default())).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "var x = 'e';");
/// ```
#[derive(Default)]
pub struct CharAt;

impl<'a> RuleMut<'a> for CharAt {
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
        if view.kind() != "subscript_expression" {
            return Ok(());
        }

        if let (Some(string), Some(index)) = (view.child(0), view.child(2)) {
            match (string.data(), index.data()) {
                (Some(Raw(Str(s))), Some(Raw(Num(i)))) => {
                    return if *i >= 0 && (*i as usize) < s.len() {
                        let ch = s.chars().nth(*i as usize).unwrap();
                        trace!("InferCharAt: reducing '{}'[{}] to '{}'", s, i, ch);
                        node.reduce(Raw(Str(ch.to_string())));
                        Ok(())
                    } else {
                        trace!("InferCharAt: index {} out of bounds, setting to undefined", i);
                        node.reduce(Undefined);
                        Ok(())
                    }
                }
                (Some(Raw(Str(s))), Some(Raw(Str(i)))) => {
                    if let Ok(i) = i.parse::<i64>() {
                        return if i >= 0 && (i as usize) < s.len() {
                            let ch = s.chars().nth(i as usize).unwrap();
                            trace!("InferCharAt: reducing '{}'[{}] to '{}'", s, i, ch);
                            node.reduce(Raw(Str(ch.to_string())));
                            Ok(())
                        } else {
                            trace!("InferCharAt: index {} out of bounds, setting to undefined", i);
                            node.reduce(Undefined);
                            Ok(())
                        };
                    } else {
                        warn!("InferCharAt: cannot parse index '{}' as number", i);
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }
}

pub fn escape_js_string(s: &str) -> String {
    let mut escaped = String::new();
    for c in s.chars() {
        match c {
            '\n' => escaped.push_str("\\n"),
            '\t' => escaped.push_str("\\t"),
            '\r' => escaped.push_str("\\r"),
            '\\' => escaped.push_str("\\\\"),
            '\'' => escaped.push_str("\\'"),
            _ => escaped.push(c),
        }
    }
    format!("'{}'", escaped)
}

/// Infers unary plus and minus on string literals
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::string::{ParseString, StringPlusMinus};
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = +'42'; var y = -'42';").unwrap();
/// tree.apply_mut(&mut (ParseString::default(), StringPlusMinus::default())).unwrap();
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
/// assert_eq!(linter.output, "var x = 42; var y = -42;");
/// ```
#[derive(Default)]
pub struct StringPlusMinus;

impl<'a> RuleMut<'a> for StringPlusMinus {
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
        if view.kind() != "unary_expression" {
            return Ok(());
        }

        if let (Some(operator), Some(operand)) = (view.child(0), view.child(1)) {
            match (operator.text()?, operand.data()) {
                ("+", Some(Raw(Str(s)))) => {
                    if let Ok(num) = s.parse::<i64>() {
                        trace!("StringPlusMinus: reducing + '{}' to {}", s, num);
                        node.reduce(Raw(Num(num)));
                    } else {
                        warn!("StringPlusMinus: cannot parse '{}' as number", s);
                    }
                }
                ("-", Some(Raw(Str(s)))) => {
                    if let Ok(num) = s.parse::<i64>() {
                        trace!("StringPlusMinus: reducing - '{}' to {}", s, -num);
                        node.reduce(Raw(Num(-num)));
                    } else {
                        warn!("StringPlusMinus: cannot parse '{}' as number", s);
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }
}

/// Infers string concatenation with + operator and reduces them to single string literals
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::string::{ParseString, Concat};
/// use minusone::js::integer::ParseInt;
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = 'Hello, ' + 'world!' + 1;").unwrap();
/// tree.apply_mut(&mut (ParseString::default(), ParseInt::default(), Concat::default())).unwrap();
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
/// assert_eq!(linter.output, "var x = 'Hello, world!1';");
/// ```
#[derive(Default)]
pub struct Concat;

impl<'a> RuleMut<'a> for Concat {
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
        if view.kind() != "binary_expression" {
            return Ok(());
        }

        if let (Some(left), Some(operator), Some(right)) =
            (view.child(0), view.child(1), view.child(2))
        {
            if operator.text()? == "+" {
                match (left.data(), right.data()) {
                    (Some(Raw(Str(s1))), Some(Raw(Str(s2)))) => {
                        trace!("Concat: reducing '{}' + '{}' to '{}'", s1, s2, s1.to_string() + s2);
                        node.reduce(Raw(Str(s1.to_string() + s2)));
                    }
                    // numbers + strings should also be concatenated as strings
                    (Some(Raw(Num(n))), Some(Raw(Str(s)))) => {
                        trace!("Concat: reducing {} + '{}' to '{}'", n, s, n.to_string() + s);
                        node.reduce(Raw(Str(n.to_string() + s)));
                    }
                    (Some(Raw(Str(s))), Some(Raw(Num(n)))) => {
                        trace!("Concat: reducing '{}' + {} to '{}'", s, n, s.to_string() + n.to_string().as_str());
                        node.reduce(Raw(Str(s.to_string() + n.to_string().as_str())));
                    }
                    (Some(Array(array)), Some(Raw(Str(s)))) => {
                        let array_str = flatten_array(array);
                        trace!("Concat: reducing array + '{}' to '{}'", s, array_str.to_string() + s);
                        node.reduce(Raw(Str(array_str.to_string() + s)));
                    }
                    (Some(Raw(Str(s))), Some(Array(array))) => {
                        let array_str = flatten_array(array);
                        trace!("Concat: reducing '{}' + array to '{}'", s, s.to_string() + array_str.as_str());
                        node.reduce(Raw(Str(s.to_string() + array_str.as_str())));
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests_js_string {
    use crate::js::string::{escape_js_string, unescaped_js_string};

    #[test]
    fn test_unescaped_js_string() {
        assert_eq!(unescaped_js_string(r#"'Hello\nWorld'"#), "Hello\nWorld");
        assert_eq!(unescaped_js_string(r#"'Tab\tSeparated'"#), "Tab\tSeparated");
        assert_eq!(unescaped_js_string(r#"'Quote: \"'"#), "Quote: \"");
        assert_eq!(unescaped_js_string(r#"'Backslash: \\'"#), "Backslash: \\");
        assert_eq!(unescaped_js_string(r#"'Unicode: \u0041'"#), "Unicode: A");
    }

    #[test]
    fn test_escape_js_string() {
        assert_eq!(escape_js_string("Hello\nWorld"), r#"'Hello\nWorld'"#);
        assert_eq!(escape_js_string("Tab\tSeparated"), r#"'Tab\tSeparated'"#);
        assert_eq!(escape_js_string("Quote: \""), r#"'Quote: \"'"#);
        assert_eq!(escape_js_string("Backslash: \\"), r#"'Backslash: \\'"#);
    }

    // todo: add tests for CharAt and StringPlusMinus rules
}
