use crate::error::{Error, MinusOneError, MinusOneErrorKind, MinusOneResult};
use crate::ps::Powershell;
use crate::ps::Powershell::{Array, Raw};
use crate::ps::Value::Num;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};

/// Parse array literal
///
/// It will parse 1,2,3,"5" as an array for powershell
///
/// # Example
/// ```
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
/// let mut ps_litter_view = Linter::default();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"abc\"");
/// ```
#[derive(Default)]
pub struct ParseArrayLiteral;

impl<'a> RuleMut<'a> for ParseArrayLiteral {
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
        if view.kind() == "array_literal_expression" && view.child_count() > 1 {
            let mut range = vec![];
            for child in view.iter() {
                if let Some(Raw(value)) = child.data() {
                    range.push(value.clone());
                } else if child.kind() != "," {
                    return Ok(());
                }
            }
            node.set(Array(range));
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
/// let mut ps_litter_view = Linter::default();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"abc\"");
/// ```
#[derive(Default)]
pub struct ParseRange;

impl<'a> RuleMut<'a> for ParseRange {
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
        if view.kind() == "range_expression" {
            if let (Some(left_node), Some(right_node)) = (view.child(0), view.child(2)) {
                if let (Some(Raw(left_value)), Some(Raw(right_value))) =
                    (left_node.data(), right_node.data())
                {
                    if let (Some(from), Some(to)) =
                        (left_value.clone().to_i64(), right_value.clone().to_i64())
                    {
                        let mut result = Vec::new();

                        let mut index = from;
                        let end = if from <= to { to + 1 } else { to - 1 };

                        while index != end {
                            result.push(Num(index));
                            if from <= to {
                                index += 1
                            } else {
                                index -= 1
                            }
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
/// let mut ps_litter_view = Linter::default();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"abc\"");
/// ```
#[derive(Default)]
pub struct ComputeArrayExpr;

impl<'a> RuleMut<'a> for ComputeArrayExpr {
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
        if view.kind() == "array_expression" {
            if let Some(statement_list) = view.named_child("statements") {
                let mut result = Vec::new();
                for statement in statement_list.iter() {
                    if statement.kind() == "empty_statement" {
                        continue;
                    }
                    match statement.data() {
                        Some(Array(values)) => {
                            result.extend(values.clone());
                        }
                        Some(Raw(value)) => {
                            result.push(value.clone());
                        }
                        _ => {
                            // stop inferring
                            return Ok(());
                        }
                    }
                }
                node.reduce(Array(result));
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
/// let mut ps_litter_view = Linter::default();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"fox\"");
/// ```
#[derive(Default)]
pub struct AddArray;

impl<'a> RuleMut<'a> for AddArray {
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
                    (Some(Array(array)), "+", Some(Raw(v))) => {
                        let mut new_array = array.clone();
                        new_array.push(v.clone());
                        node.reduce(Array(new_array));
                    }
                    // add array between both
                    (Some(Array(right_array)), "+", Some(Array(left_array))) => {
                        node.reduce(Array([right_array.clone(), left_array.clone()].concat()));
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

/// This rule will generate an array resulting from a `New-Object` command invocation.
///
/// # Example
/// ```
/// extern crate tree_sitter;
///
/// use minusone::ps::build_powershell_tree;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::integer::ParseInt;
/// use minusone::ps::array::NewObjectArray;
/// use minusone::ps::Value::Num;
/// use minusone::ps::Powershell::Array;
///
/// let mut tree = build_powershell_tree("New-Object byte[] 16").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(),
///     Forward::default(),
///     NewObjectArray::default(),
///     )
/// ).unwrap();
///
/// assert_eq!(
///     tree.root()
///     .unwrap()
///     .child(0)
///     .unwrap()
///     .child(0)
///     .unwrap()
///     .data(),
///     Some(&Array(vec![Num(0); 16]))
/// );
/// ```
#[derive(Default, Debug, Clone)]
pub struct NewObjectArray {
    max_size: Option<usize>,
}

impl NewObjectArray {
    /// Returns a new [`NewObjectArray`] with no maximum capacity. Untrusted script input can cause
    /// unbounded vec allocation.
    pub fn new() -> Self {
        Self { max_size: None }
    }

    /// Returns a [`NewObjectArray`] with a maximum capacity. Will infer array data no larger than
    /// this upper bound. Invocations exceeding this value will cause an error to be returned
    /// from [`RuleMut::leave`].
    pub fn with_max_capacity(max_size: usize) -> Self {
        Self {
            max_size: Some(max_size),
        }
    }
}

impl<'a> RuleMut<'a> for NewObjectArray {
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

        if view.kind() != "command" || view.child_count() != 2 {
            return Ok(());
        }

        if let (Some(command_name), Some(command_elements)) = (
            view.named_child("command_name"),
            view.named_child("command_elements"),
        ) {
            if command_name
                .text()
                .is_ok_and(|name| name.to_lowercase() == "new-object")
                && command_elements.child_count() == 4
                && command_elements
                    .child(0)
                    .is_some_and(|c| c.kind() == "command_argument_sep")
                && command_elements.child(1).is_some_and(|c1| {
                    c1.kind() == "generic_token"
                        && matches!(c1.text(), Ok(t) if ["byte[]", "int[]"].iter().any(|typ| **typ == t.to_lowercase()))
                })
                && command_elements
                    .child(2)
                    .is_some_and(|c2| c2.kind() == "command_argument_sep")
                && command_elements
                    .child(3)
                    .filter(|c| c.data().is_some())
                    .is_some()
            {
                if let Some(Raw(Num(size))) =
                    command_elements.child(3).as_ref().and_then(|c| c.data())
                {
                    if matches!(self.max_size, Some(max_size) if (*size) as usize > max_size) {
                        return Err(Error::MinusOneError(
                            MinusOneError::new(
                                MinusOneErrorKind::InvalidProgram,
                                &format!("Array of length {} exceeds maximum array length", size)
                            )
                        ));
                    }
                    node.set(Array(vec![
                        Num(0);
                        std::cmp::max(
                            (*size) as usize,
                            self.max_size.unwrap_or_default()
                        )
                    ]));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::ps::access::AccessString;
    use crate::ps::array::{
        AddArray, ComputeArrayExpr, NewObjectArray, ParseArrayLiteral, ParseRange,
    };
    use crate::ps::build_powershell_tree;
    use crate::ps::forward::Forward;
    use crate::ps::integer::{AddInt, ParseInt};
    use crate::ps::string::ParseString;
    use crate::ps::Powershell::Array;
    use crate::ps::Value::{Num, Str};

    #[test]
    fn test_init_num_array() {
        let mut tree = build_powershell_tree("@(1,2,3)").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ComputeArrayExpr::default(),
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
            Array(vec![Num(1), Num(2), Num(3)])
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
            Array(vec![Num(1), Num(2), Str("3".to_string())])
        );
    }

    #[test]
    fn test_init_str_array() {
        let mut tree = build_powershell_tree("@('a','b','c')").unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            Forward::default(),
            ComputeArrayExpr::default(),
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
            Array(vec![
                Str("a".to_string()),
                Str("b".to_string()),
                Str("c".to_string())
            ])
        );
    }

    #[test]
    fn test_init_int_array_without_at() {
        let mut tree = build_powershell_tree("1,2,3").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ComputeArrayExpr::default(),
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
            Array(vec![Num(1), Num(2), Num(3)])
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
            Array(vec![Num(1), Num(2), Num(3), Num(10)])
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
            ParseString::default(),
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
                Str("f".to_string()),
                Str("o".to_string()),
                Str("x".to_string())
            ])
        );
    }

    #[test]
    fn test_negative_range() {
        let mut tree = build_powershell_tree("-1..-3").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ComputeArrayExpr::default(),
            ParseArrayLiteral::default(),
            AccessString::default(),
            ParseString::default(),
            ParseRange::default(),
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
            Array(vec![Num(-1), Num(-2), Num(-3)])
        );
    }

    #[test]
    fn test_new_object_array() {
        let mut tree = build_powershell_tree("New-Object byte[] 16").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), Forward::default()))
            .unwrap();
        tree.apply_mut(&mut NewObjectArray::default()).unwrap();

        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data()
                .expect("Inferred data"),
            Array(vec![Num(0); 16]),
        );
    }
}
