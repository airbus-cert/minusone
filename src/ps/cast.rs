use rule::RuleMut;
use ps::Powershell;
use tree::{NodeMut, BranchFlow};
use error::{MinusOneResult, Error};
use ps::Value::{Num, Str, Bool};
use ps::Powershell::{Raw, PSItem};

/// Handle static cast operations
/// For example [char]0x74 => 't'
#[derive(Default)]
pub struct Cast;

/// We will infer cast value using down to top exploring
///
/// # Example
/// ```
/// extern crate tree_sitter;
/// extern crate tree_sitter_powershell;
/// extern crate minusone;
///
/// use minusone::ps::build_powershell_tree;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::integer::ParseInt;
/// use minusone::ps::linter::Linter;
/// use minusone::ps::string::ParseString;
/// use minusone::ps::cast::Cast;
///
/// let mut tree = build_powershell_tree("[char]0x61").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(),
///     Forward::default(),
///     ParseString::default(),
///     Cast::default()
///     )
/// ).unwrap();
///
/// let mut ps_litter_view = Linter::new();
/// ps_litter_view.print(&tree.root().unwrap()).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"a\"");
/// ```
impl<'a> RuleMut<'a> for Cast {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()>{
        let view = node.view();
        match view.kind() {
            "cast_expression" => {
                if let (Some(type_literal), Some(expression)) = (view.child(0), view.child(1)) {
                    match (type_literal
                        .child(1).ok_or(Error::invalid_child())? // type_spec
                        .child(0).ok_or(Error::invalid_child())? // type_name
                        .child(0).ok_or(Error::invalid_child())?.text()?.to_lowercase().as_str(),
                           expression.data()) // type_identifier
                    {
                        ("int", Some(Raw(v))) => {
                            if let Some(number) = v.clone().to_i32() {
                                node.set(Raw(Num(number)));
                            }
                        },
                        ("byte", Some(Raw(v))) => {
                            if let Some(number) = v.clone().to_i32() {
                                if number < 256 && number > 0 {
                                    node.set(Raw(Num(number)));
                                }
                            }
                        },
                        ("char", Some(Raw(Num(num)))) => {
                            let mut result = String::new();
                            result.push(char::from(*num as u8));
                            node.set(Raw(Str(result)));
                        },
                        ("int", Some(PSItem(values))) => {
                            let mut result = Vec::new();
                            for v in values {
                                if let Some(n) = v.clone().to_i32() {
                                    result.push(Num(n));
                                }
                                else {
                                    return Ok(())
                                }
                            }
                            node.set(PSItem(result));
                        },
                        ("byte", Some(PSItem(values))) => {
                            let mut result = Vec::new();
                            for v in values {
                                if let Some(n) = v.clone().to_i32() {
                                    // invalid cast
                                    if !(0..=255).contains(&n) {
                                        return Ok(())
                                    }
                                    result.push(Num(n));
                                }
                                else {
                                    return Ok(())
                                }
                            }
                            node.set(PSItem(result));
                        },
                        ("char", Some(PSItem(values))) => {
                            let mut result = Vec::new();
                            for e in values {
                                let casted_value = match e {
                                    Num(number) => {
                                        char::from_u32(*number as u32)
                                    },
                                    _ => None
                                };

                                if casted_value.is_none() {
                                    // Failed to cast -> stop the rule
                                    return Ok(())
                                }

                                result.push(Str(casted_value.unwrap().to_string()));
                            }
                            node.set(PSItem(result));
                        },
                        ("bool", Some(Raw(Num(v)))) => {
                            node.set(Raw(Bool(*v != 0)));
                        },
                        ("bool", Some(Raw(Str(v)))) => {
                            node.set(Raw(Bool(!v.is_empty())));
                        },
                        ("bool", Some(Raw(Bool(v)))) => {
                            node.set(Raw(Bool(*v)));
                        },
                        ("bool", Some(Powershell::Null)) => {
                            node.set(Raw(Bool(false)));
                        },
                        ("bool", Some(Powershell::Array(_))) | ("bool", Some(Powershell::HashMap))  => {
                            node.set(Raw(Bool(true)));
                        },
                        ("bool", None) => {
                            node.set(Raw(Bool(true)));
                        },
                        _ => ()
                    }
                }
            },

            // Forward inferred type in case of cast expression
            "expression_with_unary_operator" => {
                if let Some(child) = view.child(0) {
                    if child.kind() == "cast_expression" {
                        if let Some(data) = child.data() {
                            node.set(data.clone());
                        }
                    }
                }
            }
            _ => ()
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct CastNull;

impl<'a> RuleMut<'a> for CastNull {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()>{
        let view = node.view();
        if view.kind() == "expression_with_unary_operator" {
            if let (Some(operator), Some(expression)) = (view.child(0), view.child(1)) {
                match (operator.text()?.to_lowercase().as_str(), expression.data()) {
                    ("+", Some(Powershell::Null)) => node.set(Raw(Num(0))),
                    ("-", Some(Powershell::Null)) => node.set(Raw(Num(0))),
                    _ => ()
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use ps::build_powershell_tree;
    use ps::integer::{ParseInt, AddInt};
    use ps::forward::Forward;
    use ps::cast::Cast;
    use ps::Value::{Str, Num};
    use ps::Powershell::{Raw, Array};
    use ps::string::{ConcatString, ParseString};
    use ps::foreach::{ForEach, PSItemInferrator};
    use ps::array::ParseArrayLiteral;

    #[test]
    fn test_cast_int_to_char() {
        let mut tree = build_powershell_tree("[char]0x61").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            Cast::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Raw(Str("a".to_string()))
        );
    }

    #[test]
    fn test_cast_char_to_int() {
        let mut tree = build_powershell_tree("[int]'61'").unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            Forward::default(),
            Cast::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Raw(Num(61))
        );
    }

    #[test]
    fn test_cast_int_additive_to_char() {
        let mut tree = build_powershell_tree("[char](0x61 + 3)").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            AddInt::default(),
            Forward::default(),
            Cast::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Raw(Str("d".to_string()))
        );
    }

    #[test]
    fn test_cast_int_concat_char() {
        let mut tree = build_powershell_tree("[char]0x74 + [char]0x6f + [char]0x74 + [char]0x6f").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ConcatString::default(),
            Forward::default(),
            Cast::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Raw(Str("toto".to_string()))
        );
    }

    #[test]
    fn test_cast_foreach_char() {
        let mut tree = build_powershell_tree("(0x74, 0x6f, 0x74, 0x6f) | % {[char]$_}").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            Cast::default(),
            ForEach::default(),
            ParseArrayLiteral::default(),
            PSItemInferrator::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![
                Str("t".to_string()),
                Str("o".to_string()),
                Str("t".to_string()),
                Str("o".to_string())
            ])
        );
    }
}
