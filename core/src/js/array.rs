use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::*;
use crate::js::Value::Bool;
use crate::js::Value::{Num, Str};
use crate::js::b64::js_bytes_to_string;
use crate::js::comparator::strict_eq;
use crate::js::utils::{get_positional_arguments, js_index_from_optional_arg, method_name};
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

/// Centralized dispatcher for array literal builtins.
type ArrayBuiltinHandler = fn(&Vec<JavaScript>, &[JavaScript]) -> Option<JavaScript>;

const ARRAY_BUILTINS: &[(&str, ArrayBuiltinHandler)] = &[
    ("at", array_builtin_at),
    ("concat", array_builtin_concat),
    ("copyWithin", array_builtin_copy_within),
    ("entries", array_builtin_entries),
    ("fill", array_builtin_fill),
    ("flat", array_builtin_flat),
    ("includes", array_builtin_includes),
];

#[derive(Default)]
pub struct ArrayBuiltins;

impl<'a> RuleMut<'a> for ArrayBuiltins {
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

        let Some(object) = callee.child(0).or_else(|| callee.named_child("object")) else {
            return Ok(());
        };
        let Some(Array(input)) = object.data() else {
            return Ok(());
        };

        let args = view.named_child("arguments");
        let positional_args = get_positional_arguments(args);
        let mut arg_values = Vec::with_capacity(positional_args.len());
        for arg in positional_args {
            let Some(value) = arg.data().cloned() else {
                return Ok(());
            };
            arg_values.push(value);
        }

        let Some(result) = dispatch_array_builtin(&method, input, &arg_values) else {
            return Ok(());
        };

        trace!(
            "ArrayBuiltins: reducing '{}'.{}(...) to {}",
            Array(input.clone()),
            method,
            result
        );
        node.reduce(result);
        Ok(())
    }
}

fn dispatch_array_builtin(
    method: &str,
    input: &Vec<JavaScript>,
    args: &[JavaScript],
) -> Option<JavaScript> {
    ARRAY_BUILTINS
        .iter()
        .find_map(|(name, handler)| (*name == method).then(|| handler(input, args)))
        .flatten()
}

/// ## `.at(x)`
/// Handles negative index<br>overlapping pos/neg = `undefined`<br>no params = `.at(0)`
fn array_builtin_at(input: &Vec<JavaScript>, args: &[JavaScript]) -> Option<JavaScript> {
    let index = js_index_from_optional_arg(args.first());
    let len = input.len() as i64;
    let normalized = if index >= 0 { index } else { len + index };

    if normalized < 0 || normalized >= len {
        return Some(Undefined);
    }

    Some(input[normalized as usize].clone())
}
/// ## `.concat(thing1, ..., thingX)`
/// no params = keep the array as is<br>
/// can take any type of args<br>
/// only unpack once the arrays `[0,1,2].concat(1, "a", ["b", ["c"]])` -> `[0, 1, 2, 1, 'a', 'b', ['c']]`
fn array_builtin_concat(input: &Vec<JavaScript>, args: &[JavaScript]) -> Option<JavaScript> {
    let mut array = input.clone();
    for arg in args {
        if let Array(arr) = arg {
            array.extend_from_slice(arr);
        } else {
            array.push(arg.clone());
        }
    }
    Some(Array(array))
}

