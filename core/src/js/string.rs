use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::{Array, Null, Object, Raw, Regex};
use crate::js::JavaScript::{NaN, Undefined};
use crate::js::Value::{BigInt, Bool};
use crate::js::Value::{Num, Str};
use crate::js::array::flatten_array;
use crate::js::integer::ParseInt;
use crate::js::regex::RegexExec;
use crate::js::utils::{get_positional_arguments, method_name};
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use log::{error, trace, warn};

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

/// Centralized dispatcher for string literal builtins.
type StringBuiltinHandler = fn(&str, &[JavaScript]) -> Option<JavaScript>;

const STRING_BUILTINS: &[(&str, StringBuiltinHandler)] = &[
    ("at", string_builtin_at),
    ("charAt", string_builtin_char_at),
    ("split", string_builtin_split),
    ("replace", string_builtin_replace),
    ("replaceAll", string_builtin_replace_all),
    ("link", string_builtin_link),
    ("anchor", string_builtin_anchor),
    ("codePointAt", string_builtin_code_point_at),
    ("startsWith", string_builtin_start_with),
    ("endsWith", string_builtin_end_with),
    ("includes", string_builtin_includes),
    ("indexOf", string_builtin_index_of),
    ("lastIndexOf", string_builtin_last_index_of),
    ("padStart", string_builtin_pad_start),
    ("padEnd", string_builtin_pad_end),
];

#[derive(Default)]
pub struct StringBuiltins;

impl<'a> RuleMut<'a> for StringBuiltins {
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

        let Some(object) = callee.child(0).or_else(|| callee.named_child("object")) else {
            return Ok(());
        };
        let Some(Raw(Str(input))) = object.data() else {
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

        let Some(result) = dispatch_string_builtin(&method, input, &arg_values) else {
            return Ok(());
        };

        trace!(
            "StringBuiltins: reducing '{}'.{}(...) to {}",
            input, method, result
        );
        node.reduce(result);
        Ok(())
    }
}

fn dispatch_string_builtin(method: &str, input: &str, args: &[JavaScript]) -> Option<JavaScript> {
    STRING_BUILTINS
        .iter()
        .find_map(|(name, handler)| (*name == method).then(|| handler(input, args)))
        .flatten()
}

fn string_builtin_at(input: &str, args: &[JavaScript]) -> Option<JavaScript> {
    let index = js_index_from_optional_arg(args.first());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len() as i64;
    let normalized = if index >= 0 { index } else { len + index };

    if normalized < 0 || normalized >= len {
        return Some(Undefined);
    }

    Some(Raw(Str(chars[normalized as usize].to_string())))
}

/// The `charAt` method works the same way as `at`, BUT it does not support negative indices and
/// returns an empty string if the index is out of bounds, instead of `undefined`. It seems to me that
/// JavaScript was developed by several developers, but that they never look at each other's code...
fn string_builtin_char_at(input: &str, args: &[JavaScript]) -> Option<JavaScript> {
    let index = js_index_from_optional_arg(args.first());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len() as i64;

    if index < 0 || index >= len {
        return Some(Raw(Str(String::new())));
    }

    Some(Raw(Str(chars[index as usize].to_string())))
}

fn string_builtin_code_point_at(input: &str, args: &[JavaScript]) -> Option<JavaScript> {
    let index = js_index_from_optional_arg(args.first());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len() as i64;

    if index < 0 || index >= len {
        return Some(Undefined);
    }

    Some(Raw(Num(chars[index as usize] as u32 as f64)))
}

fn js_index_from_optional_arg(value: Option<&JavaScript>) -> i64 {
    match value {
        None => 0,
        Some(v) => match v.as_js_num() {
            Raw(Num(n)) if n.is_finite() => n.trunc() as i64,
            Raw(Num(_)) | NaN => 0,
            _ => 0,
        },
    }
}

fn string_builtin_start_with(input: &str, args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(Raw(Bool(false)));
    }

    let to_find = match args.first()? {
        Raw(Str(s)) => s.clone(),
        Array(a) => flatten_array(a, None),
        any => any.to_string(),
    };

    Some(Raw(Bool(input.starts_with(&to_find))))
}

