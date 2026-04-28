use crate::error::MinusOneResult;
use crate::ps::Powershell;
use crate::ps::Powershell::{Array, PSItem, Raw, Type};
use crate::ps::Value::{Bool, Num, Str};
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use log::{trace, warn};

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
/// let mut ps_litter_view = Linter::default();
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
                                trace!("cast (L): Setting node with casted int value: {}", number);
                                node.set(Raw(Num(number)));
                            } else {
                                warn!("cast (L): Failed to cast value {} to int", v);
                            }
                        }
                        (Some(Type(t)), Some(Raw(v))) if t == "byte" => {
                            if let Some(number) = v.clone().to_i64() {
                                if number < 256 && number > 0 {
                                    trace!("cast (L): Setting node with casted byte value: {}", number);
                                    node.set(Raw(Num(number)));
                                } else {
                                    warn!("cast (L): Failed to cast value {} to byte, out of range", number);
                                }
                            }
                        }
                        (Some(Type(t)), Some(Raw(Num(num)))) if t == "char" => {
                            if *num > 0xff {
                                warn!("cast (L): The value {} is too big to be casted to char and will be truncated to fit into a char", num);
                            }

                            let mut result = String::new();
                            // todo: check if the number is in the valid range for char (0..=0x10FFFF) instead of just truncating it to fit into a char
                            result.push(char::from(*num as u8));
                            trace!("cast (L): Setting node with casted char value: {}", result);
                            node.set(Raw(Str(result)));
                        }
                        (Some(Type(t)), Some(PSItem(values))) if t == "int" => {
                            let mut result = Vec::new();
                            for v in values {
                                if let Some(n) = v.clone().to_i64() {
                                    result.push(Num(n));
                                } else {
                                    warn!("cast (L): Failed to cast value {:?} to int, invalid number", v);
                                    return Ok(());
                                }
                            }
                            trace!("cast (L): Setting node with casted int value: {:?}", result);
                            node.set(PSItem(result));
                        }
                        (Some(Type(t)), Some(PSItem(values))) if t == "byte" => {
                            let mut result = Vec::new();
                            for v in values {
                                if let Some(n) = v.clone().to_i64() {
                                    // invalid cast
                                    if !(0..=255).contains(&n) {
                                        warn!("cast (L): Failed to cast value {:?} to byte, out of range", v);
                                        return Ok(());
                                    }
                                    result.push(Num(n));
                                } else {
                                    warn!("cast (L): Failed to cast value {:?} to byte, invalid number", v);
                                    return Ok(());
                                }
                            }
                            trace!("cast (L): Setting node with casted byte value: {:?}", result);
                            node.set(PSItem(result));
                        }
                        (Some(Type(t)), Some(PSItem(values))) => if t == "char" {
                            let mut result = Vec::new();
                            for e in values {
                                let casted_value = match e {
                                    Num(number) => {
                                        if *number < 0 || *number > u32::MAX as i64 {
                                            warn!("cast (L): The value {} is out of range for char and cannot be casted to an unsigned integer", number);
                                            return Ok(());
                                        }

                                        char::from_u32(*number as u32)
                                    }
                                    _ => {
                                        warn!("cast (L): Failed to cast value {:?} to char, invalid type", e);
                                        None
                                    }
                                };

                                if casted_value.is_none() {
                                    // Failed to cast -> stop the rule
                                    warn!("cast (L): Failed to cast value {:?} to char, invalid number", e);
                                    return Ok(());
                                }

                                result.push(Str(casted_value.unwrap().to_string()));
                            }
                            trace!("cast (L): Setting node with casted char value: {:?}", result);
                            node.set(PSItem(result));
                        },
                        (Some(Type(t)), Some(Raw(Num(v)))) => if t == "bool" {
                            trace!(" Setting node with casted bool value: {}", *v != 0);
                            node.set(Raw(Bool(*v != 0)));
                        },
                        (Some(Type(t)), Some(Raw(Str(v)))) => if t == "bool" {
                            trace!("cast (L): Setting node with casted bool value: {}", !v.is_empty());
                                node.set(Raw(Bool(!v.is_empty())));
                        } else if t == "char[]" {
                            let array = v.chars().map(|c| Str(c.to_string())).collect();
                            trace!("cast (L): Setting node with casted char[] value: {:?}", array);
                            node.set(Array(array));
                        } else if t == "string" {
                                trace!("cast (L): Setting node with casted string value: {}", v);
                            node.set(Raw(Str(v.clone())));
                        },
                        (Some(Type(t)), Some(Raw(Bool(v)))) => if t == "bool" {
                            trace!("cast (L): Setting node with casted bool value: {}", v);
                            node.set(Raw(Bool(*v)));
                        },
                        (Some(Type(t)), Some(Powershell::Null)) => if t == "bool" {
                            trace!("cast (L): Setting node with casted bool value: false");
                            node.set(Raw(Bool(false)));
                        },
                        (Some(Type(t)), Some(Powershell::HashMap(_))) => if t == "bool" {
                            trace!("cast (L): Setting node with casted bool value: true");
                            node.set(Raw(Bool(true)));
                        },
                        (Some(Type(t)), None) => if t == "bool" {
                            trace!("cast (L): Setting node with casted bool value: true");
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
                                } else {
                                    return Ok(());
                                }
                            }

                                trace!("cast (L): Setting node with casted char[] value: {:?}", result);
                            node.set(Array(result));
                        }
                        (Some(Type(t)), Some(Array(_array_value))) if t == "bool" => {
                            trace!("cast (L): Setting node with casted bool value: true");
                            node.set(Raw(Bool(true)));
                        }
                        _ => ()
                    }
                }
            }

            // Forward inferred type in case of cast expression
            "expression_with_unary_operator" => {
                if let Some(child) = view.child(0)
                    && child.kind() == "cast_expression"
                    && let Some(data) = child.data()
                {
                    trace!("cast (L): Forwarding casted value: {:?}", data);
                    node.set(data.clone());
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
        if view.kind() == "expression_with_unary_operator"
            && let (Some(operator), Some(expression)) = (view.child(0), view.child(1))
        {
            match (operator.text()?.to_lowercase().as_str(), expression.data()) {
                ("+", Some(Powershell::Null)) => {
                    trace!("cast null (L): Setting node with casted null value: 0");
                    node.set(Raw(Num(0)))
                }
                ("-", Some(Powershell::Null)) => {
                    trace!("cast null (L): Setting node with casted null value: 0");
                    node.set(Raw(Num(0)))
                }
                _ => (),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::ps::Powershell::{Array, Raw};
    use crate::ps::Value::{Num, Str};
    use crate::ps::array::ParseArrayLiteral;
    use crate::ps::build_powershell_tree;
    use crate::ps::cast::Cast;
    use crate::ps::foreach::{ForEach, PSItemInferrator};
    use crate::ps::forward::Forward;
    use crate::ps::integer::{AddInt, ParseInt};
    use crate::ps::string::{ConcatString, ParseString};
    use crate::ps::typing::ParseType;

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
