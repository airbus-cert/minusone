use rule::RuleMut;
use ps::Powershell;
use tree::{NodeMut, BranchFlow};
use error::MinusOneResult;
use ps::Powershell::{Array, Raw, Type};
use ps::Value::{Num, Str};
use base64::{engine::general_purpose, Engine as _};


/// Compute the length of predictable Array or string
///
/// # Example
/// ```
/// extern crate tree_sitter;
/// extern crate tree_sitter_powershell;
/// extern crate minusone;
///
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
/// ps_litter_view.print(&tree.root().unwrap()).unwrap();
///
/// assert_eq!(ps_litter_view.output, "3");
/// ```
#[derive(Default)]
pub struct Length;

impl<'a> RuleMut<'a> for Length {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()> {
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()> {
        let view = node.view();
        if view.kind() == "member_access" {
            if let (Some(primary_expression), Some(operator), Some(member_name)) = (view.child(0), view.child(1), view.child(2)) {
                match (primary_expression.data(), operator.text()?, member_name.text()?.to_lowercase().as_str()) {
                    (Some(Array(value)), ".", "length") => node.set(Raw(Num(value.len() as i32))),
                    (Some(Raw(Str(s))), ".", "length") => node.set(Raw(Num(s.len() as i32))),
                    _ => ()
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
/// extern crate minusone;
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
/// ps_litter_view.print(&tree.root().unwrap()).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"foo\"");
/// ```
#[derive(Default)]
pub struct DecodeBase64;

impl<'a> RuleMut<'a> for DecodeBase64 {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()> {
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()> {
        let view = node.view();
        if view.kind() == "invokation_expression" {
            if let (Some(type_lit), Some(op), Some(member_name), Some(args_list)) =
                (view.child(0), view.child(1), view.child(2), view.child(3))
            {
                match (type_lit.data(), op.text()?, member_name.text()?.to_lowercase().as_str()) {
                    (Some(Type(typename)), "::", "frombase64string") if typename == "system.convert" || typename == "convert" =>
                    {
                        // get the argument list if present
                        if let Some(argument_expression_list) =
                            args_list.named_child("argument_expression_list")
                        {
                            if let Some(arg_1) = argument_expression_list.child(0) {
                                if let Some(Raw(Str(s))) = arg_1.data() {
                                    if let Ok(bytes) = general_purpose::STANDARD.decode(s) {
                                        node.set(Array(bytes.iter().map(|b| Num(*b as i32)).collect()));
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
/// extern crate minusone;
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
/// ps_litter_view.print(&tree.root().unwrap()).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"foo\"");
/// ```
#[derive(Default)]
pub struct FromUTF;

impl<'a> RuleMut<'a> for FromUTF {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()> {
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()> {
        let view = node.view();
        if view.kind() == "invokation_expression" {
            if let (Some(member_access), Some(op), Some(member_name)) =
                (view.child(0), view.child(1), view.child(2))
            {
                if member_access.kind() != "member_access" {
                    return Ok(());
                }

                match (
                    member_access.child(0).unwrap().data(),
                    member_access
                        .child(2)
                        .unwrap()
                        .text()?
                        .to_lowercase()
                        .as_str(),
                    op.text()?,
                    member_name.text()?.to_lowercase().as_str(),
                ) {
                    (Some(Type(typename)), member_name, ".", "getstring")
                    | (Some(Type(typename)), member_name, ".", "getstring")
                        if (typename == "system.text.encoding" || typename == "text.encoding")
                            && (member_name == "utf8" || member_name == "unicode") =>
                    {
                        let arg_list = view.child(3).unwrap().child(1).unwrap().child(0).unwrap();

                        match arg_list.data() {
                            Some(Array(a)) => {
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
                            _ => {}
                        }
                    }
                    (Some(Type(typename)), "utf16", ".", "getstring")
                        if typename == "system.text.encoding" || typename == "text.encoding" =>
                    {
                        let arg_list = view.child(3).unwrap().child(1).unwrap().child(0).unwrap();

                        match arg_list.data() {
                            Some(Array(a)) => {
                                let mut int_vec = Vec::new();
                                for value in a.iter() {
                                    if let Num(n) = value {
                                        int_vec.push(*n as u8);
                                    }
                                }

                                let int_vec: Vec<u16> = int_vec
                                    .chunks_exact(2)
                                    .into_iter()
                                    .map(|a| u16::from_ne_bytes([a[0], a[1]]))
                                    .collect();
                                let int_vec = int_vec.as_slice();

                                if let Ok(s) = String::from_utf16(&int_vec) {
                                    node.set(Raw(Str(s)));
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}


#[cfg(test)]
mod test {
    use ps::method::{Length, DecodeBase64, FromUTF};
    use ps::build_powershell_tree;
    use ps::integer::ParseInt;
    use ps::forward::Forward;
    use ps::array::{ComputeArrayExpr, ParseArrayLiteral};
    use ps::Powershell::{Raw, Array};
    use ps::Value::{Num, Str};
    use ps::string::ParseString;
    use ps::typing::ParseType;

    #[test]
    fn test_array_length() {
        let mut tree = build_powershell_tree("@(1,2,3).length").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ComputeArrayExpr::default(),
            ParseArrayLiteral::default(),
            Length::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Raw(Num(3))
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
            Length::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Raw(Num(3))
        );
    }

    #[test]
    fn test_decode_base64() {
        let mut tree = build_powershell_tree("[System.Convert]::FromBase64String('Zm9v')").unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            Forward::default(),
            DecodeBase64::default(),
            ParseType::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Num(102), Num(111), Num(111)])
        );
    }

    #[test]
    fn test_error_decode_base64() {
        let mut tree = build_powershell_tree("[System.Convert]::FromBase64String('AAAAAAAAAA')").unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            Forward::default(),
            DecodeBase64::default(),
            ParseType::default()
        )).unwrap();

        assert_eq!(tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data(), None
        );
    }

    #[test]
    fn test_decode_utf8() {
        let mut tree = build_powershell_tree("[System.Text.Encoding]::utf8.getstring(@(102, 111, 111))").unwrap();
        tree.apply_mut(&mut (
            Forward::default(),
            FromUTF::default(),
            ParseType::default(),
            ParseInt::default(),
            ParseArrayLiteral::default(),
            ComputeArrayExpr::default(),
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Raw(Str("foo".to_string()))
        );
    }

    #[test]
    fn test_decode_utf16() {
        let mut tree = build_powershell_tree("[System.Text.Encoding]::utf16.getstring(@(102, 0, 111, 0, 111, 0))").unwrap();
        tree.apply_mut(&mut (
            Forward::default(),
            FromUTF::default(),
            ParseType::default(),
            ParseInt::default(),
            ParseArrayLiteral::default(),
            ComputeArrayExpr::default(),
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Raw(Str("foo".to_string()))
        );
    }
}