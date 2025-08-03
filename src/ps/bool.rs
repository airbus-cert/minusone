use crate::error::MinusOneResult;
use crate::ps::Powershell;
use crate::ps::Powershell::Raw;
use crate::ps::Value::{Bool, Num, Str};
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};

/// This rule will infer boolean variable $true $false
///
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
/// use minusone::ps::bool::{ParseBool, BoolAlgebra};
///
/// let mut tree = build_powershell_tree("$true -or $false").unwrap();
/// tree.apply_mut(&mut (
///     ParseBool::default(),
///     Forward::default(),
///     BoolAlgebra::default(),
/// )).unwrap();
///
/// let mut ps_litter_view = Linter::new();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "$true");
/// ```
#[derive(Default)]
pub struct ParseBool;

impl<'a> RuleMut<'a> for ParseBool {
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
        // Booleans in powershell are variables
        if view.kind() == "variable" {
            match view.text()?.to_lowercase().as_str() {
                "$true" => node.set(Raw(Bool(true))),
                "$false" => node.set(Raw(Bool(false))),
                _ => (),
            }
        }
        Ok(())
    }
}

/// This rule will infer boolean algebra involve -or and -and operator
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
/// use minusone::ps::bool::{ParseBool, BoolAlgebra};
///
/// let mut tree = build_powershell_tree("$true -and $false").unwrap();
/// tree.apply_mut(&mut (
///     ParseBool::default(),
///     Forward::default(),
///     BoolAlgebra::default(),
/// )).unwrap();
///
/// let mut ps_litter_view = Linter::new();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "$false");
/// ```
#[derive(Default)]
pub struct BoolAlgebra;

impl<'a> RuleMut<'a> for BoolAlgebra {
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
        // Booleans in powershell are variables
        if view.kind() == "logical_expression" {
            if let (Some(left_node), Some(operator), Some(right_node)) =
                (view.child(0), view.child(1), view.child(2))
            {
                match (
                    left_node.data(),
                    operator.text()?.to_lowercase().as_str(),
                    right_node.data(),
                ) {
                    (Some(Raw(Bool(left_value))), "-or", Some(Raw(Bool(right_value)))) => {
                        node.set(Raw(Bool(*left_value || *right_value)))
                    }
                    (Some(Raw(Bool(left_value))), "-and", Some(Raw(Bool(right_value)))) => {
                        node.set(Raw(Bool(*left_value && *right_value)))
                    }
                    _ => (),
                }
            }
        }
        Ok(())
    }
}

/// This rule will infer boolean comparaise involve integer or $null operator
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
/// use minusone::ps::bool::{Comparison};
/// use minusone::ps::integer::ParseInt;
/// use minusone::ps::string::ParseString;
///
/// let mut tree = build_powershell_tree("4 -le '5'").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(),
///     ParseString::default(),
///     Forward::default(),
///     Comparison::default(),
/// )).unwrap();
///
/// let mut ps_litter_view = Linter::new();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "$true");
/// ```
#[derive(Default)]
pub struct Comparison;

