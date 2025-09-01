use base64::{engine::general_purpose, Engine as _};
use crate::error::MinusOneResult;
use crate::ps::Powershell;
use crate::ps::Powershell::{Array, Raw, Type};
use crate::ps::tool::StringTool;
use crate::ps::Value::{Num, Str};
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};

/// Compute the length of predictable Array or string
///
/// # Example
/// ```
/// use minusone::ps::build_powershell_tree;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::linter::Linter;
/// use minusone::ps::string::ParseString;
/// use minusone::ps::method::Length;
///
/// let mut tree = build_powershell_tree("'foo'.length").unwrap();
/// tree.apply_mut(&mut (
///     Length::default(),
///     Forward::default(),
///     ParseString::default()
///     )
/// ).unwrap();
///
/// let mut ps_litter_view = Linter::new();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "3");
/// ```
#[derive(Default)]
pub struct Length;

impl<'a> RuleMut<'a> for Length {
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
        if view.kind() == "member_access" {
            if let (Some(primary_expression), Some(operator), Some(member_name)) =
                (view.child(0), view.child(1), view.child(2))
            {
                match (
                    primary_expression.data(),
                    operator.text()?,
                    &member_name.text()?.to_string(),
                    member_name.data(),
                ) {
                    (Some(Array(value)), ".", m, _)
                    | (Some(Array(value)), ".", _, Some(Raw(Str(m))))
                        if m.clone().normalize() == "length" =>
                    {
                        node.set(Raw(Num(value.len() as i64)))
                    }
                    (Some(Raw(Str(s))), ".", m, None)
                    | (Some(Raw(Str(s))), ".", _, Some(Raw(Str(m))))
                        if m.clone().normalize() == "length" =>
                    {
                        node.set(Raw(Num(s.len() as i64)))
                    }
                    _ => (),
                }
            }
        }
        Ok(())
    }
}

/// This rule will infer the [System.Convert]::FromBase64String function
///
/// [System.Text.Encoding]::utf8.getstring([System.Convert]::FromBase64String('Zm9v')) => 'foo'
///
/// # Example
/// ```
/// extern crate tree_sitter;
/// extern crate tree_sitter_powershell;
///
/// use minusone::tree::{HashMapStorage, Tree};
/// use minusone::ps::build_powershell_tree;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::linter::Linter;
/// use minusone::ps::string::ParseString;
/// use minusone::ps::typing::ParseType;
/// use minusone::ps::method::{DecodeBase64, FromUTF};
///
/// let mut tree = build_powershell_tree("[System.Text.Encoding]::utf8.getstring([System.Convert]::FromBase64String('Zm9v'))").unwrap();
/// tree.apply_mut(&mut (
///     ParseString::default(),
///     Forward::default(),
///     ParseType::default(),
///     DecodeBase64::default(),
///     FromUTF::default()
/// )).unwrap();
///
/// let mut ps_litter_view = Linter::new();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"foo\"");
/// ```
#[derive(Default)]
pub struct DecodeBase64;

impl<'a> RuleMut<'a> for DecodeBase64 {
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

