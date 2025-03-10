use error::MinusOneResult;
use ps::Powershell;
use ps::Powershell::Raw;
use ps::Value::Num;
use rule::RuleMut;
use tree::{ControlFlow, NodeMut};

/// Parse int will interpret integer node into Rust world
/// as decimal
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
/// use minusone::ps::integer::{ParseInt, AddInt};
/// use minusone::ps::linter::Linter;
///
/// let mut tree = build_powershell_tree("4 + 0x5").unwrap();
/// tree.apply_mut(&mut (ParseInt::default(), Forward::default())).unwrap();
///
/// let mut ps_litter_view = Linter::new();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "4 + 5");
/// ```
#[derive(Default)]
pub struct ParseInt;

impl<'a> RuleMut<'a> for ParseInt {
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
        let token = view.text()?;
        match view.kind() {
            "hexadecimal_integer_literal" => {
                if let Ok(number) = u32::from_str_radix(&token[2..], 16) {
                    node.reduce(Raw(Num(number as i64)));
                }
            }
            "decimal_integer_literal" => {
                if let Ok(number) = token.parse::<i64>() {
                    node.reduce(Raw(Num(number)));
                }
            }
            "expression_with_unary_operator" => {
                if let (Some(operator), Some(expression)) = (view.child(0), view.child(1)) {
                    match (operator.text()?, expression.data()) {
                        ("-", Some(Raw(Num(num)))) => node.set(Raw(Num(-num))),
                        ("+", Some(Raw(Num(num)))) => node.set(Raw(Num(*num))),
                        _ => (),
                    }
                }
            }
            _ => (),
        }

        Ok(())
    }
}

/// This rule will infer integer operation
/// of type add (+) and minus(-)
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
/// use minusone::ps::integer::{ParseInt, AddInt};
/// use minusone::ps::linter::Linter;
///
/// let mut tree = build_powershell_tree("4 + 5 - 2").unwrap();
/// tree.apply_mut(&mut (ParseInt::default(), Forward::default(), AddInt::default())).unwrap();
///
/// let mut ps_litter_view = Linter::new();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "7");
/// ```
#[derive(Default)]
pub struct AddInt;

impl<'a> RuleMut<'a> for AddInt {
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
        let node_view = node.view();
        if node_view.kind() == "additive_expression"
            || node_view.kind() == "additive_argument_expression"
        {
            if let (Some(left_op), Some(operator), Some(right_op)) =
                (node_view.child(0), node_view.child(1), node_view.child(2))
            {
                match (left_op.data(), operator.text()?, right_op.data()) {
                    (Some(Raw(Num(number_left))), "+", Some(Raw(Num(number_right)))) => {
                        if let Some(result) = number_left.checked_add(*number_right) {
                            node.reduce(Raw(Num(result)))
                        }
                    }
                    (Some(Raw(Num(number_left))), "-", Some(Raw(Num(number_right)))) => {
                        if let Some(result) = number_left.checked_sub(*number_right) {
                            node.reduce(Raw(Num(result)))
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

/// This rule will infer integer operation
/// of type add (+) and minus(-)
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
/// use minusone::ps::integer::{ParseInt, MultInt};
/// use minusone::ps::linter::Linter;
///
/// let mut tree = build_powershell_tree("3 * 4 / 12").unwrap();
/// tree.apply_mut(&mut (ParseInt::default(), Forward::default(), MultInt::default())).unwrap();
///
/// let mut ps_litter_view = Linter::new();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "1");
/// ```
#[derive(Default)]
pub struct MultInt;

impl<'a> RuleMut<'a> for MultInt {
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
        let node_view = node.view();
        if node_view.kind() == "multiplicative_expression"
            || node_view.kind() == "multiplicative_argument_expression"
        {
            if let (Some(left_op), Some(operator), Some(right_op)) =
                (node_view.child(0), node_view.child(1), node_view.child(2))
            {
                match (left_op.data(), operator.text()?, right_op.data()) {
                    (Some(Raw(Num(number_left))), "*", Some(Raw(Num(number_right)))) => {
                        if let Some(result) = number_left.checked_mul(*number_right) {
                            node.reduce(Raw(Num(result)))
                        }
                    }
                    (Some(Raw(Num(number_left))), "/", Some(Raw(Num(number_right)))) => {
                        if let Some(result) = number_left.checked_div(*number_right) {
                            node.reduce(Raw(Num(result)))
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
    use super::*;
    use ps::build_powershell_tree;
    use ps::forward::Forward;

    #[test]
    fn test_add_two_elements() {
        let mut tree = build_powershell_tree("4 + 5").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), Forward::default(), AddInt::default()))
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
            Raw(Num(9))
        );
    }

    #[test]
    fn test_add_three_elements() {
        let mut tree = build_powershell_tree("4 + 5 + 9").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), Forward::default(), AddInt::default()))
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
            Raw(Num(18))
        );
    }

    #[test]
    fn test_minus_two_elements() {
        let mut tree = build_powershell_tree("4 - 5").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), Forward::default(), AddInt::default()))
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
            Raw(Num(-1))
        );
    }

    #[test]
    fn test_minus_two_elements_with_unary_operators() {
        let mut tree = build_powershell_tree("4 + -5").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), Forward::default(), AddInt::default()))
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
            Raw(Num(-1))
        );
    }

    #[test]
    fn test_minus_two_elements_with_two_unary_operators() {
        let mut tree = build_powershell_tree("-4 - 5").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), Forward::default(), AddInt::default()))
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
            Raw(Num(-9))
        );
    }

    #[test]
    fn test_mul_two_elements() {
        let mut tree = build_powershell_tree("4 * 5").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), Forward::default(), MultInt::default()))
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
            Raw(Num(20))
        );
    }

    #[test]
    fn test_mul_three_elements() {
        let mut tree = build_powershell_tree("4 * 5 * 10").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), Forward::default(), MultInt::default()))
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
            Raw(Num(200))
        );
    }
}
