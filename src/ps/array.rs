use rule::RuleMut;
use ps::Powershell;
use tree::{NodeMut, BranchFlow};
use error::MinusOneResult;
use ps::Powershell::{Raw, Array};
use ps::Value::Num;

/// Parse array literal
///
/// It will parse 1,2,3,"5" as an array for powershell
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
/// use minusone::ps::access::AccessString;
/// use minusone::ps::join::JoinOperator;
/// use minusone::ps::array::ParseArrayLiteral;
///
/// let mut tree = build_powershell_tree("-join ('a','b','c')").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(),
///     Forward::default(),
///     ParseString::default(),
///     ParseArrayLiteral::default(),
///     JoinOperator::default()
///     )
/// ).unwrap();
///
/// let mut ps_litter_view = Linter::new();
/// ps_litter_view.print(&tree.root().unwrap()).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"abc\"");
/// ```
#[derive(Default)]
pub struct ParseArrayLiteral;

impl<'a> RuleMut<'a> for ParseArrayLiteral {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()> {
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()> {
        let view = node.view();
        if view.kind() == "array_literal_expression" {
            if let (Some(left_node), Some(right_node)) = (view.child(0), view.child(2)) {
                match (left_node.data(), right_node.data()) {
                    // Case when we are not beginning to built the range
                    (Some(Raw(left_value)), Some(Raw(right_value))) => node.set(Array(vec![left_value.clone(), right_value.clone()])),
                    // update an existing array
                    (Some(Array(left_value)), Some(Raw(right_value))) => {
                        let mut new_range = left_value.clone();
                        new_range.push(right_value.clone());
                        node.set(Array(new_range))
                    }
                    _ => ()
                }
            }
        }
        Ok(())
    }
}

/// This rule will generate
/// a range value from operator ..
///
/// # Example
/// ```
/// extern crate tree_sitter;
/// extern crate minusone;
///
/// use minusone::ps::build_powershell_tree;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::integer::ParseInt;
/// use minusone::ps::linter::Linter;
/// use minusone::ps::string::ParseString;
/// use minusone::ps::access::AccessString;
/// use minusone::ps::join::JoinOperator;
/// use minusone::ps::array::ParseRange;
///
/// let mut tree = build_powershell_tree("-join \"abc\"[0..2]").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(),
///     Forward::default(),
///     ParseRange::default(),
///     ParseString::default(),
///     JoinOperator::default(),
///     AccessString::default()
///     )
/// ).unwrap();
///
/// let mut ps_litter_view = Linter::new();
/// ps_litter_view.print(&tree.root().unwrap()).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"abc\"");
/// ```
#[derive(Default)]
pub struct ParseRange;

impl<'a> RuleMut<'a> for ParseRange {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()> {
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()> {
        let view = node.view();
        if view.kind() == "range_expression" {
            if let (Some(left_node), Some(right_node)) = (view.child(0), view.child(2)) {
                if let (Some(Raw(left_value)), Some(Raw(right_value))) = (left_node.data(), right_node.data()) {
                    if let (Some(from), Some(to)) = (left_value.clone().to_i32(), right_value.clone().to_i32()) {
                        let mut result = Vec::new();
                        for i in from .. to + 1 {
                            result.push(Num(i));
                        }
                        node.set(Array(result));
                    }
                }
            }
        }
        Ok(())
    }
}


/// This rule will handle array decared using @ operator
///
/// @(1, 2; 2+1)
///
/// # Example
/// ```
/// extern crate tree_sitter;
/// extern crate tree_sitter_powershell;
/// extern crate minusone;
///
/// use minusone::ps::build_powershell_tree;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::integer::{ParseInt, AddInt};
/// use minusone::ps::linter::Linter;
/// use minusone::ps::string::ParseString;
/// use minusone::ps::access::AccessString;
/// use minusone::ps::join::JoinOperator;
/// use minusone::ps::array::{ComputeArrayExpr, ParseArrayLiteral};
///
/// let mut tree = build_powershell_tree("-join \"abc\"[@(0, 1; 1+1)]").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(),
///     AddInt::default(),
///     Forward::default(),
///     ComputeArrayExpr::default(),
///     ParseString::default(),
///     JoinOperator::default(),
///     AccessString::default(),
///     ParseArrayLiteral::default()
///     )
/// ).unwrap();
///
/// let mut ps_litter_view = Linter::new();
/// ps_litter_view.print(&tree.root().unwrap()).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"abc\"");
/// ```
#[derive(Default)]
pub struct ComputeArrayExpr;

impl<'a> RuleMut<'a> for ComputeArrayExpr {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()> {
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()> {
        let view = node.view();
        if view.kind() == "array_expression" {
            if let Some(statement_list) = view.named_child("statements") {
                let mut result = Vec::new();
                for statement in statement_list.iter() {
                    if statement.kind() == "empty_statement" {
                        continue
                    }
                    match statement.data() {
                        Some(Array(values)) => {
                            result.extend(values.clone());
                        },
                        Some(Raw(value)) => {
                            result.push(value.clone());
                        }
                        _ => {
                            // stop inferring
                            return Ok(());
                        }
                    }
                }
                node.set(Array(result));
            }
        }
        Ok(())
    }
}

/// This rule will array concat using + operator
///
/// "foo"[0,1] + 'x' => ['f', 'o', 'x']
///
/// # Example
/// ```
/// extern crate tree_sitter;
/// extern crate tree_sitter_powershell;
/// extern crate minusone;
///
/// use minusone::ps::build_powershell_tree;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::integer::{ParseInt, AddInt};
/// use minusone::ps::linter::Linter;
/// use minusone::ps::string::ParseString;
/// use minusone::ps::access::AccessString;
/// use minusone::ps::join::JoinOperator;
/// use minusone::ps::array::{ComputeArrayExpr, ParseArrayLiteral, AddArray};
///
/// let mut tree = build_powershell_tree("-join ('foo'[0,1] + 'x')").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(),
///     Forward::default(),
///     ComputeArrayExpr::default(),
///     ParseString::default(),
///     JoinOperator::default(),
///     AccessString::default(),
///     ParseArrayLiteral::default(),
///     AddArray::default()
///     )
/// ).unwrap();
///
/// let mut ps_litter_view = Linter::new();
/// ps_litter_view.print(&tree.root().unwrap()).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"fox\"");
/// ```
#[derive(Default)]
pub struct AddArray;

impl<'a> RuleMut<'a> for AddArray {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()>{
        let node_view = node.view();
        if node_view.kind() == "additive_expression" || node_view.kind() == "additive_argument_expression" {
            if let (Some(left_op), Some(operator), Some(right_op)) = (node_view.child(0), node_view.child(1), node_view.child(2)) {
                match (left_op.data(), operator.text()?, right_op.data()) {
                    (Some(Array(array)), "+", Some(Raw(v))) => {
                        let mut new_array = array.clone();
                        new_array.push(v.clone());
                        node.set(Array(new_array));
                    },
                    _ => {}
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
    use ps::array::{ParseArrayLiteral, ComputeArrayExpr, AddArray};
    use ps::Powershell::Array;
    use ps::Value::{Num, Str};
    use ps::string::ParseString;
    use ps::access::AccessString;

    #[test]
    fn test_init_num_array() {
        let mut tree = build_powershell_tree("@(1,2,3)").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ComputeArrayExpr::default(),
            ParseArrayLiteral::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Num(1), Num(2), Num(3)])
        );
    }

