use crate::error::MinusOneResult;
use crate::ps::Powershell;
use crate::ps::Powershell::{Array, Raw, Type};
use crate::ps::Value::{Bool, Num, Str};
use crate::ps::tool::StringTool;
use crate::ps::utils::conversion::*;
use crate::ps::utils::string::*;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use log::trace;

#[derive(Default)]
pub struct ParseString;

impl<'a> RuleMut<'a> for ParseString {
    type Language = Powershell;

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

        match view.kind() {
            "verbatim_string_characters" => {
                let value = String::from(view.text()?);
                // Parse string by removing the double quote
                trace!(
                    "ParseString (L): Setting node with verbatim string: {:?}",
                    &value[1..value.len() - 1]
                );
                node.set(Raw(Str(
                    String::from(&value[1..value.len() - 1]).replace("''", "'")
                )));
            }
            "expandable_string_literal" => {
                // Only $.variable / $.sub_expression (and an occasional lone trailing "$")
                // show up as children; everything else -- literal text, `` `escapes ``
                // source (e.g. a `` `$foo `` escape sequence looks like a $foo reference once
                // partially processed), causing an unrelated substitution to clobber it.
                let text = view.text()?;
                let end = text.len() - 1; // exclude closing quote
                let mut result = String::new();
                let mut cursor = 1usize; // skip opening quote

                for child in view.iter() {
                    let child_start = child.start_rel();
                    let child_end = child.end_rel();

                    if child_start > cursor {
                        result.push_str(&unescape_literal_segment(&text[cursor..child_start]));
                    }

                    if child.kind() == "$" {
                        result.push_str(child.text()?);
                    } else if let Some(v) = child.data() {
                        match v {
                            Raw(Str(s)) => result.push_str(s),
                            Raw(Num(n)) => result.push_str(&n.to_string()),
                            Raw(Bool(true)) => result.push_str("True"),
                            Raw(Bool(false)) => result.push_str("False"),
                            Powershell::HashMap(_) => {
                                result.push_str("System.Collections.Hashtable")
                            }
                            _ => result.push_str(child.text()?),
                        }
                    } else {
                        // the expandable string have non inferred child
                        // so can't be inferred
                        return Ok(());
                    }

                    cursor = cursor.max(child_end);
                }

                if end > cursor {
                    result.push_str(&unescape_literal_segment(&text[cursor..end]));
                }

                trace!(
                    "ParseString (L): Setting node with expanded string: {:?}",
                    result
                );
                node.set(Raw(Str(result)));
            }
            _ => (),
        }
        Ok(())
    }
}

/// This rule will infer string concat operation
///
/// # Example
/// ```
/// use minusone::tree::{HashMapStorage, Tree};
/// use minusone::ps::build_powershell_tree;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::linter::Linter;
/// use minusone::ps::string::{ConcatString, ParseString};
///
/// let mut tree = build_powershell_tree("'foo' + 'bar'").unwrap();
/// tree.apply_mut(&mut (ParseString::default(), Forward::default(), ConcatString::default())).unwrap();
///
/// let mut ps_litter_view = Linter::default();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"foobar\"");
/// ```
#[derive(Default)]
pub struct ConcatString;

impl<'a> RuleMut<'a> for ConcatString {
    type Language = Powershell;

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
        if (view.kind() == "additive_expression" || view.kind() == "additive_argument_expression")
            && let (Some(left_op), Some(operator), Some(right_op)) =
                (view.child(0), view.child(1), view.child(2))
            && let (Some(Raw(Str(string_left))), "+", Some(Raw(Str(string_right)))) =
                (left_op.data(), operator.text()?, right_op.data())
        {
            node.reduce(Raw(Str(String::from(string_left) + string_right)))
        }
        Ok(())
    }
}

/// Centralized dispatcher for string literal builtins.
///
/// This includes:
/// - `str.ToLower()` / `str.ToLowerInvariant()`
/// - `str.ToUpper()` / `str.ToUpperInvariant()`
/// - `str.Replace(oldValue, newValue)`
/// - `str.Contains(value)`
/// - `str.IndexOf(value, [startIndex, [count]])`
/// - `str.Substring(startIndex, [length])`
/// - `str.Trim([char|char[]])`
/// - `str.ToCharArray([startIndex, length])`
///
/// Mirrors the JS `StringBuiltins` dispatcher (see `crate::js::string::StringBuiltins`).
type StringBuiltinHandler = fn(&str, &[Powershell]) -> Option<Powershell>;

