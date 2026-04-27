use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::{Array, Null, Raw, Regex, Undefined};
use crate::js::Value::{Bool, Num, Str};
use crate::js::string::Concat;
use crate::js::utils::{get_positional_arguments, method_name};
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, Node, NodeMut};
use log::trace;
use regex::RegexBuilder;
use std::collections::HashSet;

/// Parses JavaScript regular expressions from `/pattern/flags` or RegExp constructor into `Regex { .. }`
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::regex::ParseRegex;
/// use minusone::js::string::ParseString;
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var r = RegExp('ab+', 'i');").unwrap();
/// tree.apply_mut(&mut (ParseString::default(), ParseRegex::default())).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "var r = /ab+/i;");
/// ```
#[derive(Default)]
pub struct ParseRegex;

impl ParseRegex {
    fn parse_regex_literal(raw: &str) -> Option<(String, String)> {
        if !raw.starts_with('/') {
            return None;
        }

        let mut escaped = false;
        let mut end = None;
        for (idx, ch) in raw.char_indices().skip(1) {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == '/' {
                end = Some(idx);
            }
        }

        let end = end?;
        let pattern = raw[1..end].to_string();
        let flags = Self::normalize_flags(&raw[end + 1..])?;
        Some((pattern, flags))
    }

    fn normalize_flags(flags: &str) -> Option<String> {
        let mut seen = HashSet::new();
        for ch in flags.chars() {
            if !matches!(ch, 'd' | 'g' | 'i' | 'm' | 's' | 'u' | 'v' | 'y') {
                return None;
            }
            if !seen.insert(ch) {
                return None;
            }
        }
        Some(flags.to_string())
    }

    fn parse_constructor_call(
        callee: Option<Node<JavaScript>>,
        args: Option<Node<JavaScript>>,
    ) -> Option<JavaScript> {
        let callee = callee?;
        if callee.text().ok()? != "RegExp" {
            return None;
        }

        let positional = get_positional_arguments(args);

        let first = positional.first().and_then(|n| n.data());
        let second = positional.get(1).and_then(|n| n.data());

        let (pattern, inherited_flags) = match first {
            None | Some(Undefined) => (String::new(), String::new()),
            Some(Raw(Str(s))) => (s.clone(), String::new()),
            Some(Regex { pattern, flags }) => {
                if second.is_some() {
                    return None;
                }
                (pattern.clone(), flags.clone())
            }
            Some(Raw(Num(n))) => (n.to_string(), String::new()),
            Some(Raw(Bool(b))) => (b.to_string(), String::new()),
            Some(Null) => ("null".to_string(), String::new()),
            _ => return None,
        };

        let flags = match second {
            Some(Undefined) | None => inherited_flags,
            Some(Raw(Str(flags))) => Self::normalize_flags(flags)?,
            _ => return None,
        };

        Some(Regex { pattern, flags })
    }
}

impl<'a> RuleMut<'a> for ParseRegex {
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
        match view.kind() {
            "regex" => {
                if let Ok(text) = view.text() {
                    if let Some((pattern, flags)) = Self::parse_regex_literal(text) {
                        trace!("ParseRegex (L): /{}/{}", pattern, flags);
                        node.reduce(Regex { pattern, flags });
                    }
                }
            }
            "call_expression" => {
                let callee = view.named_child("function").or_else(|| view.child(0));
                let args = view.named_child("arguments").or_else(|| view.child(1));
                if let Some(regex) = Self::parse_constructor_call(callee, args) {
                    trace!("ParseRegex (L): RegExp constructor => {}", regex);
                    node.reduce(regex);
                }
            }
            "new_expression" => {
                let callee = view.named_child("constructor").or_else(|| view.child(1));
                let args = view.named_child("arguments").or_else(|| view.child(2));
                if let Some(regex) = Self::parse_constructor_call(callee, args) {
                    trace!("ParseRegex (L): new RegExp constructor => {}", regex);
                    node.reduce(regex);
                }
            }
            _ => {}
        }

        Ok(())
    }
}

/// Executes regex calls `exec()` and `test()` on known inputs.
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::regex::{ParseRegex, RegexExec};
/// use minusone::js::string::ParseString;
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var a = /ab+/.test('zabbbz');").unwrap();
/// tree.apply_mut(&mut (ParseString::default(), ParseRegex::default(), RegexExec::default())).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "var a = true;");
/// ```
#[derive(Default)]
pub struct RegexExec;

impl RegexExec {
    pub fn compile(pattern: &str, flags: &str) -> Option<regex::Regex> {
        let mut builder = RegexBuilder::new(pattern);
        for flag in flags.chars() {
            match flag {
                'i' => {
                    builder.case_insensitive(true);
                }
                'm' => {
                    builder.multi_line(true);
                }
                's' => {
                    builder.dot_matches_new_line(true);
                }
                'u' => {
                    builder.unicode(true);
                }
                'd' | 'g' | 'v' | 'y' => {}
                _ => return None,
            }
        }

        builder.build().ok()
    }
}

