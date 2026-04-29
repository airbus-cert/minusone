use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::*;
use crate::js::Value::Bool;
use crate::js::Value::{Num, Str};
use crate::js::b64::js_bytes_to_string;
use crate::js::utils::{get_positional_arguments, method_name};
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
        let mut expect_value = true;
        for child in view.iter() {
            if let Ok(text) = child.text() {
                match text {
                    "[" => {
                        expect_value = true;
                        continue;
                    }
                    "]" => break,
                    "," => {
                        if expect_value {
                            js.push(Undefined);
                        }
                        expect_value = true;
                        continue;
                    }
                    _ => {}
                }
            }

            if let Some(data) = child.data() {
                js.push(data.clone());
                expect_value = false;
            } else {
                warn!(
                    "ParseArray: unable to parse array element {:?}",
                    child.text()
                );
                return Ok(());
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
                (
                    Some(Array(left_values)),
                    "+",
                    Some(Object {
                        to_string_override: Some(obj_str),
                        ..
                    }),
                ) => {
                    let combined = format!("{}{}", flatten_array(left_values, None), obj_str);
                    trace!(
                        "CombineArrays (L): combining array and object => left: {:?}, right: {:?} => '{}'",
                        left_values, obj_str, combined
                    );
                    node.reduce(Raw(Str(combined)));
                }
                (
                    Some(Object {
                        to_string_override: Some(obj_str),
                        ..
                    }),
                    "+",
                    Some(Array(right_values)),
                ) => {
                    let combined = format!("{}{}", obj_str, flatten_array(right_values, None));
                    trace!(
                        "CombineArrays (L): combining object and array => left: {:?}, right: {:?} => '{}'",
                        obj_str, right_values, combined
                    );
                    node.reduce(Raw(Str(combined)));
                }
                (Some(Array(left_values)), "+", Some(Raw(raw))) => {
                    let combined = format!(
                        "{}{}",
                        flatten_array(left_values, None),
                        flatten_value(&Raw(raw.clone()), None)
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
                        flatten_value(&Raw(raw.clone()), None),
                        flatten_array(right_values, None)
                    );
                    trace!(
                        "CombineArrays (L): combining raw and array => left: {:?}, right: {:?} => '{}'",
                        raw, right_values, combined
                    );
                    node.reduce(Raw(Str(combined)));
                }
                (Some(Array(left_values)), "+", Some(javascript)) => {
                    let combined = format!(
                        "{}{}",
                        flatten_array(left_values, None),
                        match javascript {
                            Raw(Str(s)) => s.clone(),
                            Array(a) => flatten_array(a, None),
                            any => any.to_string(),
                        }
                    );
                    trace!(
                        "CombineArrays (L): combining array and non-raw => left: {:?}, right: {:?} => '{}'",
                        left_values, javascript, combined
                    );
                    node.reduce(Raw(Str(combined)));
                }
                (Some(javascript), "+", Some(Array(right_values))) => {
                    let combined = format!(
                        "{}{}",
                        match javascript {
                            Raw(Str(s)) => s.clone(),
                            Array(a) => flatten_array(a, None),
                            any => any.to_string(),
                        },
                        flatten_array(right_values, None)
                    );
                    trace!(
                        "CombineArrays (L): combining non-raw and array => left: {:?}, right: {:?} => '{}'",
                        javascript, right_values, combined
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

impl GetArrayElement {
    fn is_call_like_target(view: &crate::tree::Node<JavaScript>) -> bool {
        if let Some(parent) = view.parent() {
            if parent.kind() == "call_expression"
                && parent
                    .named_child("function")
                    .map(|f| f.start_abs() == view.start_abs() && f.end_abs() == view.end_abs())
                    .unwrap_or(false)
            {
                return true;
            }

            if parent.kind() == "new_expression"
                && parent
                    .named_child("constructor")
                    .map(|f| f.start_abs() == view.start_abs() && f.end_abs() == view.end_abs())
                    .unwrap_or(false)
            {
                return true;
            }
        }

        false
    }
}

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

        if Self::is_call_like_target(&view) {
            return Ok(());
        }

        // bypass empty arrays rules
        if let (Some(n), Some(index_node)) = (view.child(0), view.child(2))
            && let (Some(_), Some(index)) = (n.data(), index_node.data())
            && (index == NaN || index == Undefined)
        {
            trace!("GetArrayElement: accessing {} index => undefined", index);
            node.reduce(Undefined);
            return Ok(());
        }

        if let (Some(array_node), Some(index_node)) = (view.child(0), view.child(2)) {
            if let (Some(Array(_)), Some(Array(index_arr))) = (array_node.data(), index_node.data())
                && index_arr.is_empty()
            {
                trace!(
                    "GetArrayElement: array indexed by [] coerces to empty-string key => undefined"
                );
                node.reduce(Undefined);
                return Ok(());
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
                    if index_str == "constructor" || index_str == "at" {
                        trace!(
                            "GetArrayElement: preserving known array property access '{}', defer to object coercion",
                            index_str
                        );
                    } else {
                        warn!(
                            "GetArrayElement: non-numeric array index '{}' => undefined",
                            index_str
                        );
                        node.reduce(Undefined);
                    }
                    Ok(())
                };
            }
        }

        if let (Some(nan_node), Some(index_node)) = (view.child(0), view.child(2))
            && let (Some(NaN), Some(Raw(_))) = (nan_node.data(), index_node.data())
        {
            trace!("GetArrayElement: accessing index of non-array, setting to undefined");
            node.reduce(Undefined);
        }

        Ok(())
    }
}

pub fn flatten_array(arr: &Vec<JavaScript>, separator: Option<String>) -> String {
    let separator = separator.unwrap_or_else(|| ",".to_string());
    arr.iter()
        .map(|value| flatten_value(value, Some(separator.clone())))
        .collect::<Vec<String>>()
        .join(&separator)
}

fn flatten_value(value: &JavaScript, separator: Option<String>) -> String {
    match value {
        Array(arr) => flatten_array(arr, separator.clone()),
        Raw(Num(n)) => match *n {
            f64::INFINITY => "Infinity".to_string(),
            f64::NEG_INFINITY => "-Infinity".to_string(),
            n => n.to_string(),
        },
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
    format!(
        "{}{}",
        flatten_array(left, None),
        flatten_array(right, None)
    )
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

        if let (Some(operator), Some(operand)) = (view.child(0), view.child(1))
            && let ("+", Some(Array(arr))) = (operator.text()?, operand.data())
        {
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

        Ok(())
    }
}

fn recursive_array_number_extraction(arr: &Vec<JavaScript>) -> Option<f64> {
    match Array(arr.clone()).as_js_num() {
        Raw(Num(n)) => Some(n),
        NaN => None,
        _ => unreachable!("as_js_num should only return Raw(Num) or NaN"),
    }
}

/// Infers join calls on arrays
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::string::{ParseString, ToString};
/// use minusone::js::integer::ParseInt;
/// use minusone::js::array::{ParseArray, ArrayJoin};
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = ['a', 'b'].join();").unwrap();
/// tree.apply_mut(&mut (
///     ParseString::default(), ParseInt::default(), ParseArray::default(), ToString::default(), ArrayJoin::default()
/// )).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
/// assert_eq!(linter.output, "var x = 'a,b';");
/// ```
#[derive(Default)]
pub struct ArrayJoin;

impl<'a> RuleMut<'a> for ArrayJoin {
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
        if view.kind() != "call_expression" {
            return Ok(());
        }

        let Some(callee) = view.named_child("function").or_else(|| view.child(0)) else {
            return Ok(());
        };

        let Some(method) = method_name(&callee) else {
            return Ok(());
        };
        if method != "join" {
            return Ok(());
        }

        let Some(object) = callee.named_child("object") else {
            return Ok(());
        };
        let Some(Array(input)) = object.data() else {
            return Ok(());
        };

        let args = view.named_child("arguments");
        let positional_args = get_positional_arguments(args);

        let separator: Option<String> = match positional_args.first().and_then(|a| a.data()) {
            None => None,
            Some(Undefined) => None,
            Some(Raw(Str(s))) => Some(s.clone()),
            _ => return Ok(()),
        };

        let flatten = flatten_array(input, separator);
        trace!(
            "ArrayJoin: reducing {:?}.join({:?}) to '{}'",
            input,
            positional_args.first().and_then(|a| a.data()),
            flatten
        );
        node.reduce(Raw(Str(flatten)));

        Ok(())
    }
}