const STRING_BUILTINS: &[(&str, StringBuiltinHandler)] = &[
    ("tolower", |input, _| Some(Raw(Str(input.to_lowercase())))),
    ("tolowerinvariant", |input, _| {
        Some(Raw(Str(input.to_lowercase())))
    }),
    ("toupper", |input, _| Some(Raw(Str(input.to_uppercase())))),
    ("toupperinvariant", |input, _| {
        Some(Raw(Str(input.to_uppercase())))
    }),
    ("replace", string_builtin_replace),
    ("contains", string_builtin_contains),
    ("indexof", string_builtin_index_of),
    ("substring", string_builtin_substring),
    ("trim", string_builtin_trim),
    ("tochararray", string_builtin_to_char_array),
];

fn dispatch_string_builtin(method: &str, input: &str, args: &[Powershell]) -> Option<Powershell> {
    STRING_BUILTINS
        .iter()
        .find_map(|(name, handler)| (*name == method).then(|| handler(input, args)))
        .flatten()
}

/// # Example
/// ```
/// use minusone::ps::build_powershell_tree;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::linter::Linter;
/// use minusone::ps::string::{ParseString, StringBuiltins};
///
/// let mut tree = build_powershell_tree("'HELLO'.ToLower()").unwrap();
/// tree.apply_mut(&mut (ParseString::default(), Forward::default(), StringBuiltins::default())).unwrap();
///
/// let mut ps_litter_view = Linter::default();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"hello\"");
/// ```
#[derive(Default)]
pub struct StringBuiltins;

impl<'a> RuleMut<'a> for StringBuiltins {
    type Language = Powershell;

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

        let (expression, operator, member_name, args_node, args_node_is_stray) =
            if view.kind() == "invokation_expression" {
                let (Some(expression), Some(operator), Some(member_name), Some(arguments_list)) =
                    (view.child(0), view.child(1), view.child(2), view.child(3))
                else {
                    return Ok(());
                };
                (expression, operator, member_name, arguments_list, false)
            } else if view.kind() == "array_literal_expression" && view.child_count() == 1 {
                let Some(first) = view.child(0) else {
                    return Ok(());
                };
                let member_access = match first.kind() {
                    "member_access" => first,
                    _ => match first.child(0) {
                        Some(c) if c.kind() == "member_access" => c,
                        _ => return Ok(()),
                    },
                };

                let Some(parent) = view.parent() else {
                    return Ok(());
                };
                if parent.kind() != "command_elements" {
                    return Ok(());
                }

                // find the argument_list immediately following this node in the parent
                let mut found_self = false;
                let mut next_arg_list = None;
                for sibling in parent.iter() {
                    if found_self {
                        if sibling.kind() == "argument_list" {
                            next_arg_list = Some(sibling);
                        }
                        break;
                    }
                    if sibling.id() == view.id() {
                        found_self = true;
                    }
                }
                let Some(arguments_list) = next_arg_list else {
                    return Ok(());
                };

                let (Some(expression), Some(operator), Some(member_name)) = (
                    member_access.child(0),
                    member_access.child(1),
                    member_access.child(2),
                ) else {
                    return Ok(());
                };
                (expression, operator, member_name, arguments_list, true)
            } else {
                return Ok(());
            };

        if operator.text()? != "." {
            return Ok(());
        }

        let Some(Raw(Str(input))) = expression.data() else {
            return Ok(());
        };

        let method = match member_name.data() {
            Some(Raw(Str(m))) => m.clone().normalize(),
            _ => member_name.text()?.to_string().normalize(),
        };

        let positional_args = get_positional_arguments(&args_node);
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
            "StringBuiltins (L): reducing '{}'.{}() to {:?}",
            input, method, result
        );

        if args_node_is_stray {
            node.set_by_node_id(args_node.id(), Powershell::DeadCode);
        }
        node.reduce(result);
        Ok(())
    }
}

fn get_positional_arguments<'a>(
    args_node: &crate::tree::Node<'a, Powershell>,
) -> Vec<crate::tree::Node<'a, Powershell>> {
    match args_node.named_child("argument_expression_list") {
        None => vec![],
        Some(list) => list.iter().filter(|c| c.kind() != ",").collect(),
    }
}