/// # `.copyWithin(x, y[, z])`
/// No params = keep the array as is<br>
/// `.copyWithin(x)` -> `.copyWithin(x, 0)`<br>
/// `.copyWithin(x, y)` -> copy everything from x at y `[0,1,2,3,4,5,6].copyWithin(2,0)` -> `[0,1,0,1,2,3,4]`<br>
/// `.copyWithin(x, y, z)` -> copy everything from x at y but limits to z elements `[0,1,2,3,4,5,6].copyWithin(2,0,3)` -> `[0,1,0,1,2,5,6]`
fn array_builtin_copy_within(input: &Vec<JavaScript>, args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(Array(input.clone()));
    }

    let len = input.len();

    let target = match args[0].as_js_num() {
        Raw(Num(n)) => n as usize,
        NaN => 0usize,
        _ => unreachable!("The result of as_js_num should be either Raw(Num(x)) or NaN"),
    };

    let start = if args.len() >= 2 {
        match args[1].as_js_num() {
            Raw(Num(n)) => n as usize,
            NaN => 0usize,
            _ => unreachable!("The result of as_js_num should be either Raw(Num(x)) or NaN"),
        }
    } else {
        0usize
    };

    let end = if args.len() >= 3 {
        match args[2].as_js_num() {
            Raw(Num(n)) => n as usize,
            NaN => return Some(Array(input.clone())),
            _ => unreachable!("The result of as_js_num should be either Raw(Num(x)) or NaN"),
        }
    } else {
        len
    };

    let count = end.min(len).saturating_sub(start);

    let mut array = input.clone();
    for i in 0..count {
        if target + i >= len {
            break;
        }
        array[target + i] = input[start + i].clone();
    }

    Some(Array(array))
}

fn array_builtin_entries(input: &Vec<JavaScript>, _args: &[JavaScript]) -> Option<JavaScript> {
    Some(Iterator {
        values: input.clone(),
        index: 0,
    })
}

/// # `.fill(x[, y[, z]])`
/// No params = fill with `undefined`<br>
/// `.fill(x)` fill the array with x<br>
/// `fill(x, y)` -> fill the array with x from y `[0,1,2,3].fill(9,1)` -> `[0,9,9,9]`<br>
/// `fill(x, y, z)` -> fill the array with x from y to z `[0,1,2,3,5].fill(9,1,4)` -> `[0,9,9,9,5]`
fn array_builtin_fill(input: &Vec<JavaScript>, args: &[JavaScript]) -> Option<JavaScript> {
    let fill_with = if args.is_empty() {
        Undefined
    } else {
        args[0].clone()
    };

    let start = if args.len() >= 2 {
        match args[1].as_js_num() {
            Raw(Num(n)) => n as usize,
            NaN => 0usize,
            _ => unreachable!("The result of as_js_num should be either Raw(Num(x)) or NaN"),
        }
    } else {
        0usize
    };

    let end = if args.len() >= 3 {
        match args[2].as_js_num() {
            Raw(Num(n)) => n as usize,
            NaN => 0usize,
            _ => unreachable!("The result of as_js_num should be either Raw(Num(x)) or NaN"),
        }
    } else {
        input.len()
    };

    let mut array = input.clone();
    for i in start..end {
        array[i] = fill_with.clone();
    }

    Some(Array(array))
}

/// # `.flat(x)`
///  No args -> `.flat(1)`<br>
/// recursively unpack x times arrays in arrays `[0,1,[2,[3,[4,5]]]].flat(2)` -> `[0,1,2,3,[4,5]]`
fn array_builtin_flat(input: &Vec<JavaScript>, args: &[JavaScript]) -> Option<JavaScript> {
    let depth = if args.is_empty() {
        1usize
    } else {
        match args[0].as_js_num() {
            Raw(Num(n)) => n as usize,
            NaN => 0usize,
            _ => unreachable!("The result of as_js_num should be either Raw(Num(x)) or NaN"),
        }
    };

    Some(Array(unpack_array(input, depth)))
}

fn unpack_array(values: &[JavaScript], depth: usize) -> Vec<JavaScript> {
    let mut out = Vec::new();
    for value in values {
        match value {
            Array(arr) if depth > 0 => {
                out.extend(unpack_array(arr, depth - 1));
            }
            other => out.push(other.clone()),
        }
    }
    out
}