impl<'a> RuleMut<'a> for RegexExec {
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
        if method != "test" && method != "exec" {
            return Ok(());
        }

        let Some(object) = callee.named_child("object") else {
            return Ok(());
        };
        let Some(Regex { pattern, flags }) = object.data() else {
            return Ok(());
        };

        let args = get_positional_arguments(view.named_child("arguments"));
        let input = match args.first().and_then(|a| a.data()) {
            Some(v) => v.to_string(),
            None => "undefined".to_string(),
        };

        let Some(regex) = Self::compile(pattern, flags) else {
            return Ok(());
        };

        if method == "test" {
            let result = regex.is_match(&input);
            trace!(
                "RegexExec (L): /{}/{}.test({:?}) => {}",
                pattern, flags, input, result
            );
            node.reduce(Raw(Bool(result)));
            return Ok(());
        }

        if let Some(captures) = regex.captures(&input) {
            let mut groups = vec![];
            for group in captures.iter() {
                match group {
                    Some(group) => groups.push(Raw(Str(group.as_str().to_string()))),
                    None => groups.push(Undefined),
                }
            }
            trace!(
                "RegexExec (L): /{}/{}.exec({:?}) => {} groups",
                pattern,
                flags,
                input,
                groups.len()
            );
            node.reduce(Array(groups));
        } else {
            trace!(
                "RegexExec (L): /{}/{}.exec({:?}) => null",
                pattern, flags, input
            );
            node.reduce(Null);
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
pub struct RegexConcat;

impl<'a> RuleMut<'a> for RegexConcat {
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
        flow: ControlFlow,
    ) -> MinusOneResult<()> {
        let pending = {
            let view = node.view();
            if view.kind() != "binary_expression" {
                return Ok(());
            }

            match (view.child(0), view.child(1), view.child(2)) {
                (Some(left), Some(operator), Some(right)) if operator.text()? == "+" => {
                    let left_update = match left.data() {
                        Some(regex @ Regex { .. }) => Some((left.id(), regex.to_string())),
                        _ => None,
                    };
                    let right_update = match right.data() {
                        Some(regex @ Regex { .. }) => Some((right.id(), regex.to_string())),
                        _ => None,
                    };
                    (left_update, right_update)
                }
                _ => return Ok(()),
            }
        };

        let mut changed = false;
        if let Some((id, s)) = pending.0 {
            node.set_by_node_id(id, Raw(Str(s)));
            changed = true;
        }
        if let Some((id, s)) = pending.1 {
            node.set_by_node_id(id, Raw(Str(s)));
            changed = true;
        }

        if changed {
            Concat::default().leave(node, flow)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests_js_regex {
    use super::*;
    use crate::js::build_javascript_tree;
    use crate::js::integer::ParseInt;
    use crate::js::linter::Linter;
    use crate::js::objects::object::ObjectField;
    use crate::js::string::{Concat, ParseString};

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            ParseRegex::default(),
            ParseInt::default(),
            Concat::default(),
            RegexConcat::default(),
            ObjectField::default(),
            RegexExec::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_parse_regex_literal() {
        assert_eq!(deobfuscate("var r = /ab+/gi;"), "var r = /ab+/gi;");
    }

    #[test]
    fn test_parse_regex_constructor() {
        assert_eq!(
            deobfuscate("var r = RegExp('ab+', 'i');"),
            "var r = /ab+/i;"
        );
        assert_eq!(
            deobfuscate("var r = new RegExp('a+', 'm');"),
            "var r = /a+/m;"
        );
    }

    #[test]
    fn test_regex_test_and_exec() {
        assert_eq!(
            deobfuscate("var a = /ab+/.test('zabbbz');"),
            "var a = true;"
        );
        assert_eq!(deobfuscate("var a = /ab+/.test('zzz');"), "var a = false;");
        assert_eq!(
            deobfuscate("var m = /a(b+)/.exec('zabbbz');"),
            "var m = ['abbb', 'bbb'];"
        );
        assert_eq!(deobfuscate("var m = /a+/.exec('zzz');"), "var m = null;");
    }

    #[test]
    fn test_regexp_concat() {
        assert_eq!(
            deobfuscate("var m = RegExp + '';"),
            "var m = 'function RegExp() { [native code] }';"
        );
    }

    #[test]
    fn test_regex_concat() {
        assert_eq!(deobfuscate("var m = /a/ + 'a';"), "var m = '/a/a';");
        assert_eq!(deobfuscate("var m = /a/ + 1;"), "var m = '/a/1';");
        assert_eq!(deobfuscate("var m = /a/g + /a/i;"), "var m = '/a/g/a/i';");
    }
}