fn string_builtin_replace(input: &str, args: &[Powershell]) -> Option<Powershell> {
    let (Some(Raw(Str(from))), Some(Raw(to))) = (args.first(), args.get(1)) else {
        return None;
    };
    Some(Raw(Str(input.replace(from, &to.to_string()))))
}

fn string_builtin_contains(input: &str, args: &[Powershell]) -> Option<Powershell> {
    let Some(Raw(Str(needle))) = args.first() else {
        return None;
    };
    Some(Raw(Bool(input.contains(needle.as_str()))))
}

fn find_char_index(haystack: &[char], needle: &[char]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    if needle.len() > haystack.len() {
        return None;
    }
    (0..=haystack.len() - needle.len()).find(|&i| haystack[i..i + needle.len()] == *needle)
}

fn string_builtin_index_of(input: &str, args: &[Powershell]) -> Option<Powershell> {
    let Some(Raw(Str(needle))) = args.first() else {
        return None;
    };
    let haystack: Vec<char> = input.chars().collect();
    let needle: Vec<char> = needle.chars().collect();
    let len = haystack.len();

    let start = match args.get(1) {
        None => 0,
        Some(Raw(Num(n))) if *n >= 0 && (*n as usize) <= len => *n as usize,
        _ => return None,
    };

    let end = match args.get(2) {
        None => len,
        Some(Raw(Num(n))) if *n >= 0 && start + (*n as usize) <= len => start + (*n as usize),
        _ => return None,
    };

    let result = find_char_index(&haystack[start..end], &needle)
        .map(|i| (start + i) as i64)
        .unwrap_or(-1);
    Some(Raw(Num(result)))
}

fn string_builtin_substring(input: &str, args: &[Powershell]) -> Option<Powershell> {
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();

    let start = match args.first() {
        Some(Raw(Num(n))) if *n >= 0 && (*n as usize) <= len => *n as usize,
        _ => return None,
    };

    let end = match args.get(1) {
        None => len,
        Some(Raw(Num(n))) if *n >= 0 && start + (*n as usize) <= len => start + (*n as usize),
        _ => return None,
    };

    Some(Raw(Str(chars[start..end].iter().collect())))
}

fn string_builtin_trim(input: &str, args: &[Powershell]) -> Option<Powershell> {
    match args.first() {
        None => Some(Raw(Str(input.trim().to_string()))),
        Some(Raw(Str(c))) if c.chars().count() == 1 => {
            let ch = c.chars().next()?;
            Some(Raw(Str(input.trim_matches(ch).to_string())))
        }
        Some(Array(values)) => {
            let chars = to_chars(values)?;
            Some(Raw(Str(input
                .trim_matches(|c: char| chars.contains(&c))
                .to_string())))
        }
        _ => None,
    }
}

fn string_builtin_to_char_array(input: &str, args: &[Powershell]) -> Option<Powershell> {
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();

    let (start, end) = match (args.first(), args.get(1)) {
        (None, None) => (0, len),
        (Some(Raw(Num(s))), Some(Raw(Num(l))))
            if *s >= 0
                && (*s as usize) <= len
                && *l >= 0
                && (*s as usize) + (*l as usize) <= len =>
        {
            (*s as usize, (*s as usize) + (*l as usize))
        }
        _ => return None,
    };

    Some(Array(
        chars[start..end]
            .iter()
            .map(|c| Str(c.to_string()))
            .collect(),
    ))
}

#[derive(Default)]
pub struct StringReplaceOp;

impl<'a> RuleMut<'a> for StringReplaceOp {
    type Language = Powershell;

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
        if view.kind() == "comparison_expression"
            && let (Some(left_expression), Some(operator), Some(right_expression)) =
                (view.child(0), view.child(1), view.child(2))
        {
            match (
                left_expression.data(),
                operator.text()?.to_lowercase().as_str(),
                right_expression.data(),
            ) {
                (Some(Raw(Str(src))), "-replace", Some(Array(params)))
                | (Some(Raw(Str(src))), "-creplace", Some(Array(params))) => {
                    // -replace operator need two params
                    if let (Some(Str(old)), Some(Str(new))) = (params.first(), params.get(1)) {
                        node.reduce(Raw(Str(src.replace(old, new))));
                    }
                }
                _ => (),
            }
        }
        Ok(())
    }
}

