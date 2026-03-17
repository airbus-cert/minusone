use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::{Array, Raw};
use crate::js::JavaScript::{NaN, Undefined};
use crate::js::Value::Bool;
use crate::js::Value::{Num, Str};
use crate::js::array::flatten_array;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use log::{trace, warn};

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
                        if let Some('{') = chars.peek() {
                            chars.next(); // consume '{'
                            while let Some(h) = chars.next() {
                                if h == '}' {
                                    break;
                                }
                                hex.push(h);
                            }
                        } else {
                            for _ in 0..4 {
                                if let Some(h) = chars.next() {
                                    hex.push(h);
                                } else {
                                    warn!("ParseString: incomplete unicode escape sequence");
                                    break;
                                }
                            }
                        }

                        if let Ok(code_point) = u32::from_str_radix(&hex, 16) {
                            if let Some(ch) = std::char::from_u32(code_point) {
                                result.push(ch);
                            } else {
                                warn!("ParseString: invalid unicode code point: {}", hex);
                            }
                        } else {
                            warn!("ParseString: invalid unicode escape sequence: {}", hex);
                        }
                    }
                    'x' => {
                        let mut hex = String::new();
                        for _ in 0..2 {
                            if let Some(h) = chars.next() {
                                hex.push(h);
                            } else {
                                warn!("ParseString: incomplete hex escape sequence");
                                break;
                            }
                        }
                        if let Ok(code_point) = u8::from_str_radix(&hex, 16) {
                            result.push(code_point as char);
                        } else {
                            warn!("ParseString: invalid hex escape sequence: {}", hex);
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
                    return if *i >= 0.0 && (*i as usize) < s.len() {
                        let ch = s.chars().nth(*i as usize).unwrap();
                        trace!("InferCharAt: reducing '{}'[{}] to '{}'", s, i, ch);
                        node.reduce(Raw(Str(ch.to_string())));
                        Ok(())
                    } else {
                        trace!(
                            "InferCharAt: index {} out of bounds, setting to undefined",
                            i
                        );
                        node.reduce(Undefined);
                        Ok(())
                    };
                }
                (Some(Raw(Str(s))), Some(Raw(Str(i)))) => {
                    if let Ok(i) = i.parse::<f64>() {
                        return if i >= 0.0 && (i as usize) < s.len() {
                            let ch = s.chars().nth(i as usize).unwrap();
                            trace!("InferCharAt: reducing '{}'[{}] to '{}'", s, i, ch);
                            node.reduce(Raw(Str(ch.to_string())));
                            Ok(())
                        } else {
                            trace!(
                                "InferCharAt: index {} out of bounds, setting to undefined",
                                i
                            );
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

/// Infers unary `+` and `-` on string literals
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
                    if let Ok(num) = s.parse::<f64>() {
                        trace!("StringPlusMinus: reducing + '{}' to {}", s, num);
                        node.reduce(Raw(Num(num)));
                    } else {
                        trace!(
                            "StringPlusMinus: cannot parse +'{}' as number, falling back to NaN",
                            s
                        );
                        node.reduce(NaN);
                    }
                }
                ("-", Some(Raw(Str(s)))) => {
                    if let Ok(num) = s.parse::<f64>() {
                        trace!("StringPlusMinus: reducing - '{}' to {}", s, -num);
                        node.reduce(Raw(Num(-num)));
                    } else {
                        trace!(
                            "StringPlusMinus: cannot parse -'{}' as number, falling back to NaN",
                            s
                        );
                        node.reduce(NaN);
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }
}

/// Infers string concatenation with `+` and reduces them to single string literals
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
                        trace!(
                            "Concat: reducing '{}' + '{}' to '{}'",
                            s1,
                            s2,
                            s1.to_string() + s2
                        );
                        node.reduce(Raw(Str(s1.to_string() + s2)));
                    }
                    // numbers + strings should also be concatenated as strings
                    (Some(Raw(Num(n))), Some(Raw(Str(s)))) => {
                        trace!(
                            "Concat: reducing {} + '{}' to '{}'",
                            n,
                            s,
                            n.to_string() + s
                        );
                        node.reduce(Raw(Str(n.to_string() + s)));
                    }
                    (Some(Raw(Str(s))), Some(Raw(Num(n)))) => {
                        trace!(
                            "Concat: reducing '{}' + {} to '{}'",
                            s,
                            n,
                            s.to_string() + n.to_string().as_str()
                        );
                        node.reduce(Raw(Str(s.to_string() + n.to_string().as_str())));
                    }
                    (Some(Array(array)), Some(Raw(Str(s)))) => {
                        let array_str = flatten_array(array);
                        trace!(
                            "Concat: reducing array + '{}' to '{}'",
                            s,
                            array_str.to_string() + s
                        );
                        node.reduce(Raw(Str(array_str.to_string() + s)));
                    }
                    (Some(Raw(Str(s))), Some(Array(array))) => {
                        let array_str = flatten_array(array);
                        trace!(
                            "Concat: reducing '{}' + array to '{}'",
                            s,
                            s.to_string() + array_str.as_str()
                        );
                        node.reduce(Raw(Str(s.to_string() + array_str.as_str())));
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }
}

/// Infers toString calls
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::string::{ParseString, ToString};
/// use minusone::js::integer::ParseInt;
/// use minusone::js::array::ParseArray;
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = 31['toString']('32');").unwrap();
/// tree.apply_mut(&mut (
///     ParseString::default(), ParseInt::default(), ParseArray::default(), ToString::default()
/// )).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
/// assert_eq!(linter.output, "var x = 'v';");
/// ```
#[derive(Default)]
pub struct ToString;

impl<'a> RuleMut<'a> for ToString {
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

        if let (Some(subscript_expression), Some(arguments)) = (view.child(0), view.child(1)) {
            if subscript_expression.kind() == "subscript_expression" {
                if let (Some(object), Some(property)) =
                    (subscript_expression.child(0), subscript_expression.child(2))
                {
                    if property.data() == Some(&Raw(Str("toString".to_string()))) {
                        // get radix argument if exists
                        let radix = if arguments.child_count() > 2 {
                            if let Some(arg) = arguments.child(1) {
                                if let Some(Raw(Num(radix))) = arg.data() {
                                    *radix as i64
                                } else if let Some(Raw(Str(radix_str))) = arg.data() {
                                    if let Ok(radix) = radix_str.parse::<i64>() {
                                        radix
                                    } else {
                                        warn!(
                                            "ToString: cannot parse radix argument '{}' as number, defaulting to 10",
                                            radix_str
                                        );
                                        10
                                    }
                                } else {
                                    warn!(
                                        "ToString: unsupported radix argument type, defaulting to 10"
                                    );
                                    10
                                }
                            } else {
                                10
                            }
                        } else {
                            10
                        };

                        let result = match object.data() {
                            Some(Raw(Num(n))) => {
                                if radix == 10 {
                                    n.to_string()
                                } else if radix >= 2 && radix <= 36 {
                                    let mut num = *n as i64;
                                    let mut result = String::new();
                                    let negative = num < 0;
                                    if negative {
                                        num = -num;
                                    }
                                    while num > 0 {
                                        let digit = (num % radix) as u8;
                                        result.push(if digit < 10 {
                                            (b'0' + digit) as char
                                        } else {
                                            (b'a' + digit - 10) as char
                                        });
                                        num /= radix;
                                    }
                                    if negative {
                                        result.push('-');
                                    }
                                    result.chars().rev().collect()
                                } else {
                                    warn!("ToString: invalid radix {}, defaulting to 10", radix);
                                    n.to_string()
                                }
                            }
                            Some(Raw(Bool(b))) => b.to_string(),
                            Some(Raw(Str(s))) => s.to_string(),
                            Some(Array(array)) => flatten_array(array),
                            _ => {
                                warn!("ToString: unsupported object type for toString call");
                                return Ok(());
                            }
                        };

                        trace!(
                            "ToString: reducing {:?}['toString']({}) to '{}'",
                            object.data(),
                            radix,
                            result
                        );
                        node.reduce(Raw(Str(result)));
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests_js_string {
    use crate::js::build_javascript_tree;
    use crate::js::integer::ParseInt;
    use crate::js::linter::Linter;
    use crate::js::specials::AddSubSpecials;
    use crate::js::string::*;
    use crate::js::string::{escape_js_string, unescaped_js_string};

    fn deobfuscate_string(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            ParseInt::default(),
            StringPlusMinus::default(),
            CharAt::default(),
            Concat::default(),
            ToString::default(),
            AddSubSpecials::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_unescaped_js_string() {
        assert_eq!(unescaped_js_string(r#"'Hello\nWorld'"#), "Hello\nWorld");
        assert_eq!(unescaped_js_string(r#"'Tab\tSeparated'"#), "Tab\tSeparated");
        assert_eq!(unescaped_js_string(r#"'Quote: \"'"#), "Quote: \"");
        assert_eq!(unescaped_js_string(r#"'Backslash: \\'"#), "Backslash: \\");
        assert_eq!(unescaped_js_string(r#"'Unicode: \u0041'"#), "Unicode: A");
        assert_eq!(
            unescaped_js_string(
                r#"'Unicode: \u0030 \u{00030} \u{000030} \u{0000000000000030} \u{30}'"#
            ),
            "Unicode: 0 0 0 0 0"
        );
        assert_eq!(unescaped_js_string(r#"'Hex: \x41'"#), "Hex: A");
    }

    #[test]
    fn test_escape_js_string() {
        assert_eq!(escape_js_string("Hello\nWorld"), r#"'Hello\nWorld'"#);
        assert_eq!(escape_js_string("Tab\tSeparated"), r#"'Tab\tSeparated'"#);
        assert_eq!(escape_js_string("Quote: \""), r#"'Quote: "'"#);
        assert_eq!(escape_js_string("Backslash: \\"), r#"'Backslash: \\'"#);
    }

    #[test]
    fn test_concat() {
        assert_eq!(
            deobfuscate_string("var x = 'Hello, ' + 'world!' + 1;"),
            "var x = 'Hello, world!1';"
        );
    }

    #[test]
    fn test_charat() {
        assert_eq!(deobfuscate_string("var x = 'test'[1];"), "var x = 'e';");
        assert_eq!(
            deobfuscate_string("var x = 'test'[10];"),
            "var x = undefined;"
        );
    }

    #[test]
    fn test_charat_concat() {
        assert_eq!(
            deobfuscate_string(
                "var x = 'minusone'[0] + 'minusone'[1] + 'minusone'[2] + 'minusone'[3] + 'minusone'[4] + 'minusone'[5] + 'minusone'[6] + 'minusone'[7];"
            ),
            "var x = 'minusone';"
        );
    }

    #[test]
    fn test_string_plus_minus() {
        assert_eq!(
            deobfuscate_string("var x = +'42'; var y = -'42';"),
            "var x = 42; var y = -42;"
        );
        assert_eq!(
            deobfuscate_string("var x = 'b' + 'a' + +'a' + 'a'"),
            "var x = 'baNaNa'"
        );
    }
}