fn string_builtin_end_with(input: &str, args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(Raw(Bool(false)));
    }

    let to_find = match args.first()? {
        Raw(Str(s)) => s.clone(),
        Array(a) => flatten_array(a, None),
        any => any.to_string(),
    };

    Some(Raw(Bool(input.ends_with(&to_find))))
}

fn string_builtin_includes(input: &str, args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(Raw(Bool(false)));
    }

    let to_find = match args.first()? {
        Raw(Str(s)) => s.clone(),
        Array(a) => flatten_array(a, None),
        any => any.to_string(),
    };

    Some(Raw(Bool(input.contains(&to_find))))
}

fn string_builtin_index_of(input: &str, args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(Raw(Num(-1.0)));
    }

    let to_find = match args.first()? {
        Raw(Str(s)) => s.clone(),
        Array(a) => flatten_array(a, None),
        any => any.to_string(),
    };

    Some(Raw(Num(input
        .find(&to_find)
        .map(|i| i as f64)
        .unwrap_or(-1.0))))
}

fn string_builtin_last_index_of(input: &str, args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(Raw(Num(-1.0)));
    }

    let to_find = match args.first()? {
        Raw(Str(s)) => s.clone(),
        Array(a) => flatten_array(a, None),
        any => any.to_string(),
    };

    Some(Raw(Num(input
        .rfind(&to_find)
        .map(|i| i as f64)
        .unwrap_or(-1.0))))
}

fn string_builtin_pad(input: &str, args: &[JavaScript], pad_start: bool) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(Raw(Str(input.to_string())));
    }

    let target_length = match args.first()? {
        Raw(Num(n)) if n.is_finite() && *n >= 0.0 => *n as usize,
        _ => 0,
    };

    let pad_string = match args.get(1) {
        None => " ".to_string(),
        Some(Raw(Str(s))) => s.clone(),
        Some(Array(a)) => flatten_array(a, None),
        Some(js) => js.to_string(),
    };

    if input.len() >= target_length {
        return Some(Raw(Str(input.to_string())));
    }

    let padding_needed = target_length - input.len();
    match pad_string.as_str() {
        "" => Some(Raw(Str(input.to_string()))),
        _ => {
            let repeated_pad =
                pad_string.repeat((padding_needed + pad_string.len() - 1) / pad_string.len());
            let final_pad = &repeated_pad[..padding_needed];
            if pad_start {
                Some(Raw(Str(format!("{}{}", final_pad, input))))
            } else {
                Some(Raw(Str(format!("{}{}", input, final_pad))))
            }
        }
    }
}
fn string_builtin_pad_start(input: &str, args: &[JavaScript]) -> Option<JavaScript> {
    string_builtin_pad(input, args, true)
}

fn string_builtin_pad_end(input: &str, args: &[JavaScript]) -> Option<JavaScript> {
    string_builtin_pad(input, args, false)
}

fn string_builtin_split(input: &str, args: &[JavaScript]) -> Option<JavaScript> {
    let separator_owned: Option<String> = match args.first() {
        None => None,
        Some(Undefined) => None,
        Some(Raw(Str(s))) => Some(s.clone()),
        Some(Raw(Num(n))) => Some(n.to_string()),
        _ => return None,
    };

    let limit = match args.get(1) {
        None => None,
        Some(Raw(Num(n))) if *n >= 0.0 => Some(*n as usize),
        Some(Raw(Num(_))) => Some(0),
        _ => return None,
    };

    let mut parts = split_parts(input, separator_owned.as_deref());
    if let Some(limit) = limit {
        parts.truncate(limit);
    }

    Some(Array(parts.into_iter().map(|s| Raw(Str(s))).collect()))
}

fn string_builtin_replace(input: &str, args: &[JavaScript]) -> Option<JavaScript> {
    string_builtin_replace_like(input, args, false)
}

fn string_builtin_replace_all(input: &str, args: &[JavaScript]) -> Option<JavaScript> {
    string_builtin_replace_like(input, args, true)
}