/// This rule will infer format operator
///
/// # Example
/// ```
/// use minusone::tree::{HashMapStorage, Tree};
/// use minusone::ps::build_powershell_tree;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::linter::Linter;
/// use minusone::ps::string::{ParseString, FormatString};
/// use minusone::ps::array::ParseArrayLiteral;
///
/// let mut tree = build_powershell_tree("\"{1} {0}\" -f 'world', 'hello'").unwrap();
/// tree.apply_mut(&mut (
///     ParseString::default(),
///     Forward::default(),
///     FormatString::default(),
///     ParseArrayLiteral::default())
/// ).unwrap();
///
/// let mut ps_litter_view = Linter::default();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"hello world\"");
/// ```
#[derive(Default)]
pub struct FormatString;

impl<'a> RuleMut<'a> for FormatString {
    type Language = Powershell;

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
        if view.kind() == "format_expression"
            && let (Some(format_str_node), Some(format_args_node)) = (view.child(0), view.child(2))
        {
            match (format_str_node.data(), format_args_node.data()) {
                (Some(Raw(Str(format_str))), Some(Array(format_args))) => {
                    let mut result = format_str.clone();
                    for (index, new) in format_args.iter().enumerate() {
                        result = result
                            .replace(format!("{{{index}}}").as_str(), new.to_string().as_str());
                    }
                    node.reduce(Raw(Str(result)));
                }
                (Some(Raw(Str(format_str))), Some(Raw(format_arg))) => {
                    node.reduce(Raw(Str(
                        format_str.replace("{0}", format_arg.to_string().as_str())
                    )));
                }
                _ => (),
            }
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct StringSplitMethod;

impl<'a> RuleMut<'a> for StringSplitMethod {
    type Language = Powershell;

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
        if view.kind() == "invokation_expression"
            && let (Some(expression), Some(operator), Some(member_name), Some(arguments_list)) =
                (view.child(0), view.child(1), view.child(2), view.child(3))
        {
            match (
                expression.data(),
                operator.text()?,
                &member_name.text()?.to_string(),
                member_name.data(),
            ) {
                (Some(Raw(Str(src))), ".", m, _)
                | (Some(Raw(Str(src))), ".", _, Some(Raw(Str(m))))
                    if m.clone().to_lowercase().remove_tilt().remove_quote() == "split" =>
                {
                    if let Some(argument_expression_list) =
                        arguments_list.named_child("argument_expression_list")
                        && let Some(arg_1) = argument_expression_list.child(0)
                        && let Some(Raw(Str(separator))) = arg_1.data()
                    {
                        // not reduce to have a better deobfuscation
                        // if we reduce this step we will maybe lost the string
                        let array = src
                            .split(separator)
                            .collect::<Vec<&str>>()
                            .iter()
                            .map(|e| Str(e.to_string()))
                            .collect();
                        trace!(
                            "StringSplitMethod (L): Setting node with split string: {:?} with separator: {:?}",
                            array, separator
                        );
                        node.set(Array(array));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// This rule will infer the [System.String]::new(...) constructor, mirroring the
/// public `System.String` constructor overloads reachable from PowerShell source:
///
/// - `string(char[] value)` -> [System.string]::new(@(72, 101, 108, 108, 111)) => "Hello"
/// - `string(char[] value, int startIndex, int length)`
/// - `string(char c, int count)` -> repeats a char
///
/// The pointer-based overloads (`char*`, `sbyte*`, `sbyte*, int, int`, `sbyte*, int, int, Encoding`)
/// are not reachable from safe PowerShell script text, so they are not handled.
#[derive(Default)]
pub struct NewStringMethod;

impl<'a> RuleMut<'a> for NewStringMethod {
    type Language = Powershell;

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
        if view.kind() == "invokation_expression"
            && let (
                Some(primary_expression),
                Some(operator),
                Some(member_name),
                Some(arguments_list),
            ) = (view.child(0), view.child(1), view.child(2), view.child(3))
        {
            match (
                primary_expression.data(),
                operator.text()?,
                &member_name.text()?.to_string(),
                member_name.data(),
            ) {
                (Some(Type(typename)), "::", m, _)
                | (Some(Type(typename)), "::", _, Some(Raw(Str(m))))
                    if m.clone().normalize() == "new"
                        && (typename == "system.string" || typename == "string") =>
                {
                    if let Some(argument_expression_list) =
                        arguments_list.named_child("argument_expression_list")
                    {
                        let arg_1 = argument_expression_list.child(0);
                        let arg_2 = argument_expression_list.child(2);
                        let arg_3 = argument_expression_list.child(4);

                        match (arg_1, arg_2, arg_3) {
                            // string(char[] value), PowerShell also implicitly coerces a plain string argument into a char[] to match this overload
                            (Some(a1), None, None) => {
                                if let Some(Array(values)) = a1.data()
                                    && let Some(chars) = to_chars(values)
                                {
                                    let result: String = chars.into_iter().collect();
                                    trace!(
                                        "NewStringMethod (L): Setting node with result: {:?}",
                                        result
                                    );
                                    node.set(Raw(Str(result)));
                                } else if let Some(Raw(Str(s))) = a1.data() {
                                    trace!(
                                        "NewStringMethod (L): Setting node with coerced string result: {:?}",
                                        s
                                    );
                                    node.set(Raw(Str(s.clone())));
                                }
                            }
                            // string(char c, int count)
                            (Some(a1), Some(a2), None) => {
                                if let (Some(Raw(c)), Some(Raw(Num(count)))) =
                                    (a1.data(), a2.data())
                                    && let Some(c) = to_char(c)
                                    && *count >= 0
                                {
                                    let result: String =
                                        std::iter::repeat(c).take(*count as usize).collect();
                                    trace!(
                                        "NewStringMethod (L): Setting node with repeated char result: {:?}",
                                        result
                                    );
                                    node.set(Raw(Str(result)));
                                }
                            }
                            // string(char[] value, int startIndex, int length)
                            (Some(a1), Some(a2), Some(a3)) => {
                                if let (
                                    Some(Array(values)),
                                    Some(Raw(Num(start))),
                                    Some(Raw(Num(length))),
                                ) = (a1.data(), a2.data(), a3.data())
                                    && let Some(chars) = to_chars(values)
                                    && *start >= 0
                                    && *length >= 0
                                {
                                    let start = *start as usize;
                                    let length = *length as usize;
                                    if let Some(slice) = chars.get(start..start + length) {
                                        let result: String = slice.iter().collect();
                                        trace!(
                                            "NewStringMethod (L): Setting node with sliced result: {:?}",
                                            result
                                        );
                                        node.set(Raw(Str(result)));
                                    }
                                }
                            }
                            _ => (),
                        }
                    }
                }
                _ => (),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::ps::Powershell::Raw;
    use crate::ps::Value::Str;
    use crate::ps::array::{ComputeArrayExpr, ParseArrayLiteral};
    use crate::ps::build_powershell_tree;
    use crate::ps::forward::Forward;
    use crate::ps::integer::ParseInt;
    use crate::ps::join::JoinComparison;
    use crate::ps::linter::Linter;
    use crate::ps::string::*;
    use crate::ps::typing::ParseType;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_powershell_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            ParseInt::default(),
            Forward::default(),
            StringBuiltins::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_concat_two_elements() {
        let mut tree = build_powershell_tree("'a' + 'b'").unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            Forward::default(),
            ConcatString::default(),
        ))
        .unwrap();
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data()
                .expect("Inferred type"),
            Raw(Str("ab".to_string()))
        );
    }

    #[test]
    fn test_backtick_escape_sequences() {
        let mut tree = build_powershell_tree("\"a`nb`tc`0d`ae`bf`ff`vg`\"h`` i\"").unwrap();
        tree.apply_mut(&mut (ParseString::default(), Forward::default()))
            .unwrap();
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data()
                .expect("Inferred type"),
            Raw(Str("a\nb\tc\0d\u{7}e\u{8}f\u{c}f\u{b}g\"h` i".to_string()))
        );
    }

    #[test]
    fn test_backtick_unknown_escape_drops_backtick() {
        let mut tree = build_powershell_tree("\"a`zb\"").unwrap();
        tree.apply_mut(&mut (ParseString::default(), Forward::default()))
            .unwrap();
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data()
                .expect("Inferred type"),
            Raw(Str("azb".to_string()))
        );
    }

    #[test]
    fn test_backtick_escaped_dollar_not_expanded() {
        let mut tree = build_powershell_tree("$x = \"world\"\n\"hello `$x = $x\"").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseString::default(),
                Forward::default(),
                crate::ps::var::Var::default(),
            ),
            crate::ps::strategy::PowershellStrategy::default(),
        )
        .unwrap();

        assert_eq!(
            *tree
                .root()
                .unwrap() // program
                .child(0)
                .unwrap() // statement_list
                .child(1)
                .unwrap() // second statement (the expandable string)
                .data()
                .expect("Inferred type"),
            Raw(Str("hello $x = world".to_string()))
        );
    }

    #[test]
    fn test_no_collision_when_substituted_value_looks_like_another_reference() {
        let mut tree =
            build_powershell_tree("$y = \"resolved\"\n$x = \"literal `$y text\"\n\"a=$x b=$y\"")
                .unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseString::default(),
                Forward::default(),
                crate::ps::var::Var::default(),
            ),
            crate::ps::strategy::PowershellStrategy::default(),
        )
        .unwrap();

        assert_eq!(
            *tree
                .root()
                .unwrap() // program
                .child(0)
                .unwrap() // statement_list
                .child(2)
                .unwrap() // third statement (the expandable string)
                .data()
                .expect("Inferred type"),
            Raw(Str("a=literal $y text b=resolved".to_string()))
        );
    }