        // infer type of function pointer
        if view.kind() == "member_access" {
            if let (Some(type_lit), Some(op), Some(member_name)) =
                (view.child(0), view.child(1), view.child(2))
            {
                match (
                    type_lit.data(),
                    op.text()?,
                    &member_name.text()?.to_string(),
                    member_name.data(),
                ) {
                    (Some(Type(typename)), "::", m, _)
                    | (Some(Type(typename)), "::", _, Some(Raw(Str(m))))
                        if m.clone().normalize() == "frombase64string"
                            && (typename == "system.convert" || typename == "convert") =>
                    {
                        // infer type of member access
                        node.set(Type(String::from("convert::frombase64string")));
                    }
                    _ => (),
                }
            }
        } else if view.kind() == "invokation_expression" {
            if let (Some(type_lit), Some(op), Some(member_name), Some(args_list)) =
                (view.child(0), view.child(1), view.child(2), view.child(3))
            {
                match (
                    type_lit.data(),
                    op.text()?,
                    &member_name.text()?.to_string(),
                    member_name.data(),
                ) {
                    (Some(Type(typename)), "::", m, _)
                    | (Some(Type(typename)), ".", m, _)
                    | (Some(Type(typename)), ".", _, Some(Raw(Str(m))))
                    | (Some(Type(typename)), "::", _, Some(Raw(Str(m))))
                        if ((typename == "system.convert" || typename == "convert")
                            && m.clone().normalize() == "frombase64string")
                            || (typename == "convert::frombase64string" && m == "invoke") =>
                    {
                        // get the argument list if present
                        if let Some(argument_expression_list) =
                            args_list.named_child("argument_expression_list")
                        {
                            if let Some(arg_1) = argument_expression_list.child(0) {
                                if let Some(Raw(Str(s))) = arg_1.data() {
                                    if let Ok(bytes) = general_purpose::STANDARD.decode(s) {
                                        node.set(Array(
                                            bytes.iter().map(|b| Num(*b as i64)).collect(),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }
}

/// This rule will infer the [System.Text.Encoding]::utf8.getstring function
///
/// [System.Text.Encoding]::utf8.getstring([System.Convert]::FromBase64String('Zm9v')) => 'foo'
///
/// # Example
/// ```
/// extern crate tree_sitter;
/// extern crate tree_sitter_powershell;
///
/// use minusone::tree::{HashMapStorage, Tree};
/// use minusone::ps::build_powershell_tree;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::linter::Linter;
/// use minusone::ps::string::ParseString;
/// use minusone::ps::typing::ParseType;
/// use minusone::ps::method::{DecodeBase64, FromUTF};
///
/// let mut tree = build_powershell_tree("[System.Text.Encoding]::utf8.getstring([System.Convert]::FromBase64String('Zm9v'))").unwrap();
/// tree.apply_mut(&mut (
///     ParseString::default(),
///     Forward::default(),
///     ParseType::default(),
///     DecodeBase64::default(),
///     FromUTF::default()
/// )).unwrap();
///
/// let mut ps_litter_view = Linter::new();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"foo\"");
/// ```
#[derive(Default)]
pub struct FromUTF;

impl<'a> RuleMut<'a> for FromUTF {
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
        if view.kind() == "member_access" {
            if let (Some(type_lit), Some(op), Some(member_name)) =
                (view.child(0), view.child(1), view.child(2))
            {
                match (
                    type_lit.data(),
                    op.text()?,
                    &member_name.text()?.to_string(),
                    member_name.data(),
                ) {
                    (Some(Type(typename)), "::", m, _)
                    | (Some(Type(typename)), "::", _, Some(Raw(Str(m))))
                        if ["utf8", "utf16", "unicode", "ascii"]
                            .contains(&m.clone().normalize().as_str())
                            && (typename == "system.text.encoding"
                                || typename == "text.encoding") =>
                    {
                        // infer type of member access
                        let mut function_typename = String::from("text.encoding.");
                        function_typename += &m.clone().normalize();
                        node.set(Type(function_typename));
                    }

                    (Some(Type(typename)), ".", m, _)
                    | (Some(Type(typename)), ".", _, Some(Raw(Str(m))))
                        if ["text.encoding.utf8",
                            "text.encoding.utf16",
                            "text.encoding.unicode",
                            "text.encoding.ascii"]
                        .contains(&typename.as_str())
                            && m.clone().normalize() == "getstring" =>
                    {
                        let mut function_typename = typename.clone();
                        function_typename += ".getstring";
                        node.set(Type(function_typename));
                    }
                    _ => (),
                }
            }
        } else if view.kind() == "invokation_expression" {
            if let (Some(type_node), Some(op), Some(member_name), Some(args_list)) =
                (view.child(0), view.child(1), view.child(2), view.child(3))
            {
                match (
                    type_node.data(),
                    op.text()?,
                    &member_name.text()?.to_string(),
                    member_name.data(),
                ) {
                    (Some(Type(typename)), ".", m, _)
                    | (Some(Type(typename)), ".", _, Some(Raw(Str(m))))
                        if ((typename == "text.encoding.utf8" || typename == "text.encoding.ascii")
                            && m.clone().normalize() == "getstring")
                            || ((typename == "text.encoding.utf8.getstring" || typename == "text.encoding.ascii.getstring")
                                && m.clone().normalize() == "invoke") =>
                    {
                        if let Some(argument_expression_list) =
                            args_list.named_child("argument_expression_list")
                        {
                            if let Some(arg_1) = argument_expression_list.child(0) {
                                if let Some(Array(a)) = arg_1.data() {
                                    let mut int_vec = Vec::new();
                                    for value in a.iter() {
                                        if let Num(n) = value {
                                            int_vec.push(*n as u8);
                                        }
                                    }
                                    if let Ok(s) = String::from_utf8(int_vec) {
                                        node.set(Raw(Str(s)));
                                    }
                                }
                            }
                        }
                    }
                    (Some(Type(typename)), ".", m, _)
                    | (Some(Type(typename)), ".", _, Some(Raw(Str(m))))
                        if ((typename == "text.encoding.utf16"
                            || typename == "text.encoding.unicode")
                            && m.clone().normalize() == "getstring")
                            || ((typename == "text.encoding.utf16.getstring"
                                || typename == "text.encoding.unicode.getstring")
                                && m.clone().normalize() == "invoke") =>
                    {
                        if let Some(argument_expression_list) =
                            args_list.named_child("argument_expression_list")
                        {
                            if let Some(arg_1) = argument_expression_list.child(0) {
                                if let Some(Array(a)) = arg_1.data() {
                                    let mut int_vec = Vec::new();
                                    for value in a.iter() {
                                        if let Num(n) = value {
                                            int_vec.push(*n as u8);
                                        }
                                    }

                                    let int_vec: Vec<u16> = int_vec
                                        .chunks_exact(2)
                                        .map(|a| u16::from_ne_bytes([a[0], a[1]]))
                                        .collect();
                                    let int_vec = int_vec.as_slice();

                                    if let Ok(s) = String::from_utf16(int_vec) {
                                        node.set(Raw(Str(s)));
                                    }
                                }
                            }
                        }
                    },
                    _ => (),
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::ps::array::{ComputeArrayExpr, ParseArrayLiteral};
    use crate::ps::build_powershell_tree;
    use crate::ps::forward::Forward;
    use crate::ps::integer::ParseInt;
    use crate::ps::method::{DecodeBase64, FromUTF, Length};
    use crate::ps::string::ParseString;
    use crate::ps::typing::ParseType;
    use crate::ps::Powershell::{Array, Raw};
    use crate::ps::Value::{Num, Str};

    #[test]
    fn test_array_length() {
        let mut tree = build_powershell_tree("@(1,2,3).length").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ComputeArrayExpr::default(),
            ParseArrayLiteral::default(),
            Length::default(),
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
            Raw(Num(3))
        );
    }

    #[test]
    fn test_str_length() {
        let mut tree = build_powershell_tree("'foo'.length").unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            Forward::default(),
            ComputeArrayExpr::default(),
            ParseArrayLiteral::default(),
            Length::default(),
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
            Raw(Num(3))
        );
    }

    #[test]
    fn test_decode_base64() {
        let mut tree = build_powershell_tree("[System.Convert]::FromBase64String('Zm9v')").unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            Forward::default(),
            DecodeBase64::default(),
            ParseType::default(),
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
            Array(vec![Num(102), Num(111), Num(111)])
        );
    }

    #[test]
    fn test_error_decode_base64() {
        let mut tree =
            build_powershell_tree("[System.Convert]::FromBase64String('AAAAAAAAAA')").unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            Forward::default(),
            DecodeBase64::default(),
            ParseType::default(),
        ))
        .unwrap();

        assert_eq!(
            tree.root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data(),
            None
        );
    }

    #[test]
    fn test_error_decode_base64_with_invoke() {
        let mut tree =
            build_powershell_tree("[System.Convert]::'FromBase64String'.invoke('AAAAAAAAAA')")
                .unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            Forward::default(),
            DecodeBase64::default(),
            ParseType::default(),
        ))
        .unwrap();

        assert_eq!(
            tree.root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data(),
            None
        );
    }

    #[test]
    fn test_decode_utf8() {
        let mut tree =
            build_powershell_tree("[System.Text.Encoding]::utf8.getstring(@(102, 111, 111))")
                .unwrap();
        tree.apply_mut(&mut (
            Forward::default(),
            FromUTF::default(),
            ParseType::default(),
            ParseInt::default(),
            ParseArrayLiteral::default(),
            ComputeArrayExpr::default(),
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
            Raw(Str("foo".to_string()))
        );
    }

    #[test]
    fn test_decode_utf16() {
        let mut tree = build_powershell_tree(
            "[System.Text.Encoding]::utf16.getstring(@(102, 0, 111, 0, 111, 0))",
        )
        .unwrap();
        tree.apply_mut(&mut (
            Forward::default(),
            FromUTF::default(),
            ParseType::default(),
            ParseInt::default(),
            ParseArrayLiteral::default(),
            ComputeArrayExpr::default(),
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
            Raw(Str("foo".to_string()))
        );
    }

    #[test]
    fn test_decode_utf16_with_invoke() {
        let mut tree = build_powershell_tree(
            "[System.Text.Encoding]::'utf16'.'getstring'.invoke(@(102, 0, 111, 0, 111, 0))",
        )
        .unwrap();
        tree.apply_mut(&mut (
            Forward::default(),
            FromUTF::default(),
            ParseType::default(),
            ParseInt::default(),
            ParseString::default(),
            ParseArrayLiteral::default(),
            ComputeArrayExpr::default(),
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
            Raw(Str("foo".to_string()))
        );
    }
}
