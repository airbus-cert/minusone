use crate::error::MinusOneResult;
use crate::ps::Powershell;
use crate::ps::Powershell::{Array, PSItem, Raw, Type};
use crate::ps::Value::{Bool, Num, Str};
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};

/// Handle static cast operations
/// For example [char]0x74 => 't'
#[derive(Default)]
pub struct Cast;

/// We will infer cast value using down to top exploring
///
/// # Example
/// ```
/// use minusone::ps::build_powershell_tree;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::integer::ParseInt;
/// use minusone::ps::linter::Linter;
/// use minusone::ps::string::ParseString;
/// use minusone::ps::cast::Cast;
/// use minusone::ps::typing::ParseType;
///
/// let mut tree = build_powershell_tree("[char]0x61").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(),
///     Forward::default(),
///     ParseString::default(),
///     Cast::default(),
///     ParseType::default()
///     )
/// ).unwrap();
///
/// let mut ps_litter_view = Linter::new();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"a\"");
/// ```
impl<'a> RuleMut<'a> for Cast {
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
            "cast_expression" => {
                if let (Some(type_literal), Some(expression)) = (view.child(0), view.child(1)) {
                    match (type_literal.data(), expression.data()) // type_identifier
                    {
                        (Some(Type(t)), Some(Raw(v))) if t == "int" => {
                            if let Some(number) = v.clone().to_i64() {
                                node.set(Raw(Num(number)));
                            }
                        },
                        (Some(Type(t)), Some(Raw(v))) if t == "byte" => {
                            if let Some(number) = v.clone().to_i64() {
                                if number < 256 && number > 0 {
                                    node.set(Raw(Num(number)));
                                }
                            }
                        },
                        (Some(Type(t)), Some(Raw(Num(num)))) if t == "char" => {
                            let mut result = String::new();
                            result.push(char::from(*num as u8));
                            node.set(Raw(Str(result)));
                        },
                        (Some(Type(t)), Some(PSItem(values))) if t == "int" => {
                            let mut result = Vec::new();
                            for v in values {
                                if let Some(n) = v.clone().to_i64() {
                                    result.push(Num(n));
                                } else {
                                    return Ok(())
                                }
                            }
                            node.set(PSItem(result));
                        },
                        (Some(Type(t)), Some(PSItem(values))) if t == "byte" => {
                            let mut result = Vec::new();
                            for v in values {
                                if let Some(n) = v.clone().to_i64() {
                                    // invalid cast
                                    if !(0..=255).contains(&n) {
                                        return Ok(())
                                    }
                                    result.push(Num(n));
                                } else {
                                    return Ok(())
                                }
                            }
                            node.set(PSItem(result));
                        },
                        (Some(Type(t)), Some(PSItem(values))) => if t == "char" {
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
                        (Some(Type(t)), Some(Raw(Num(v)))) => if t == "bool" {
                            node.set(Raw(Bool(*v != 0)));
                        },
                        (Some(Type(t)), Some(Raw(Str(v)))) => if t == "bool" {
                            node.set(Raw(Bool(!v.is_empty())));
                        } else if t == "char[]" {
                            node.set(Array(v.chars().map(|c| Str(c.to_string())).collect()));
                        } else if t == "string" {
                            node.set(Raw(Str(v.clone())));
                        },
                        (Some(Type(t)), Some(Raw(Bool(v)))) => if t == "bool" {
                            node.set(Raw(Bool(*v)));
                        },
                        (Some(Type(t)), Some(Powershell::Null)) => if t == "bool" {
                            node.set(Raw(Bool(false)));
                        },
                        (Some(Type(t)), Some(Powershell::HashMap(_))) => if t == "bool" {
                            node.set(Raw(Bool(true)));
                        },
                        (Some(Type(t)), None) => if t == "bool" {
                            node.set(Raw(Bool(true)));
                        },
                        (Some(Type(t)), Some(Array(array_value))) if t == "char[]" => {
                            let mut result = Vec::new();
                            let transformed_value: Vec<Option<char>> = array_value.iter().map(|v| {
                                match v {
                                    Num(n) => char::from_u32(*n as u32),
                                    Str(c) if c.len() == 1 => c.chars().next(),
                                    _ => None
                                }
                            }).collect();

                            for v in transformed_value {
                                if let Some(c) = v {
                                    result.push(Str(String::from(c)));
                                }
                                else {
                                    return Ok(());
                                }
                            }

                            node.set(Array(result));
                        },
                        (Some(Type(t)), Some(Array(_array_value))) if t == "bool" => {
                            node.set(Raw(Bool(true)));
                        }
                        _ => ()
                    }
                } 
            }

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
            _ => (),
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct CastNull;

impl<'a> RuleMut<'a> for CastNull {
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
        if view.kind() == "expression_with_unary_operator" {
            if let (Some(operator), Some(expression)) = (view.child(0), view.child(1)) {
                match (operator.text()?.to_lowercase().as_str(), expression.data()) {
                    ("+", Some(Powershell::Null)) => node.set(Raw(Num(0))),
                    ("-", Some(Powershell::Null)) => node.set(Raw(Num(0))),
                    _ => (),
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::ps::array::ParseArrayLiteral;
    use crate::ps::build_powershell_tree;
    use crate::ps::cast::Cast;
    use crate::ps::foreach::{ForEach, PSItemInferrator};
    use crate::ps::forward::Forward;
    use crate::ps::integer::{AddInt, ParseInt};
    use crate::ps::string::{ConcatString, ParseString};
    use crate::ps::typing::ParseType;
    use crate::ps::Powershell::{Array, Raw};
    use crate::ps::Value::{Num, Str};

    #[test]
    fn test_cast_int_to_char() {
        let mut tree = build_powershell_tree("[char]0x61").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            Cast::default(),
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
            Raw(Str("a".to_string()))
        );
    }

    #[test]
    fn test_cast_char_to_int() {
        let mut tree = build_powershell_tree("[int]'61'").unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            Forward::default(),
            Cast::default(),
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
            Raw(Num(61))
        );
    }

    #[test]
    fn test_cast_int_additive_to_char() {
        let mut tree = build_powershell_tree("[char](0x61 + 3)").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            AddInt::default(),
            Forward::default(),
            Cast::default(),
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
            Raw(Str("d".to_string()))
        );
    }

    #[test]
    fn test_cast_int_concat_char() {
        let mut tree =
            build_powershell_tree("[char]0x74 + [char]0x6f + [char]0x74 + [char]0x6f").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ConcatString::default(),
            Forward::default(),
            Cast::default(),
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
            Raw(Str("toto".to_string()))
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
            PSItemInferrator::default(),
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
            Array(vec![
                Str("t".to_string()),
                Str("o".to_string()),
                Str("t".to_string()),
                Str("o".to_string())
            ])
        );
    }
}