    #[test]
    fn test_verbatim_string_ignores_backtick() {
        let mut tree = build_powershell_tree("'a`nb'").unwrap();
        tree.apply_mut(&mut (ParseString::default(), Forward::default()))
            .unwrap();
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data()
                .expect("Inferred type"),
            Raw(Str("a`nb".to_string()))
        );
    }

    #[test]
    fn test_infer_subexpression_elements() {
        let mut tree = build_powershell_tree("\"foo$(\"b\"+\"a\"+\"r\")\"").unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            Forward::default(),
            ConcatString::default(),
        ))
        .unwrap();
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data()
                .expect("Inferred type"),
            Raw(Str("foobar".to_string()))
        );
    }

    #[test]
    fn test_replace_operator() {
        let mut tree =
            build_powershell_tree("\"hello world\" -replace \"world\", \"toto\"").unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            Forward::default(),
            StringReplaceOp::default(),
            ParseArrayLiteral::default(),
        ))
        .unwrap();
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data()
                .expect("Inferred type"),
            Raw(Str("hello toto".to_string()))
        );
    }

    #[test]
    fn test_new_string_from_char_codes() {
        let mut tree =
            build_powershell_tree("[System.String]::new(@(72, 101, 108, 108, 111))").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ParseArrayLiteral::default(),
            ComputeArrayExpr::default(),
            ParseType::default(),
            NewStringMethod::default(),
        ))
        .unwrap();

        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data()
                .expect("Inferred type"),
            Raw(Str("Hello".to_string()))
        );
    }

    #[test]
    fn test_new_string_short_alias() {
        let mut tree = build_powershell_tree("[string]::new(@(72, 105))").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ParseArrayLiteral::default(),
            ComputeArrayExpr::default(),
            ParseType::default(),
            NewStringMethod::default(),
        ))
        .unwrap();

        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data()
                .expect("Inferred type"),
            Raw(Str("Hi".to_string()))
        );
    }

    #[test]
    fn test_new_string_repeat_char() {
        let mut tree = build_powershell_tree("[System.String]::new([char]65, 5)").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            crate::ps::cast::Cast::default(),
            ParseArrayLiteral::default(),
            ComputeArrayExpr::default(),
            ParseType::default(),
            NewStringMethod::default(),
        ))
        .unwrap();

        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data()
                .expect("Inferred type"),
            Raw(Str("AAAAA".to_string()))
        );
    }

    #[test]
    fn test_new_string_slice() {
        let mut tree =
            build_powershell_tree("[System.String]::new(@(72, 101, 108, 108, 111), 1, 3)").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ParseArrayLiteral::default(),
            ComputeArrayExpr::default(),
            ParseType::default(),
            NewStringMethod::default(),
        ))
        .unwrap();

        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data()
                .expect("Inferred type"),
            Raw(Str("ell".to_string()))
        );
    }

    #[test]
    fn test_new_string_from_plain_string() {
        // Real PowerShell coerces a plain string argument into a char[] to match
        // the string(char[]) overload, so [System.String]::new("test") => "test"
        let mut tree = build_powershell_tree("[System.String]::new(\"test string\")").unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            Forward::default(),
            ParseType::default(),
            NewStringMethod::default(),
        ))
        .unwrap();

        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data()
                .expect("Inferred type"),
            Raw(Str("test string".to_string()))
        );
    }

    #[test]
    fn test_to_lower() {
        assert_eq!(deobfuscate("'HeLLo'.ToLower()"), "\"hello\"");
    }

    #[test]
    fn test_to_upper() {
        assert_eq!(deobfuscate("'HeLLo'.ToUpper()"), "\"HELLO\"");
    }

    #[test]
    fn test_to_upper_dynamic_member_name() {
        assert_eq!(deobfuscate("'hi'.'ToUpper'()"), "\"HI\"");
    }

    #[test]
    fn test_replace_method() {
        assert_eq!(deobfuscate("'foo'.Replace('oo', 'aa')"), "\"faa\"");
    }

    #[test]
    fn test_replace_method_non_string_replacement() {
        assert_eq!(deobfuscate("'a1b1c'.Replace('1', 2)"), "\"a2b2c\"");
    }

    #[test]
    fn test_touppertolower_as_command_argument() {
        assert_eq!(
            deobfuscate("Write-Host \"aBcDe\".ToLower()"),
            "Write-Host \"abcde\""
        );
    }

    #[test]
    fn test_contains() {
        assert_eq!(deobfuscate("'hello'.Contains('ell')"), "$true");
        assert_eq!(deobfuscate("'hello'.Contains('xyz')"), "$false");
        assert_eq!(deobfuscate("'hello'.Contains('')"), "$true");
    }

    #[test]
    fn test_index_of() {
        assert_eq!(deobfuscate("'hello'.IndexOf('l')"), "2");
        assert_eq!(deobfuscate("'hello'.IndexOf('z')"), "-1");
        assert_eq!(deobfuscate("'hello'.IndexOf('')"), "0");
        assert_eq!(deobfuscate("'hello'.IndexOf('l', 3)"), "3");
        assert_eq!(deobfuscate("'hello'.IndexOf('l', 0, 2)"), "-1");
        assert_eq!(deobfuscate("'hello'.IndexOf('l', 2, 1)"), "2");
        assert_eq!(deobfuscate("'hello'.IndexOf('l', 3, 0)"), "-1");
        assert_eq!(
            deobfuscate("'hello'.IndexOf('l', 10)"),
            "\"hello\".IndexOf(\"l\", 10)"
        );
    }

    #[test]
    fn test_substring() {
        assert_eq!(deobfuscate("'hello'.Substring(1)"), "\"ello\"");
        assert_eq!(deobfuscate("'hello'.Substring(1, 3)"), "\"ell\"");
        assert_eq!(deobfuscate("'hello'.Substring(5)"), "\"\"");
        assert_eq!(deobfuscate("'hello'.Substring(5, 0)"), "\"\"");
        assert_eq!(
            deobfuscate("'hello'.Substring(10)"),
            "\"hello\".Substring(10)"
        );
        assert_eq!(
            deobfuscate("'hello'.Substring(1, 100)"),
            "\"hello\".Substring(1, 100)"
        );
    }

    #[test]
    fn test_trim() {
        assert_eq!(deobfuscate("'  hello  '.Trim()"), "\"hello\"");
        assert_eq!(deobfuscate("'xxhelloxx'.Trim('x')"), "\"hello\"");
        assert_eq!(deobfuscate("'aaa'.Trim('a')"), "\"\"");
        assert_eq!(deobfuscate("''.Trim()"), "\"\"");
    }

    #[test]
    fn test_trim_char_array() {
        let mut tree = build_powershell_tree("'xyhelloyx'.Trim(@('x', 'y'))").unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            Forward::default(),
            ParseArrayLiteral::default(),
            ComputeArrayExpr::default(),
            StringBuiltins::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        assert_eq!(linter.output, "\"hello\"");
    }

    #[test]
    fn test_to_char_array() {
        let mut tree = build_powershell_tree("'hello'.ToCharArray() -join ','").unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            Forward::default(),
            StringBuiltins::default(),
            JoinComparison::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        assert_eq!(linter.output, "\"h,e,l,l,o\"");

        let mut tree = build_powershell_tree("'hello'.ToCharArray(1, 2) -join ','").unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            ParseInt::default(),
            Forward::default(),
            StringBuiltins::default(),
            JoinComparison::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        assert_eq!(linter.output, "\"e,l\"");
    }

    #[test]
    fn test_format_operator() {
        let mut tree = build_powershell_tree("\"{1} {0}\" -f \"world\", \"hello\"").unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            Forward::default(),
            FormatString::default(),
            ParseArrayLiteral::default(),
        ))
        .unwrap();
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data()
                .expect("Inferred type"),
            Raw(Str("hello world".to_string()))
        );
    }
}
