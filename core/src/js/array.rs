use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::*;
use crate::js::Value::Bool;
use crate::js::Value::{Num, Str};
use crate::js::b64::js_bytes_to_string;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use log::{trace, warn};

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
            match (left.data(), op.text()?, right.data()) {
                (Some(Array(left_values)), "+", Some(Array(right_values))) => {
                    let combined = combine_arrays(left_values, right_values);
                    trace!(
                        "CombineArrays (L): combining arrays {:?} and {:?} => '{}'",
                        left_values, right_values, combined
                    );
                    node.reduce(Raw(Str(combined)));
                    return Ok(());
                }
                (Some(Array(l)), "-", Some(Raw(Num(r)))) => {
                    let l = flatten_array(l);
                    trace!("Flatten : {}", l);
                    if !l.contains(",") {
                        if let Some(l_num) = l.parse::<f64>().ok() {
                            let result = l_num - *r;
                            trace!("AddInt (L): {} - {} = {}", l, r, result);
                            node.reduce(Raw(Num(result)));
                        } else {
                            node.reduce(NaN);
                        }
                    } else {
                        trace!("AddInt (L): {} - {} = NaN", l, r);
                        node.reduce(NaN);
                    }
                }
                (Some(Raw(Num(l))), "-", Some(Array(r))) => {
                    let r = flatten_array(r);
                    if !r.contains(",") {
                        if let Some(r_num) = r.parse::<f64>().ok() {
                            let result = l - r_num;
                            trace!("AddInt (L): {} - {} = {}", l, r, result);
                            node.reduce(Raw(Num(result)));
                        } else {
                            trace!("AddInt (L): {} - {} = NaN", l, r);
                            node.reduce(NaN);
                        }
                    } else {
                        trace!("AddInt (L): {} - {} = NaN", l, r);
                        node.reduce(NaN);
                    }
                }
                (Some(Array(left_values)), "+", Some(Raw(raw))) => {
                    let combined = format!(
                        "{}{}",
                        flatten_array(left_values),
                        flatten_value(&Raw(raw.clone()))
                    );
                    trace!(
                        "CombineArrays (L): combining array and raw => left: {:?}, right: {:?} => '{}'",
                        left_values, raw, combined
                    );
                    node.reduce(Raw(Str(combined)));
                }
                (Some(Raw(raw)), "+", Some(Array(right_values))) => {
                    let combined = format!(
                        "{}{}",
                        flatten_value(&Raw(raw.clone())),
                        flatten_array(right_values)
                    );
                    trace!(
                        "CombineArrays (L): combining raw and array => left: {:?}, right: {:?} => '{}'",
                        raw, right_values, combined
                    );
                    node.reduce(Raw(Str(combined)));
                }
                _ => {}
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

        // This bypass empty arrays rules
        if let (Some(n), Some(index_node)) = (view.child(0), view.child(2)) {
            if let (Some(_), Some(index)) = (n.data(), index_node.data()) {
                if index == NaN || index == Undefined {
                    trace!("GetArrayElement: accessing {} index => undefined", index);
                    node.reduce(Undefined);
                    return Ok(());
                }
            }
        }

        if let (Some(array_node), Some(index_node)) = (view.child(0), view.child(2)) {
            if let Some(Array(arr)) = array_node.data() {
                if arr.is_empty() {
                    trace!("GetArrayElement: accessing index of empty array, setting to undefined");
                    node.reduce(Undefined);
                    return Ok(());
                }
            }
            if let (Some(Array(arr)), Some(Raw(Num(index)))) =
                (array_node.data(), index_node.data())
            {
                return if (*index as usize) < arr.len() {
                    trace!(
                        "GetArrayElement: accessing index {} of array {:?}",
                        index, arr
                    );
                    node.reduce(arr[*index as usize].clone());
                    Ok(())
                } else {
                    trace!(
                        "GetArrayElement: index {} out of bounds, setting to undefined",
                        index
                    );
                    node.reduce(Undefined);
                    Ok(())
                };
            }
            if let (Some(Array(arr)), Some(Raw(Str(index_str)))) =
                (array_node.data(), index_node.data())
            {
                return if let Ok(index) = index_str.parse::<usize>() {
                    if index < arr.len() {
                        trace!(
                            "GetArrayElement: accessing index '{}' of array {:?} => index {}",
                            index_str, arr, index
                        );
                        node.reduce(arr[index].clone());
                    } else {
                        trace!(
                            "GetArrayElement: index '{}' out of bounds, setting to undefined",
                            index_str
                        );
                        node.reduce(Undefined);
                    }
                    Ok(())
                } else {
                    warn!(
                        "GetArrayElement: cannot parse index '{}' as number",
                        index_str
                    );
                    node.reduce(Undefined);
                    Ok(())
                };
            }
        }

        if let (Some(nan_node), Some(index_node)) = (view.child(0), view.child(2)) {
            if let (Some(NaN), Some(Raw(_))) = (nan_node.data(), index_node.data()) {
                trace!("GetArrayElement: accessing index of non-array, setting to undefined");
                node.reduce(Undefined);
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

        Undefined => String::new(),
        NaN => "NaN".to_string(),
        Bytes(bytes) => js_bytes_to_string(bytes),
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
                        node.reduce(Raw(Num(0.0)));
                    } else if arr.len() == 1 {
                        if let Some(num) = recursive_array_number_extraction(arr) {
                            trace!("ArrayPlusMinus: reducing + {:?} to {}", arr, num);
                            node.reduce(Raw(Num(num)));
                        } else {
                            trace!(
                                "ArrayPlusMinus: Cannot extract number from array {:?}, setting to NaN",
                                arr
                            );
                            node.reduce(NaN);
                        }
                    } else {
                        trace!(
                            "ArrayPlusMinus: Cannot apply unary plus to array with multiple elements, setting to NaN"
                        );
                        node.reduce(NaN);
                    }
                }

                _ => {}
            }
        }

        Ok(())
    }
}

fn recursive_array_number_extraction(arr: &Vec<JavaScript>) -> Option<f64> {
    if arr.len() == 1 {
        match &arr[0] {
            Raw(Num(n)) => Some(*n),
            Raw(Str(s)) => s.parse::<f64>().ok(),
            Array(inner) => recursive_array_number_extraction(inner),
            _ => None,
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests_js_array {
    use super::*;
    use crate::js::build_javascript_tree;
    use crate::js::forward::Forward;
    use crate::js::integer::ParseInt;
    use crate::js::linter::Linter;
    use crate::js::string::ParseString;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseString::default(),
            ParseArray::default(),
            CombineArrays::default(),
            Forward::default(),
            GetArrayElement::default(),
            ArrayPlusMinus::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_array_parsing() {
        assert_eq!(
            deobfuscate("var x = [1, 2, [3, '4']]"),
            "var x = [1, 2, [3, '4']]",
        );
    }

    #[test]
    fn test_combine_arrays() {
        assert_eq!(
            deobfuscate("var x = [0, 1,7] + [3, [7, '2', [88]]]"),
            "var x = '0,1,73,7,2,88'",
        );
    }

    #[test]
    fn test_get_array_element() {
        assert_eq!(
            deobfuscate("var x = ([1, [2, '3'], 4][1])[0];"),
            "var x = 2;",
        );
    }

    #[test]
    fn test_array_plus_minus() {
        assert_eq!(deobfuscate("var x = +[['455']];"), "var x = 455;",);

        assert_eq!(deobfuscate("var x = +['a'];"), "var x = NaN;",);

        assert_eq!(deobfuscate("var x = [8] - 1;"), "var x = 7;",);
    }
}