    #[test]
    fn test_init_mix_array() {
        let mut tree = build_powershell_tree("@(1,2,'3')").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseString::default(),
            Forward::default(),
            ComputeArrayExpr::default(),
            ParseArrayLiteral::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Num(1), Num(2), Str("3".to_string())])
        );
    }

    #[test]
    fn test_init_str_array() {
        let mut tree = build_powershell_tree("@('a','b','c')").unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            Forward::default(),
            ComputeArrayExpr::default(),
            ParseArrayLiteral::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Str("a".to_string()), Str("b".to_string()), Str("c".to_string())])
        );
    }

    #[test]
    fn test_init_int_array_without_at() {
        let mut tree = build_powershell_tree("1,2,3").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ComputeArrayExpr::default(),
            ParseArrayLiteral::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Num(1), Num(2), Num(3)])
        );
    }

    #[test]
    fn test_init_array_with_multi_statement() {
        let mut tree = build_powershell_tree("@(1,2,3; 4 + 6)").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            AddInt::default(),
            Forward::default(),
            ComputeArrayExpr::default(),
            ParseArrayLiteral::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Num(1), Num(2), Num(3), Num(10)])
        );
    }

    #[test]
    fn test_concat_array() {
        let mut tree = build_powershell_tree("'foo'[0,1] + 'x'").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            AddArray::default(),
            Forward::default(),
            ComputeArrayExpr::default(),
            ParseArrayLiteral::default(),
            AccessString::default(),
            ParseString::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Str("f".to_string()), Str("o".to_string()), Str("x".to_string())])
        );
    }
}