fn string_builtin_replace_like(
    input: &str,
    args: &[JavaScript],
    replace_all: bool,
) -> Option<JavaScript> {
    let replacement = match args.get(1) {
        None => "undefined".to_string(),
        Some(Raw(Str(s))) => s.clone(),
        Some(js) => js.to_string(),
    };

    let result = match args.first() {
        None => return None,
        Some(Regex { pattern, flags }) => {
            let regex = RegexExec::compile(pattern, flags)?;
            if replace_all {
                if !flags.contains('g') {
                    error!(
                        "Replace: replaceAll called with regex without global flag, treating as replace. This should crash the engine, skipping."
                    );
                    return None;
                }
                regex.replace_all(input, &replacement).to_string()
            } else if flags.contains('g') {
                regex.replace_all(input, &replacement).to_string()
            } else {
                regex.replace(input, &replacement).to_string()
            }
        }
        Some(Raw(Str(s))) => {
            if replace_all {
                input.replace(s, &replacement)
            } else {
                input.replacen(s, &replacement, 1)
            }
        }
        Some(js) => {
            let pattern = js.to_string();
            if replace_all {
                input.replace(&pattern, &replacement)
            } else {
                input.replacen(&pattern, &replacement, 1)
            }
        }
    };

    Some(Raw(Str(result)))
}

fn string_builtin_link(input: &str, args: &[JavaScript]) -> Option<JavaScript> {
    string_builtin_dynamic_tag(input, args, true)
}

fn string_builtin_anchor(input: &str, args: &[JavaScript]) -> Option<JavaScript> {
    string_builtin_dynamic_tag(input, args, false)
}

