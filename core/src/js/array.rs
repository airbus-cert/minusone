use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::{Array, Raw};
use crate::js::Value::Bool;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use js::Value;
use js::Value::{Num, Str};
use log::{debug, trace, warn};
use js::JavaScript::Undefined;

/// Parses JavaScript array literals into `Array(_)`.
#[derive(Default)]
pub struct ParseArray;

impl<'a> RuleMut<'a> for ParseArray {
    type Language = JavaScript;

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
        if view.kind() != "array" {
            return Ok(());
        }

        let mut js = Vec::new();
        for child in view.iter() {
            if let Some(data) = child.data() {
                js.push(data.clone());
            }
        }

        trace!("ParseArray (L): array with {} elements", js.len());
        node.reduce(Array(js));

        Ok(())
    }
}

/// Infers `+` on two arrays
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::integer::ParseInt;
/// use minusone::js::array::{ParseArray, CombineArrays};
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = [1, 2] + [3, 4]").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(), ParseArray::default(), CombineArrays::default()
/// )).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "var x = '1,23,4'");
/// ```

#[derive(Default)]
pub struct CombineArrays;

impl<'a> RuleMut<'a> for CombineArrays {
    type Language = JavaScript;

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
        if view.kind() != "binary_expression" {
            return Ok(());
        }

        if let (Some(left), Some(op), Some(right)) = (view.child(0), view.child(1), view.child(2)) {
            if let (Some(Array(left_values)), "+", Some(Array(right_values))) =
                (left.data(), op.text()?, right.data())
            {
                debug!(
                    "CombineArrays (L): combining arrays => left: {:?}, right: {:?}",
                    left_values, right_values
                );
                let combined = combine_arrays(left_values, right_values);
                trace!("CombineArrays (L): combining arrays => '{}'", combined);
                node.reduce(Raw(Str(combined)));
                return Ok(());
            }
            if let (Some(Array(left_values)), "+", Some(Raw(raw))) =
                (left.data(), op.text()?, right.data())
            {
                debug!(
                    "CombineArrays (L): combining array and raw => left: {:?}, right: {:?}",
                    left_values, raw
                );
                let combined = format!("{}{}", flatten_array(left_values), flatten_value(&Raw(raw.clone())));
                trace!("CombineArrays (L): combining array and raw => '{}'", combined);
                node.reduce(Raw(Str(combined)));
            }
        }

        Ok(())
    }
}

/// Get element at array index
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::string::ParseString;
/// use minusone::js::integer::ParseInt;
/// use minusone::js::array::{ParseArray, GetArrayElement};
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = [1, [2, '3'], 4][1];").unwrap();
/// tree.apply_mut(&mut (
///     ParseString::default(), ParseInt::default(), ParseArray::default(), GetArrayElement::default()
/// )).unwrap();
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
/// assert_eq!(linter.output, "var x = [2, '3'];");
/// ```
#[derive(Default)]
pub struct GetArrayElement;

impl<'a> RuleMut<'a> for GetArrayElement {
    type Language = JavaScript;

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
        if view.kind() != "subscript_expression" {
            return Ok(());
        }

        if let (Some(array_node), Some(index_node)) = (view.child(0), view.child(2)) {
            if let (Some(Array(arr)), Some(Raw(Num(index)))) = (array_node.data(), index_node.data()) {
                if (*index as usize) < arr.len() {
                    trace!("GetArrayElement: accessing index {} of array {:?}", index, arr);
                    node.reduce(arr[*index as usize].clone());
                } else {
                    trace!("GetArrayElement: index {} out of bounds, setting to undefined", index);
                    node.reduce(Undefined);
                }
            }
        }

        Ok(())
    }
}

pub fn flatten_array(arr: &Vec<JavaScript>) -> String {
    arr.iter()
        .map(flatten_value)
        .filter(|s| !s.is_empty())
        .collect::<Vec<String>>()
        .join(",")
}

fn flatten_value(value: &JavaScript) -> String {
    match value {
        Array(arr) => flatten_array(arr),
        Raw(Num(n)) => n.to_string(),
        Raw(Str(s)) => s.clone(),
        Raw(Bool(b)) => b.to_string(),
        _ => {
            warn!("CombineArrays: Unsupported value type");
            String::new()
        }
    }
}

fn combine_arrays(left: &Vec<JavaScript>, right: &Vec<JavaScript>) -> String {
    format!("{}{}", flatten_array(left), flatten_array(right))
}

/// Infers unary plus and minus on arrays
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::string::ParseString;
/// use minusone::js::array::{ParseArray, ArrayPlusMinus};
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = +[['455']];").unwrap();
/// tree.apply_mut(&mut (ParseString::default(), ParseArray::default(), ArrayPlusMinus::default())).unwrap();
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
/// assert_eq!(linter.output, "var x = 455;");
/// ```
#[derive(Default)]
pub struct ArrayPlusMinus;

impl<'a> RuleMut<'a> for ArrayPlusMinus {
    type Language = JavaScript;

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
        if view.kind() != "unary_expression" {
            return Ok(());
        }

        if let (Some(operator), Some(operand)) = (view.child(0), view.child(1)) {
            match (operator.text()?, operand.data()) {
                ("+", Some(Array(arr))) => {
                    if arr.is_empty() {
                        node.reduce(Raw(Num(0)));
                    } else if arr.len() == 1 {
                        if let Some(num) = recursive_array_number_extraction(arr) {
                            trace!("ArrayPlusMinus: reducing + {:?} to {}", arr, num);
                            node.reduce(Raw(Num(num)));
                        } else {
                            warn!("ArrayPlusMinus: Cannot parse array {:?} as number", arr);
                        }
                    } else {
                        warn!("ArrayPlusMinus: Cannot apply unary plus to array with multiple elements");
                    }
                }

                _ => {}
            }
        }

        Ok(())
    }
}

fn recursive_array_number_extraction(arr: &Vec<JavaScript>) -> Option<i64> {
    if arr.len() == 1 {
        match &arr[0] {
            Raw(Num(n)) => Some(*n),
            Raw(Str(s)) => s.parse::<i64>().ok(),
            Array(inner) => recursive_array_number_extraction(inner),
            _ => None,
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests_js_array {
    use js::string::ParseString;
    use super::*;
    use crate::js::build_javascript_tree;
    use crate::js::integer::ParseInt;
    use crate::js::linter::Linter;

    #[test]
    fn test_combine_arrays() {
        let mut tree = build_javascript_tree("var x = [0, 1,7] + [3, [7, '2', [88]]]").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(), ParseString::default(), ParseArray::default(), CombineArrays::default()
        )).unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();

        assert_eq!(linter.output, "var x = '0,1,73,7,2,88'");
    }
}