fn array_builtin_includes(input: &Vec<JavaScript>, args: &[JavaScript]) -> Option<JavaScript> {
    let to_search = if args.is_empty() {
        Undefined
    } else {
        args[0].clone()
    };

    for value in input {
        match strict_eq(value, &to_search, "===") {
            Some(true) => return Some(Raw(Bool(true))),
            _ => {}
        }
    }

    Some(Raw(Bool(false)))
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
        .filter(|s| !s.is_empty())
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

#[cfg(test)]
mod tests_js_array {
    use super::*;
    use crate::js::build_javascript_tree;
    use crate::js::forward::Forward;
    use crate::js::integer::{ParseInt, PosNeg, Substract};
    use crate::js::iterator::IteratorBuiltins;
    use crate::js::linter::Linter;
    use crate::js::objects::object::ObjectField;
    use crate::js::specials::AddSubSpecials;
    use crate::js::string::BracketCharAt;
    use crate::js::string::ParseString;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseString::default(),
            ParseArray::default(),
            CombineArrays::default(),
            Forward::default(),
            Substract::default(),
            ArrayBuiltins::default(),
            IteratorBuiltins::default(),
            PosNeg::default(),
            ObjectField::default(),
            GetArrayElement::default(),
            ArrayPlusMinus::default(),
            AddSubSpecials::default(),
            BracketCharAt::default(),
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
            "var x = [1, 2, [3, '4']]"
        );
    }

    #[test]
    fn test_combine_arrays() {
        assert_eq!(
            deobfuscate("var x = [0, 1,7] + [3, [7, '2', [88]]]"),
            "var x = '0,1,73,7,2,88'"
        );
    }

    #[test]
    fn test_get_array_element() {
        assert_eq!(
            deobfuscate("var x = ([1, [2, '3'], 4][1])[0];"),
            "var x = 2;"
        );
    }

    #[test]
    fn test_array_plus_minus() {
        assert_eq!(deobfuscate("var x = +[['455']];"), "var x = 455;");

        assert_eq!(deobfuscate("var x = +['a'];"), "var x = NaN;");

        assert_eq!(deobfuscate("var x = [8] - 1;"), "var x = 7;");
    }

    #[test]
    fn test_jsfuck_from_array_access() {
        assert_eq!(deobfuscate("var x = ([][[]]+[])[1];"), "var x = 'n';");
    }

    #[test]
    fn test_dont_reduce_array_lookup_when_used_as_callee() {
        assert_eq!(deobfuscate("var x = [][[]]();"), "var x = [][[]]();");
    }

    #[test]
    fn test_builtin_at() {
        assert_eq!(deobfuscate("var x = [0,1,2].at()"), "var x = 0");
        assert_eq!(deobfuscate("var x = [0,1,2].at(2)"), "var x = 2");
        assert_eq!(deobfuscate("var x = [0,1,2].at(3)"), "var x = undefined");
        assert_eq!(deobfuscate("var x = [0,1,2].at(-1)"), "var x = 2");
        assert_eq!(deobfuscate("var x = [0,1,2].at(-3)"), "var x = 0");
        assert_eq!(deobfuscate("var x = [0,1,2].at(-4)"), "var x = undefined");
    }

    #[test]
    fn test_builtin_concat() {
        assert_eq!(deobfuscate("var x = [0].concat()"), "var x = [0]");
        assert_eq!(deobfuscate("var x = [0].concat(1)"), "var x = [0, 1]");
        assert_eq!(
            deobfuscate("var x = [0,1,2].concat(1, 'a', ['b', ['c']])"),
            "var x = [0, 1, 2, 1, 'a', 'b', ['c']]"
        );
    }

    #[test]
    fn test_builtin_copy_within() {
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5, 6].copyWithin()"),
            "var x = [0, 1, 2, 3, 4, 5, 6]"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5, 6].copyWithin(2)"),
            "var x = [0, 1, 0, 1, 2, 3, 4]"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5, 6].copyWithin('2')"),
            "var x = [0, 1, 0, 1, 2, 3, 4]"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5, 6].copyWithin('??')"),
            "var x = [0, 1, 2, 3, 4, 5, 6]"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5, 6].copyWithin(2, 3)"),
            "var x = [0, 1, 3, 4, 5, 6, 6]"
        ); // crash here
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5, 6].copyWithin(2, '??')"),
            "var x = [0, 1, 0, 1, 2, 3, 4]"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5, 6].copyWithin(2, 0, 3)"),
            "var x = [0, 1, 0, 1, 2, 5, 6]"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5, 6].copyWithin(2, 0, '??')"),
            "var x = [0, 1, 2, 3, 4, 5, 6]"
        );
    }

    #[test]
    fn test_builtin_entries() {
        assert_eq!(
            deobfuscate("var x = [0, 1, 2].entries()"),
            "var x = [object Array Iterator]"
        );

        // Order of the fields is random ??
        let result = deobfuscate("var x = [0, 1, 2].entries().next()");
        if result.starts_with("var x = {v") {
            assert_eq!(result, "var x = {value: [0, 0], done: false}");
        } else {
            assert_eq!(result, "var x = {done: false, value: [0, 0]}");
        }

        assert_eq!(
            deobfuscate("var x = [0, 1, 2].entries().next().value"),
            "var x = [0, 0]"
        );
    }

    #[test]
    fn test_builtin_fill() {
        assert_eq!(deobfuscate("var x = [].fill()"), "var x = []");
        assert_eq!(
            deobfuscate("var x = [1, 2, 3].fill()"),
            "var x = [undefined, undefined, undefined]"
        );
        assert_eq!(
            deobfuscate("var x = [1, 2, 3].fill(0)"),
            "var x = [0, 0, 0]"
        );
        assert_eq!(
            deobfuscate("var x = [1, 2, 3].fill(0, 1)"),
            "var x = [1, 0, 0]"
        );
        assert_eq!(
            deobfuscate("var x = [1, 2, 3].fill(0, '??')"),
            "var x = [0, 0, 0]"
        );
        assert_eq!(
            deobfuscate("var x = [1, 2, 3, 4, 5].fill(0, 1, 4)"),
            "var x = [1, 0, 0, 0, 5]"
        );
        assert_eq!(
            deobfuscate("var x = [1, 2, 3, 4, 5].fill(0, 1, '??')"),
            "var x = [1, 2, 3, 4, 5]"
        );
        assert_eq!(
            deobfuscate("var x = [1, 2, 3, 4, 5].fill(0, '??', 4)"),
            "var x = [0, 0, 0, 0, 5]"
        );
        assert_eq!(
            deobfuscate("var x = [1, 2, 3, 4, 5].fill(0, '??', '??')"),
            "var x = [1, 2, 3, 4, 5]"
        );
    }

    #[test]
    fn test_builtin_flat() {
        assert_eq!(
            deobfuscate("var x = [0, 1, [2, [3, [4, 5]]]].flat()"),
            "var x = [0, 1, 2, [3, [4, 5]]]"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, [2, [3, [4, 5]]]].flat(0)"),
            "var x = [0, 1, [2, [3, [4, 5]]]]"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, [2, [3, [4, 5]]]].flat(1)"),
            "var x = [0, 1, 2, [3, [4, 5]]]"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, [2, [3, [4, 5]]]].flat(2)"),
            "var x = [0, 1, 2, 3, [4, 5]]"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, [2, [3, [4, 5]]]].flat('??')"),
            "var x = [0, 1, [2, [3, [4, 5]]]]"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, [2, [3, [4, 5]]]].flat(-1)"),
            "var x = [0, 1, [2, [3, [4, 5]]]]"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, [2, [3, [4, 5]]]].flat(Infinity)"),
            "var x = [0, 1, 2, 3, 4, 5]"
        );
    }

    #[test]
    fn test_builtin_includes() {
        assert_eq!(deobfuscate("var x = [0,1,2].includes()"), "var x = false");
        assert_eq!(deobfuscate("var x = [0,1,2].includes(2)"), "var x = true");
        assert_eq!(
            deobfuscate("var x = [0,1,2].includes('2')"),
            "var x = false"
        );
    }
}