fn string_builtin_dynamic_tag(
    input: &str,
    args: &[JavaScript],
    is_link: bool,
) -> Option<JavaScript> {
    let tag_content = match args.first() {
        None => "undefined".to_string(),
        Some(Raw(Str(s))) => s.clone(),
        Some(js) => js.to_string(),
    };

    let escaped = tag_content.replace('"', "&quot;");
    let result = if is_link {
        format!(r#"<a href="{}">{}</a>"#, escaped, input)
    } else {
        format!(r#"<a name="{}">{}</a>"#, escaped, input)
    };

    Some(Raw(Str(result)))
}

/// Infers charAt with bracket calls on string literals and reduces them to single-character string
/// literals using arrays indexes
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::string::{ParseString, BracketCharAt};
/// use minusone::js::integer::ParseInt;
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = 'test'[1];").unwrap();
/// tree.apply_mut(&mut (ParseString::default(), ParseInt::default(), BracketCharAt::default())).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "var x = 'e';");
/// ```
#[derive(Default)]
pub struct BracketCharAt;

impl<'a> RuleMut<'a> for BracketCharAt {
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
        if view.kind() == "subscript_expression" {
            if let (Some(string), Some(index)) = (view.child(0), view.child(2)) {
                match (string.data(), index.data()) {
                    (Some(Raw(Str(str))), Some(js)) => match js.as_js_num() {
                        Raw(Num(i)) => {
                            let index = i as i64;
                            if index >= 0 && (index as usize) < str.chars().count() {
                                let ch = str.chars().nth(index as usize).unwrap();
                                trace!("InferCharAt: reducing '{}'[{}] to '{}'", str, index, ch);
                                node.reduce(Raw(Str(ch.to_string())));
                            } else {
                                trace!(
                                    "InferCharAt: index {} out of bounds, setting to undefined",
                                    index
                                );
                                node.reduce(Undefined);
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
            return Ok(());
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

#[derive(Default)]
pub struct CharCodeAt;

impl<'a> RuleMut<'a> for CharCodeAt {
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
        if method != "charCodeAt" {
            return Ok(());
        }

        let Some(object) = callee.child(0).or_else(|| callee.named_child("object")) else {
            return Ok(());
        };
        let Some(Raw(Str(input))) = object.data() else {
            return Ok(());
        };

        let args = view.named_child("arguments");
        let positional_args = get_positional_arguments(args);

        let index: usize = match positional_args.first().and_then(|a| a.data()) {
            None => 0,
            Some(Raw(Str(s))) => s.parse::<f64>().ok().map(|n| n as usize).unwrap_or(0),
            Some(Raw(Num(n))) => *n as usize,
            _ => 0,
        };

        let bytes = input.as_bytes();

        let result = if index < bytes.len() {
            Raw(Num(bytes[index] as f64))
        } else {
            NaN
        };

        trace!(
            "InferCharCodeAt: reducing '{}'.charCodeAt({}) to {}",
            input, index, result
        );
        node.reduce(result);

        Ok(())
    }
}

/// Infers `String.fromCharCode(...)` static calls.
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::string::{FromCharCode, ParseString};
/// use minusone::js::integer::ParseInt;
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = String.fromCharCode(65, 66, 67);").unwrap();
/// tree.apply_mut(&mut (ParseString::default(), ParseInt::default(), FromCharCode::default())).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
/// assert_eq!(linter.output, "var x = 'ABC';");
/// ```
#[derive(Default)]
pub struct FromCharCode;

impl<'a> RuleMut<'a> for FromCharCode {
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
        if method != "fromCharCode" {
            return Ok(());
        }

        let Some(object) = callee.child(0).or_else(|| callee.named_child("object")) else {
            return Ok(());
        };
        let Ok(object_name) = object.text() else {
            return Ok(());
        };
        if object_name != "String" {
            return Ok(());
        }

        let args = view.named_child("arguments");
        let positional_args = get_positional_arguments(args);

        let mut code_units = Vec::with_capacity(positional_args.len());
        for arg in positional_args {
            let Some(value) = arg.data() else {
                return Ok(());
            };
            let Some(num) = js_to_number_for_from_char_code(value) else {
                return Ok(());
            };
            code_units.push(to_uint16(num));
        }

        let mut out = String::new();
        for ch in std::char::decode_utf16(code_units.into_iter()) {
            let Ok(ch) = ch else {
                return Ok(());
            };
            out.push(ch);
        }

        trace!(
            "FromCharCode: reducing String.fromCharCode(...) to '{}'",
            out
        );
        node.reduce(Raw(Str(out)));
        Ok(())
    }
}

/// Infers `String(...)` coercion calls.
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::string::{ParseString, StringConstructor};
/// use minusone::js::integer::ParseInt;
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = String(1);").unwrap();
/// tree.apply_mut(&mut (ParseString::default(), ParseInt::default(), StringConstructor::default())).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
/// assert_eq!(linter.output, "var x = '1';");
/// ```
#[derive(Default)]
pub struct StringConstructor;

impl<'a> RuleMut<'a> for StringConstructor {
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
        if callee.kind() != "identifier" {
            return Ok(());
        }
        if callee.text()? != "String" {
            return Ok(());
        }

        let args = view.named_child("arguments");
        let positional_args = get_positional_arguments(args);

        let result = match positional_args.first().and_then(|a| a.data()) {
            None => String::new(),
            Some(Raw(Str(s))) => s.clone(),
            Some(value) => value.to_string(),
        };

        trace!("StringConstructor: reducing String(...) to '{}'", result);
        node.reduce(Raw(Str(result)));
        Ok(())
    }
}

fn to_uint16(n: f64) -> u16 {
    if !n.is_finite() || n == 0.0 {
        return 0;
    }
    (n.trunc() as i64).rem_euclid(65536) as u16
}

fn js_to_number_for_from_char_code(value: &JavaScript) -> Option<f64> {
    match value {
        Raw(Num(n)) => Some(*n),
        Raw(Str(s)) => {
            let t = s.trim();
            if t.is_empty() {
                Some(0.0)
            } else if let Ok(n) = t.parse::<f64>() {
                Some(n)
            } else {
                match ParseInt::from_str(t) {
                    Raw(Num(n)) => Some(n),
                    _ => None,
                }
            }
        }
        Raw(Bool(b)) => Some(if *b { 1.0 } else { 0.0 }),
        Undefined | NaN => Some(f64::NAN),
        Null => Some(0.0),
        _ => None,
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
                        let array_str = flatten_array(array, None);
                        trace!(
                            "Concat: reducing array + '{}' to '{}'",
                            s,
                            array_str.to_string() + s
                        );
                        node.reduce(Raw(Str(array_str.to_string() + s)));
                    }
                    (Some(Raw(Str(s))), Some(Array(array))) => {
                        let array_str = flatten_array(array, None);
                        trace!(
                            "Concat: reducing '{}' + array to '{}'",
                            s,
                            s.to_string() + array_str.as_str()
                        );
                        node.reduce(Raw(Str(s.to_string() + array_str.as_str())));
                    }
                    (Some(Raw(Str(s))), Some(Raw(BigInt(b)))) => {
                        trace!(
                            "Concat: reducing '{}' + {}n to '{}'",
                            s,
                            b,
                            s.to_string() + b.to_string().as_str()
                        );
                        node.reduce(Raw(Str(s.to_string() + b.to_string().as_str())));
                    }
                    (Some(Raw(BigInt(b))), Some(Raw(Str(s)))) => {
                        trace!(
                            "Concat: reducing {}n + '{}' to '{}'",
                            b,
                            s,
                            b.to_string() + s.to_string().as_str()
                        );
                        node.reduce(Raw(Str(b.to_string() + s.to_string().as_str())));
                    }

                    (Some(Raw(Str(s))), Some(Raw(Bool(b)))) => {
                        trace!(
                            "Concat: reducing '{}' + {} to '{}'",
                            s,
                            b,
                            s.to_string() + b.to_string().as_str()
                        );
                        node.reduce(Raw(Str(s.to_string() + b.to_string().as_str())));
                    }
                    (Some(Raw(Bool(b))), Some(Raw(Str(s)))) => {
                        trace!(
                            "Concat: reducing {} + '{}' to '{}'",
                            b,
                            s,
                            b.to_string() + s.to_string().as_str()
                        );
                        node.reduce(Raw(Str(b.to_string() + s.to_string().as_str())));
                    }
                    (
                        Some(Raw(Str(s))),
                        Some(Object {
                            to_string_override: Some(obj_str),
                            ..
                        }),
                    ) => {
                        trace!(
                            "Concat: reducing '{}' + object override to '{}{}'",
                            s, s, obj_str
                        );
                        node.reduce(Raw(Str(format!("{}{}", s, obj_str))));
                    }
                    (
                        Some(Object {
                            to_string_override: Some(obj_str),
                            ..
                        }),
                        Some(Raw(Str(s))),
                    ) => {
                        trace!(
                            "Concat: reducing object override + '{}' to '{}{}'",
                            s, obj_str, s
                        );
                        node.reduce(Raw(Str(format!("{}{}", obj_str, s))));
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

        let Some(callee) = view.named_child("function").or_else(|| view.child(0)) else {
            return Ok(());
        };

        let is_to_string = match callee.kind() {
            "subscript_expression" => callee
                .child(2)
                .map(|property| {
                    property.data() == Some(&Raw(Str("toString".to_string())))
                        || property
                            .text()
                            .ok()
                            .map(|t| t.trim_matches(['\'', '"']).to_string())
                            .as_deref()
                            == Some("toString")
                })
                .unwrap_or(false),
            "member_expression" => callee
                .named_child("property")
                .and_then(|p| p.text().ok().map(|t| t == "toString"))
                .unwrap_or(false),
            _ => false,
        };

        if !is_to_string {
            return Ok(());
        }

        let Some(object) = callee.child(0).or_else(|| callee.named_child("object")) else {
            return Ok(());
        };

        let args = view.named_child("arguments");
        let positional_args = get_positional_arguments(args);

        // get radix argument if exists
        let radix = if let Some(arg) = positional_args.first() {
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
                warn!("ToString: unsupported radix argument type, defaulting to 10");
                10
            }
        } else {
            10
        };

        let object_value = object
            .data()
            .cloned()
            .or_else(|| object.iter().find_map(|child| child.data().cloned()));

        let result = match object_value.as_ref() {
            Some(Raw(Num(n))) => {
                if radix == 10 {
                    n.to_string()
                } else if (2..=36).contains(&radix) {
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
            Some(Array(array)) => flatten_array(array, None),
            _ => {
                warn!("ToString: unsupported object type for toString call");
                return Ok(());
            }
        };

        trace!(
            "ToString: reducing {:?}.toString({}) to '{}'",
            object_value, radix, result
        );
        node.reduce(Raw(Str(result)));

        Ok(())
    }
}

fn split_parts(input: &str, separator: Option<&str>) -> Vec<String> {
    match separator {
        None => vec![input.to_string()],
        Some("") => input.chars().map(|c| c.to_string()).collect(),
        Some(sep) => input.split(sep).map(|s| s.to_string()).collect(),
    }
}

#[cfg(test)]
mod tests_js_string {
    use crate::js::array::{GetArrayElement, ParseArray};
    use crate::js::build_javascript_tree;
    use crate::js::forward::Forward;
    use crate::js::integer::{ParseInt, PosNeg};
    use crate::js::linter::Linter;
    use crate::js::regex::ParseRegex;
    use crate::js::specials::AddSubSpecials;
    use crate::js::string::*;
    use crate::js::string::{escape_js_string, unescaped_js_string};

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            ParseInt::default(),
            ParseArray::default(),
            ParseRegex::default(),
            StringBuiltins::default(),
            Forward::default(),
            PosNeg::default(),
            BracketCharAt::default(),
            CharCodeAt::default(),
            FromCharCode::default(),
            StringConstructor::default(),
            Concat::default(),
            GetArrayElement::default(),
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
            deobfuscate("var x = 'Hello, ' + 'world!' + 1;"),
            "var x = 'Hello, world!1';"
        );
    }

    #[test]
    fn test_charat() {
        assert_eq!(deobfuscate("var x = 'abc'.charAt();"), "var x = 'a';");
        assert_eq!(deobfuscate("var x = 'abc'.charAt(1);"), "var x = 'b';");
        assert_eq!(deobfuscate("var x = 'abc'['charAt'](2);"), "var x = 'c';");
        assert_eq!(deobfuscate("var x = 'abc'.charAt(3);"), "var x = '';");
        assert_eq!(deobfuscate("var x = 'abc'.charAt(-3);"), "var x = '';");
        assert_eq!(deobfuscate("var x = 'abc'.charAt('1');"), "var x = 'b';");
        assert_eq!(deobfuscate("var x = 'test'[1];"), "var x = 'e';");
        assert_eq!(deobfuscate("var x = 'test'[10];"), "var x = undefined;");
        assert_eq!(deobfuscate("var x = 'abc'[0];"), "var x = 'a';");
        assert_eq!(deobfuscate("var x = 'abc'[-1];"), "var x = undefined;");
        assert_eq!(deobfuscate("var x = 'abc'[3];"), "var x = undefined;");
    }

    #[test]
    fn test_at() {
        assert_eq!(deobfuscate("var x = 'abc'.at();"), "var x = 'a';");
        assert_eq!(deobfuscate("var x = 'abc'.at(1);"), "var x = 'b';");
        assert_eq!(deobfuscate("var x = 'abc'.at(-1);"), "var x = 'c';");
        assert_eq!(deobfuscate("var x = 'abc'.at('2');"), "var x = 'c';");
        assert_eq!(deobfuscate("var x = 'abc'['at']('-2');"), "var x = 'b';");
        assert_eq!(deobfuscate("var x = 'abc'.at(10);"), "var x = undefined;");
    }

    #[test]
    fn test_charat_concat() {
        assert_eq!(
            deobfuscate(
                "var x = 'minusone'[0] + 'minusone'[1] + 'minusone'[2] + 'minusone'[3] + 'minusone'[4] + 'minusone'[5] + 'minusone'[6] + 'minusone'[7];"
            ),
            "var x = 'minusone';"
        );
    }

    #[test]
    fn test_charcodeat() {
        assert_eq!(deobfuscate("var x = 'ABC'.charCodeAt(0);"), "var x = 65;");
        assert_eq!(deobfuscate("var x = 'ABC'.charCodeAt(14);"), "var x = NaN;");
    }

    #[test]
    fn test_from_char_code() {
        assert_eq!(
            deobfuscate(
                "var x = String.fromCharCode(0x6D, 0x69, 0x6E, 0x75, 0x73, 0x6F, 0x6E, 0x65);"
            ),
            "var x = 'minusone';"
        );
        assert_eq!(
            deobfuscate("var x = String['fromCharCode'](65, 66, 67);"),
            "var x = 'ABC';"
        );
    }

    #[test]
    fn test_code_point_at() {
        assert_eq!(deobfuscate("var x = 'abc'.codePointAt(1);"), "var x = 98;");
        assert_eq!(deobfuscate("var x = 'abc'.codePointAt();"), "var x = 97;");
        assert_eq!(
            deobfuscate("var x = '☃★♲'.codePointAt(1);"),
            "var x = 9733;"
        );
        assert_eq!(
            deobfuscate("var x = 'abc'.codePointAt(-1);"),
            "var x = undefined;"
        );
        assert_eq!(
            deobfuscate("var x = 'abc'.codePointAt(3);"),
            "var x = undefined;"
        );
    }

    #[test]
    fn test_string_constructor() {
        assert_eq!(deobfuscate("var x = String(1);"), "var x = '1';");
        assert_eq!(deobfuscate("var x = String();"), "var x = '';");
    }

    #[test]
    fn test_string_plus_minus() {
        assert_eq!(
            deobfuscate("var x = +'42'; var y = -'42';"),
            "var x = 42; var y = -42;"
        );
        assert_eq!(deobfuscate("var x = +'0xff';"), "var x = 255;");
        assert_eq!(deobfuscate("var x = +'-0x56';"), "var x = NaN;");
        assert_eq!(deobfuscate("var x = +'-56';"), "var x = -56;");
        assert_eq!(
            deobfuscate("var x = 'b' + 'a' + +'a' + 'a'"),
            "var x = 'baNaNa'"
        );
    }

    #[test]
    fn test_start_with() {
        assert_eq!(
            deobfuscate("var x = '123'.startsWith('1');"),
            "var x = true;"
        );
        assert_eq!(
            deobfuscate("var x = '123'.startsWith('2');"),
            "var x = false;"
        );
        assert_eq!(
            deobfuscate("var x = '123'.startsWith([1]);"),
            "var x = true;"
        );
        assert_eq!(
            deobfuscate("var x = '123'.startsWith('');"),
            "var x = true;"
        );
        assert_eq!(
            deobfuscate("var x = '123'.startsWith([]);"),
            "var x = true;"
        );
    }

    #[test]
    fn test_end_with() {
        assert_eq!(deobfuscate("var x = '123'.endsWith('3');"), "var x = true;");
        assert_eq!(
            deobfuscate("var x = '123'.endsWith('2');"),
            "var x = false;"
        );
        assert_eq!(deobfuscate("var x = '123'.endsWith([3]);"), "var x = true;");
        assert_eq!(deobfuscate("var x = '123'.endsWith('');"), "var x = true;");
        assert_eq!(deobfuscate("var x = '123'.endsWith([]);"), "var x = true;");
    }

    #[test]
    fn test_includes() {
        assert_eq!(deobfuscate("var x = '123'.includes('3');"), "var x = true;");
        assert_eq!(deobfuscate("var x = '123'.includes('2');"), "var x = true;");
        assert_eq!(
            deobfuscate("var x = '123'.includes('4');"),
            "var x = false;"
        );
        assert_eq!(deobfuscate("var x = '123'.includes([1]);"), "var x = true;");
        assert_eq!(deobfuscate("var x = '123'.includes('');"), "var x = true;");
        assert_eq!(deobfuscate("var x = '123'.includes([]);"), "var x = true;");
    }

    #[test]
    fn test_index_of() {
        assert_eq!(deobfuscate("var x = '123'.indexOf('3');"), "var x = 2;");
        assert_eq!(deobfuscate("var x = '123'.indexOf('2');"), "var x = 1;");
        assert_eq!(deobfuscate("var x = '123'.indexOf('4');"), "var x = -1;");
        assert_eq!(deobfuscate("var x = '123'.indexOf([1]);"), "var x = 0;");
        assert_eq!(deobfuscate("var x = '123'.indexOf('');"), "var x = 0;");
        assert_eq!(deobfuscate("var x = '123'.indexOf();"), "var x = -1;");
        assert_eq!(deobfuscate("var x = '123'.indexOf([]);"), "var x = 0;");
    }

    #[test]
    fn test_last_index_of() {
        assert_eq!(
            deobfuscate("var x = '123123'.lastIndexOf('3');"),
            "var x = 5;"
        );
        assert_eq!(
            deobfuscate("var x = '123123'.lastIndexOf('2');"),
            "var x = 4;"
        );
        assert_eq!(
            deobfuscate("var x = '123123'.lastIndexOf('4');"),
            "var x = -1;"
        );
        assert_eq!(
            deobfuscate("var x = '123123'.lastIndexOf([1]);"),
            "var x = 3;"
        );
        assert_eq!(
            deobfuscate("var x = '123123'.lastIndexOf('');"),
            "var x = 6;"
        );
        assert_eq!(
            deobfuscate("var x = '123123'.lastIndexOf();"),
            "var x = -1;"
        );
        assert_eq!(
            deobfuscate("var x = '123123'.lastIndexOf([]);"),
            "var x = 6;"
        );
    }

    #[test]
    fn test_pad() {
        assert_eq!(
            deobfuscate("var x = '123'.padStart(5, '0');"),
            "var x = '00123';"
        );
        assert_eq!(
            deobfuscate("var x = '123'.padEnd(5, '0');"),
            "var x = '12300';"
        );
        assert_eq!(
            deobfuscate("var x = '123'.padStart(5);"),
            "var x = '  123';"
        );
        assert_eq!(deobfuscate("var x = '123'.padEnd(5);"), "var x = '123  ';");
    }

    #[test]
    fn test_to_string_dot_and_subscript() {
        assert_eq!(deobfuscate("var x = (1)['toString']();"), "var x = '1';");
        assert_eq!(deobfuscate("var x = (1).toString();"), "var x = '1';");
    }

    #[test]
    fn test_split_with_params() {
        assert_eq!(
            deobfuscate("var x = 'alert164t50t471t47t51'['split']('t')[0];"),
            "var x = 'aler';"
        );
        assert_eq!(
            deobfuscate("var x = 'a,b,c'.split(',', 2)[1];"),
            "var x = 'b';"
        );
    }

    #[test]
    fn test_replace() {
        // string
        assert_eq!(
            deobfuscate("var x = 'a,b,c'.replace(',', '');"),
            "var x = 'ab,c';"
        );
        assert_eq!(
            deobfuscate("var x = 'a,b,c'.replaceAll(',', '');"),
            "var x = 'abc';"
        );

        // num
        assert_eq!(
            deobfuscate("var x = '124'.replaceAll(4, 3);"),
            "var x = '123';"
        );

        // regex
        assert_eq!(
            deobfuscate("var x = 'a,b,c'.replaceAll(/,/g, '');"),
            "var x = 'abc';"
        );
        assert_eq!(
            deobfuscate("var x = 'a,b,c'.replaceAll(/,/, '');"),
            "var x = 'a,b,c'.replaceAll(/,/, '');"
        );
        assert_eq!(
            deobfuscate("var x = 'a,b,c'.replace(/,/g, '');"),
            "var x = 'abc';"
        );
        assert_eq!(
            deobfuscate("var x = 'a,b,c'.replace(/,/, '');"),
            "var x = 'ab,c';"
        );
    }

    #[test]
    fn test_dynamic_tags() {
        assert_eq!(
            deobfuscate("var x = 'minusone'.link('https://minusone.skyblue.team/');"),
            "var x = '<a href=\"https://minusone.skyblue.team/\">minusone</a>';"
        );
        assert_eq!(
            deobfuscate("var x = 'minusone'.anchor('minusone');"),
            "var x = '<a name=\"minusone\">minusone</a>';"
        );
        assert_eq!(
            deobfuscate("var x = 'minusone'.link();"),
            "var x = '<a href=\"undefined\">minusone</a>';"
        );
        assert_eq!(
            deobfuscate("var x = 'minusone'.anchor();"),
            "var x = '<a name=\"undefined\">minusone</a>';"
        );
        assert_eq!(
            deobfuscate("var x = 'minusone'['link']('https://minusone.skyblue.team/');"),
            "var x = '<a href=\"https://minusone.skyblue.team/\">minusone</a>';"
        );
        assert_eq!(
            deobfuscate("var x = 'minusone'.link('\"');"),
            "var x = '<a href=\"&quot;\">minusone</a>';"
        );
    }
}