impl<'a> RuleMut<'a> for Comparison {
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
        // Booleans in powershell are variables
        if view.kind() == "comparison_expression" {
            if let (Some(left_node), Some(operator), Some(right_node)) =
                (view.child(0), view.child(1), view.child(2))
            {
                match (
                    left_node.data(),
                    operator.text()?.to_lowercase().as_str(),
                    right_node.data(),
                ) {
                    // String comparison
                    (Some(Raw(Str(left_value))), "-eq", Some(Raw(Str(right_value)))) => {
                        node.set(Raw(Bool(left_value == right_value)))
                    }
                    (Some(Raw(Str(left_value))), "-ne", Some(Raw(Str(right_value)))) => {
                        node.set(Raw(Bool(left_value != right_value)))
                    }
                    (Some(Raw(Str(left_value))), "-ge", Some(Raw(Str(right_value)))) => {
                        node.set(Raw(Bool(left_value >= right_value)))
                    }
                    (Some(Raw(Str(left_value))), "-gt", Some(Raw(Str(right_value)))) => {
                        node.set(Raw(Bool(left_value > right_value)))
                    }
                    (Some(Raw(Str(left_value))), "-le", Some(Raw(Str(right_value)))) => {
                        node.set(Raw(Bool(left_value <= right_value)))
                    }
                    (Some(Raw(Str(left_value))), "-lt", Some(Raw(Str(right_value)))) => {
                        node.set(Raw(Bool(left_value < right_value)))
                    }

                    // Integer comparison
                    (Some(Raw(Num(left_value))), "-eq", Some(Raw(Num(right_value)))) => {
                        node.set(Raw(Bool(left_value == right_value)))
                    }
                    (Some(Raw(Num(left_value))), "-ne", Some(Raw(Num(right_value)))) => {
                        node.set(Raw(Bool(left_value != right_value)))
                    }
                    (Some(Raw(Num(left_value))), "-ge", Some(Raw(Num(right_value)))) => {
                        node.set(Raw(Bool(left_value >= right_value)))
                    }
                    (Some(Raw(Num(left_value))), "-gt", Some(Raw(Num(right_value)))) => {
                        node.set(Raw(Bool(left_value > right_value)))
                    }
                    (Some(Raw(Num(left_value))), "-le", Some(Raw(Num(right_value)))) => {
                        node.set(Raw(Bool(left_value <= right_value)))
                    }
                    (Some(Raw(Num(left_value))), "-lt", Some(Raw(Num(right_value)))) => {
                        node.set(Raw(Bool(left_value < right_value)))
                    }

                    // Boolean comparison
                    // Seems to be standardized with Rust???
                    (Some(Raw(Bool(left_value))), "-eq", Some(Raw(Bool(right_value)))) => {
                        node.set(Raw(Bool(left_value == right_value)))
                    }
                    (Some(Raw(Bool(left_value))), "-ne", Some(Raw(Bool(right_value)))) => {
                        node.set(Raw(Bool(left_value != right_value)))
                    }
                    (Some(Raw(Bool(left_value))), "-ge", Some(Raw(Bool(right_value)))) => {
                        node.set(Raw(Bool(left_value >= right_value)))
                    }
                    (Some(Raw(Bool(left_value))), "-gt", Some(Raw(Bool(right_value)))) => {
                        node.set(Raw(Bool(left_value > right_value)))
                    }
                    (Some(Raw(Bool(left_value))), "-le", Some(Raw(Bool(right_value)))) => {
                        node.set(Raw(Bool(left_value <= right_value)))
                    }
                    (Some(Raw(Bool(left_value))), "-lt", Some(Raw(Bool(right_value)))) => {
                        node.set(Raw(Bool(left_value < right_value)))
                    }

                    // Mixed type comparison
                    // Str and bool comparison
                    (Some(Raw(Str(left_value))), "-eq", Some(Raw(Bool(right_value)))) => {
                        node.set(Raw(Bool(
                            (left_value.to_lowercase() == "true" && *right_value == true)
                                || (left_value.to_lowercase() == "false" && *right_value == false),
                        )))
                    }
                    (Some(Raw(Bool(left_value))), "-eq", Some(Raw(Str(right_value)))) => {
                        node.set(Raw(Bool(
                            (!right_value.is_empty() && *left_value)
                                || (right_value.is_empty() && !*left_value),
                        )))
                    }
                    (Some(Raw(Str(left_value))), "-ne", Some(Raw(Bool(right_value)))) => {
                        node.set(Raw(Bool(
                            !((left_value.to_lowercase() == "true" && *right_value == true)
                                || (left_value.to_lowercase() == "false" && *right_value == false)),
                        )))
                    }
                    (Some(Raw(Bool(left_value))), "-ne", Some(Raw(Str(right_value)))) => {
                        node.set(Raw(Bool(
                            !((!right_value.is_empty() && *left_value)
                                || (right_value.is_empty() && !*left_value)),
                        )))
                    }

                    // true or false compare to string
                    (Some(Raw(Bool(true))), "-gt", Some(Raw(Str(right_value)))) => {
                        node.set(Raw(Bool(right_value.is_empty())))
                    }
                    (Some(Raw(Bool(true))), "-ge", Some(Raw(Str(_)))) => node.set(Raw(Bool(true))),
                    (Some(Raw(Bool(false))), "-gt", Some(Raw(_))) => node.set(Raw(Bool(false))),
                    (Some(Raw(Bool(false))), "-ge", Some(Raw(Str(right_value)))) => {
                        node.set(Raw(Bool(right_value.len() == 0)))
                    }

                    // String to number comparison
                    (Some(Raw(Str(left_value))), "-eq", Some(Raw(Num(right_value)))) => {
                        node.set(Raw(Bool(*left_value == right_value.to_string())))
                    }
                    (Some(Raw(Str(left_value))), "-ne", Some(Raw(Num(right_value)))) => {
                        node.set(Raw(Bool(*left_value != right_value.to_string())))
                    }
                    (Some(Raw(Str(left_value))), "-ge", Some(Raw(Num(right_value)))) => {
                        node.set(Raw(Bool(*left_value >= right_value.to_string())))
                    }
                    (Some(Raw(Str(left_value))), "-gt", Some(Raw(Num(right_value)))) => {
                        node.set(Raw(Bool(*left_value > right_value.to_string())))
                    }
                    (Some(Raw(Str(left_value))), "-le", Some(Raw(Num(right_value)))) => {
                        node.set(Raw(Bool(*left_value <= right_value.to_string())))
                    }
                    (Some(Raw(Str(left_value))), "-lt", Some(Raw(Num(right_value)))) => {
                        node.set(Raw(Bool(*left_value < right_value.to_string())))
                    }

                    // number to string comparison
                    (Some(Raw(Num(left_value))), "-eq", Some(Raw(Str(right_value)))) => {
                        node.set(Raw(Bool(left_value.to_string() == *right_value)))
                    }
                    (Some(Raw(Num(left_value))), "-ne", Some(Raw(Str(right_value)))) => {
                        node.set(Raw(Bool(left_value.to_string() != *right_value)))
                    }
                    (Some(Raw(Num(left_value))), "-ge", Some(Raw(Str(right_value)))) => {
                        node.set(Raw(Bool(left_value.to_string() >= *right_value)))
                    }
                    (Some(Raw(Num(left_value))), "-gt", Some(Raw(Str(right_value)))) => {
                        node.set(Raw(Bool(left_value.to_string() > *right_value)))
                    }
                    (Some(Raw(Num(left_value))), "-le", Some(Raw(Str(right_value)))) => {
                        node.set(Raw(Bool(left_value.to_string() <= *right_value)))
                    }
                    (Some(Raw(Num(left_value))), "-lt", Some(Raw(Str(right_value)))) => {
                        node.set(Raw(Bool(left_value.to_string() < *right_value)))
                    }

                    _ => (),
                }
            }
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct Not;

impl<'a> RuleMut<'a> for Not {
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
        if node_view.kind() == "expression_with_unary_operator" {
            if let (Some(operator), Some(expression)) = (node_view.child(0), node_view.child(1)) {
                if let ("!", Some(Raw(Bool(b)))) = (operator.text()?, expression.data()) {
                    node.set(Raw(Bool(!(*b))));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::ps::bool::{BoolAlgebra, Comparison, ParseBool};
    use crate::ps::build_powershell_tree;
    use crate::ps::forward::Forward;
    use crate::ps::integer::ParseInt;
    use crate::ps::string::ParseString;
    use crate::ps::Powershell::Raw;
    use crate::ps::Value::Bool;

    #[test]
    fn test_parse_bool_true() {
        let mut tree = build_powershell_tree("$true").unwrap();
        tree.apply_mut(&mut (ParseBool::default(), Forward::default()))
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
            Raw(Bool(true))
        );
    }

    #[test]
    fn test_parse_bool_false() {
        let mut tree = build_powershell_tree("$false").unwrap();
        tree.apply_mut(&mut (ParseBool::default(), Forward::default()))
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
            Raw(Bool(false))
        );
    }

    #[test]
    fn test_boolean_algebra_or() {
        let mut tree = build_powershell_tree("$true -or $false").unwrap();
        tree.apply_mut(&mut (
            ParseBool::default(),
            Forward::default(),
            BoolAlgebra::default(),
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
            Raw(Bool(true))
        );
    }

    #[test]
    fn test_boolean_algebra_and() {
        let mut tree = build_powershell_tree("$true -and $false").unwrap();
        tree.apply_mut(&mut (
            ParseBool::default(),
            Forward::default(),
            BoolAlgebra::default(),
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
            Raw(Bool(false))
        );
    }

    #[test]
    fn test_comparison_int_int() {
        let mut tree = build_powershell_tree("4 -le 5").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            Comparison::default(),
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
            Raw(Bool(true))
        );
    }

    #[test]
    fn test_comparison_int_str() {
        let mut tree = build_powershell_tree("4 -le '5'").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseString::default(),
            Forward::default(),
            Comparison::default(),
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
            Raw(Bool(true))
        );
    }

    #[test]
    fn test_comparison_special_case_1() {
        let mut tree = build_powershell_tree("'True' -eq $true").unwrap();
        tree.apply_mut(&mut (
            ParseBool::default(),
            ParseString::default(),
            Forward::default(),
            Comparison::default(),
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
            Raw(Bool(true))
        );
    }

    #[test]
    fn test_comparison_special_case_2() {
        let mut tree = build_powershell_tree("'False' -eq $false").unwrap();
        tree.apply_mut(&mut (
            ParseBool::default(),
            ParseString::default(),
            Forward::default(),
            Comparison::default(),
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
            Raw(Bool(true))
        );
    }

    #[test]
    fn test_comparison_special_case_3() {
        let mut tree = build_powershell_tree("'' -eq $true").unwrap();
        tree.apply_mut(&mut (
            ParseBool::default(),
            ParseString::default(),
            Forward::default(),
            Comparison::default(),
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
            Raw(Bool(false))
        );
    }

    #[test]
    fn test_comparison_special_case_4() {
        let mut tree = build_powershell_tree("'' -eq $false").unwrap();
        tree.apply_mut(&mut (
            ParseBool::default(),
            ParseString::default(),
            Forward::default(),
            Comparison::default(),
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
            Raw(Bool(false))
        );
    }

    #[test]
    fn test_comparison_special_case_5() {
        let mut tree = build_powershell_tree("$false -eq ''").unwrap();
        tree.apply_mut(&mut (
            ParseBool::default(),
            ParseString::default(),
            Forward::default(),
            Comparison::default(),
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
            Raw(Bool(true))
        );
    }
}
