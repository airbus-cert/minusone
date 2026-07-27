use crate::error::MinusOneResult;
use crate::ps::Powershell;
use crate::ps::Powershell::{Array, Raw, Type};
use crate::ps::Value::{Bool, Num, Str};
use crate::ps::tool::StringTool;
use crate::ps::utils::conversion::*;
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
                // expand what is expandable
                let value = String::from(view.text()?);
                // Parse string by removing the double quote
                let mut result = String::from(&value[1..value.len() - 1]).replace("\"\"", "\"");

                for child in view.iter() {
                    // relicate token from tree-sitter
                    if child.kind() == "$" {
                        continue;
                    }

                    if let Some(v) = child.data() {
                        match v {
                            Raw(Str(s)) => {
                                result = result.replace(child.text()?, s);
                            }
                            Raw(Num(n)) => {
                                result = result.replace(child.text()?, n.to_string().as_str());
                            }
                            Raw(Bool(true)) => {
                                result = result.replace(child.text()?, "True");
                            }
                            Raw(Bool(false)) => {
                                result = result.replace(child.text()?, "False");
                            }
                            Powershell::HashMap(_) => {
                                result =
                                    result.replace(child.text()?, "System.Collections.Hashtable");
                            }
                            _ => (),
                        }
                    } else {
                        // the expandable string have non inferred child
                        // so can't be inferred
                        return Ok(());
                    }
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

#[derive(Default)]
pub struct StringReplaceMethod;

impl<'a> RuleMut<'a> for StringReplaceMethod {
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
                    if m.clone().to_lowercase().remove_tilt().remove_quote() == "replace" =>
                {
                    if let Some(argument_expression_list) =
                        arguments_list.named_child("argument_expression_list")
                        && let (Some(arg_1), Some(arg_2)) = (
                            argument_expression_list.child(0),
                            argument_expression_list.child(2),
                        )
                        && let (Some(Raw(Str(from))), Some(Raw(to))) = (arg_1.data(), arg_2.data())
                    {
                        node.reduce(Raw(Str(src.replace(from, &to.to_string()))));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
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
    use crate::ps::string::{
        ConcatString, FormatString, NewStringMethod, ParseString, StringReplaceOp,
    };
    use crate::ps::typing::ParseType;

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
