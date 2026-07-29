use crate::error::MinusOneResult;
use crate::ps::Powershell;
use crate::ps::Powershell::{Array, Raw, Type};
use crate::ps::Value::{Num, Str};
use crate::ps::tool::StringTool;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use base64::{Engine as _, engine::general_purpose};
use log::{trace, warn};

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
/// let mut ps_litter_view = Linter::default();
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
        if view.kind() == "member_access"
            && let (Some(primary_expression), Some(operator), Some(member_name)) =
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
                    trace!(
                        "Length (L): Setting node with array length: {}",
                        value.len()
                    );
                    node.set(Raw(Num(value.len() as i64)))
                }
                (Some(Powershell::Bytes(value)), ".", m, _)
                | (Some(Powershell::Bytes(value)), ".", _, Some(Raw(Str(m))))
                    if m.clone().normalize() == "length" =>
                {
                    trace!(
                        "Length (L): Setting node with bytes length: {}",
                        value.len()
                    );
                    node.set(Raw(Num(value.len() as i64)))
                }
                (Some(Raw(Str(s))), ".", m, None)
                | (Some(Raw(Str(s))), ".", _, Some(Raw(Str(m))))
                    if m.clone().normalize() == "length" =>
                {
                    trace!("Length (L): Setting node with string length: {}", s.len());
                    node.set(Raw(Num(s.len() as i64)))
                }
                _ => (),
            }
        }
        Ok(())
    }
}

/// This rule will infer the [System.Convert]::FromBase64String function
///
/// [System.Convert]::FromBase64String('Zm9v') => @(102, 111, 111)
///
/// Combined with [`crate::ps::encoding::EncodingGetString`], this lets a full
/// `[System.Text.Encoding]::UTF8.GetString([System.Convert]::FromBase64String('Zm9v'))` chain fold to `'foo'`.
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
/// use minusone::ps::method::DecodeBase64;
/// use minusone::ps::encoding::{EncodingType, EncodingGetString};
///
/// let mut tree = build_powershell_tree("[System.Text.Encoding]::utf8.getstring([System.Convert]::FromBase64String('Zm9v'))").unwrap();
/// tree.apply_mut(&mut (
///     ParseString::default(),
///     Forward::default(),
///     ParseType::default(),
///     DecodeBase64::default(),
///     EncodingType::default(),
///     EncodingGetString::default()
/// )).unwrap();
///
/// let mut ps_litter_view = Linter::default();
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
                        trace!(
                            "DecodeBase64 (L): Setting node with type convert::frombase64string"
                        );
                        node.set(Type(String::from("convert::frombase64string")));
                    }
                    _ => (),
                }
            }
        } else if view.kind() == "invokation_expression"
            && let (Some(type_lit), Some(op), Some(member_name), Some(args_list)) =
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
                        && let Some(arg_1) = argument_expression_list.child(0)
                        && let Some(Raw(Str(s))) = arg_1.data()
                    {
                        match general_purpose::STANDARD.decode(s) {
                            Ok(bytes) => {
                                trace!(
                                    "DecodeBase64 (L): Setting node with decoded bytes: {:?}",
                                    bytes
                                );
                                node.set(Powershell::Bytes(bytes));
                            }
                            Err(e) => {
                                warn!(
                                    "DecodeBase64 (L): Failed to decode base64 string: {}. Error: {}",
                                    s, e
                                );
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::ps::Powershell;
    use crate::ps::Powershell::Raw;
    use crate::ps::Value::Num;
    use crate::ps::array::{ComputeArrayExpr, ParseArrayLiteral};
    use crate::ps::build_powershell_tree;
    use crate::ps::forward::Forward;
    use crate::ps::integer::ParseInt;
    use crate::ps::method::{DecodeBase64, Length};
    use crate::ps::string::ParseString;
    use crate::ps::typing::ParseType;

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
            Powershell::Bytes(vec![102, 111, 111])
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
}
