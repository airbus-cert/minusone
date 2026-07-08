use crate::error::MinusOneResult;
use crate::js::JavaScript::*;
use crate::js::Value::Bool;
use crate::js::Value::{Num, Str};
use crate::js::b64::js_bytes_to_string;
use crate::js::utils::*;
use crate::js::{IteratorKind, JavaScript};
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, Node, NodeMut};
use log::{trace, warn};
use std::clone::Clone;

/// Parses JavaScript array literals into `Array(_)`.
#[derive(Default)]
pub struct ParseArray;

impl ParseArray {
    fn parse_array_call<'a>(
        &self,
        node: &mut NodeMut<'a, JavaScript>,
        func_index: usize,
        args_index: usize,
    ) -> MinusOneResult<()> {
        let view = node.view();

        let is_array = view
            .child(func_index)
            .map(|f| f.text().ok() == Some("Array"))
            .unwrap_or(false);

        if !is_array {
            return Ok(());
        }

        let Some(args_node) = view.child(args_index) else {
            return Ok(());
        };

        let mut js = Vec::new();
        for child in args_node.iter() {
            if let Ok(text) = child.text() {
                if text == "(" || text == ")" || text == "," {
                    continue;
                }
            }
            if let Some(data) = child.data() {
                js.push(data.clone());
            } else {
                warn!(
                    "ParseArray: unable to parse Array call argument {:?}",
                    child.text()
                );
                return Ok(());
            }
        }

        trace!("ParseArray (L): Array call with {} elements", js.len());
        node.reduce(Array(js));
        Ok(())
    }
}

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
        match view.kind() {
            "array" => {
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
            }
            "call_expression" => self.parse_array_call(node, 0, 1)?,
            "new_expression" => self.parse_array_call(node, 1, 2)?,
            _ => {}
        }

        Ok(())
    }
}

/// Centralized dispatcher for array literal builtins.
///
/// This includes:
/// - `arr.at(x)`
/// - `arr.concat(thing1, ..., thingX)`
/// - `arr.copyWithin(x, y[, z])`
/// - `arr.entries()`
/// - `arr.fill(x[, y[, z]])`
/// - `arr.flat(x)`
/// - `arr.includes(x)`
/// - `arr.indexOf(x, y)`
/// - `arr.join(s)`
/// - `arr.lastIndexOf(x, y)`
/// - `arr.pop()` \*
/// - `arr.push(thing1, ..., thingX)` \*
/// - `arr.reverse()` \*
/// - `arr.shift()` \*
/// - `arr.slice(x[, y])`
/// - `arr.sort([fn])` \* \**
/// - `arr.toReversed()`
/// - `arr.toSorted([fn])` \**
/// - `arr.toString()`
/// - `arr.unshift(thing1, ..., thingX)` \*
/// - `arr.values()`
///
/// \* **Warning:** This function mutates the var/let value, this is **NOT** implemented.<br>
/// \** **Warning:** does **NOT** implement custom sorting with an arrow function
type ArrayBuiltinHandler = fn(&Vec<JavaScript>, &[JavaScript]) -> Option<JavaScript>;

const ARRAY_BUILTINS: &[(&str, ArrayBuiltinHandler)] = &[
    ("at", array_builtin_at),
    ("concat", array_builtin_concat),
    ("copyWithin", array_builtin_copy_within),
    ("entries", array_builtin_entries),
    ("fill", array_builtin_fill),
    ("flat", array_builtin_flat),
    ("includes", array_builtin_includes),
    ("indexOf", array_builtin_index_of),
    ("join", array_builtin_join),
    ("lastIndexOf", array_builtin_last_index_of),
    // ("pop", array_builtin_pop),
    // ("push", array_builtin_push),
    // ("reverse", array_builtin_reverse),
    // ("shift", array_builtin_shift),
    ("slice", array_builtin_slice),
    // ("sort", array_builtin_sort),
    ("toReversed", array_builtin_to_reversed),
    ("toSorted", array_builtin_to_sorted),
    ("toString", |arr, _| {
        Some(Raw(Str(flatten_array(arr, None))))
    }),
    // ("unshift", array_builtin_unshift),
    ("values", array_builtin_values),
];

fn is_array_builtin(method: &str) -> bool {
    ARRAY_BUILTINS.iter().any(|(name, _)| *name == method)
}

fn property_name(node: &Node<JavaScript>) -> Option<String> {
    match node.kind() {
        "member_expression" => method_name(node),
        "subscript_expression" => {
            let index = node.child(2).or_else(|| node.named_child("index"))?;
            let Some(Raw(Str(name))) = index.data() else {
                return None;
            };
            Some(name.clone())
        }
        _ => None,
    }
}

fn property_object<'a>(node: &'a Node<'a, JavaScript>) -> Option<Node<'a, JavaScript>> {
    match node.kind() {
        "member_expression" | "subscript_expression" => {
            node.child(0).or_else(|| node.named_child("object"))
        }
        _ => None,
    }
}

fn array_builtin_name_from_ref(node: &Node<JavaScript>) -> Option<String> {
    let method = property_name(node)?;
    if !is_array_builtin(&method) {
        return None;
    }

    let object = property_object(node)?;
    let Some(Array(_)) = object.data() else {
        return None;
    };

    Some(method)
}

fn array_builtin_native_source(node: &Node<JavaScript>) -> Option<String> {
    let method = array_builtin_name_from_ref(node)?;
    Some(format!("function {method}() {{ [native code] }}"))
}

fn is_array_builtin_constructor_name_chain(node: &Node<JavaScript>) -> bool {
    let Some(last_prop) = property_name(node) else {
        return false;
    };
    if last_prop != "name" {
        return false;
    }

    let Some(constructor_node) = property_object(node) else {
        return false;
    };
    let Some(constructor_prop) = property_name(&constructor_node) else {
        return false;
    };
    if constructor_prop != "constructor" {
        return false;
    }

    let Some(base_node) = property_object(&constructor_node) else {
        return false;
    };

    array_builtin_name_from_ref(&base_node).is_some()
}

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

        if view.kind() == "call_expression" {
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
            return Ok(());
        }

        if is_array_builtin_constructor_name_chain(&view) {
            let result = Raw(Str("Function".to_string()));
            trace!(
                "ArrayBuiltins: reducing array builtin constructor.name chain to {}",
                result
            );
            node.reduce(result);
            return Ok(());
        }

        if view.kind() == "binary_expression" {
            let left = view.child(0);
            let right = view.child(2);
            let operator = view.child(1);

            let (Some(left), Some(right), Some(operator)) = (left, right, operator) else {
                return Ok(());
            };

            if operator.kind() != "+" && operator.text()? != "+" {
                return Ok(());
            }

            let left_data = left.data().cloned();
            let right_data = right.data().cloned();

            if let (Some(left_src), Some(right_value)) =
                (array_builtin_native_source(&left), right_data.as_ref())
            {
                let result = Raw(Str(
                    format!("{}{}", left_src, as_known_string(right_value),),
                ));
                trace!(
                    "ArrayBuiltins: reducing builtin string concat to {}",
                    result
                );
                node.reduce(result);
                return Ok(());
            }

            if let (Some(left_value), Some(right_src)) =
                (left_data.as_ref(), array_builtin_native_source(&right))
            {
                let result = Raw(Str(
                    format!("{}{}", as_known_string(left_value), right_src,),
                ));
                trace!(
                    "ArrayBuiltins: reducing builtin string concat to {}",
                    result
                );
                node.reduce(result);
                return Ok(());
            }
        }

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

fn array_builtin_values(input: &Vec<JavaScript>, _args: &[JavaScript]) -> Option<JavaScript> {
    Some(Iterator {
        values: input.clone(),
        index: 0,
        kind: IteratorKind::Values,
    })
}

fn array_builtin_entries(input: &Vec<JavaScript>, _args: &[JavaScript]) -> Option<JavaScript> {
    Some(Iterator {
        values: input.clone(),
        index: 0,
        kind: IteratorKind::Entries,
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

/// # `.includes(x)`
/// No params = .includes(undefined)<br>
/// Only returns Raw(Bool(b))
fn array_builtin_includes(input: &Vec<JavaScript>, args: &[JavaScript]) -> Option<JavaScript> {
    let to_search = if args.is_empty() {
        Undefined
    } else {
        args[0].clone()
    };

    for value in input {
        match value == &to_search {
            true => return Some(Raw(Bool(true))),
            _ => {}
        }
    }

    Some(Raw(Bool(false)))
}

/// # `.indexOf(x, y)`
/// Default start = 0<br>
/// returns -1 if not found<br>
/// Only returns Raw(Num(n))
fn array_builtin_index_of(input: &Vec<JavaScript>, args: &[JavaScript]) -> Option<JavaScript> {
    let to_search = if args.is_empty() {
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

    for (i, value) in input.iter().skip(start).enumerate() {
        match value == &to_search {
            true => return Some(Raw(Num((i + start) as f64))),
            _ => {}
        }
    }

    Some(Raw(Num(-1f64)))
}

/// # ``
/// `.indexOf(x, y)` but searches backward<br>
/// returns -1 if not found<br>
/// Only returns Raw(Num(n))
fn array_builtin_last_index_of(input: &Vec<JavaScript>, args: &[JavaScript]) -> Option<JavaScript> {
    let to_search = if args.is_empty() {
        Undefined
    } else {
        args[0].clone()
    };

    let len = input.len();
    if len == 0 {
        return Some(Raw(Num(-1.0)));
    }

    let from_index = if args.len() >= 2 {
        match args[1].as_js_num() {
            Raw(Num(n)) => {
                let n = n.trunc() as isize;
                if n >= len as isize {
                    len as isize - 1
                } else if n < 0 {
                    let idx = len as isize + n;
                    if idx < 0 {
                        return Some(Raw(Num(-1.0)));
                    }
                    idx
                } else {
                    n
                }
            }
            NaN => 0,
            _ => unreachable!("The result of as_js_num should be either Raw(Num(x)) or NaN"),
        }
    } else {
        len as isize - 1
    };

    for i in (0..=from_index as usize).rev() {
        if &input[i] == &to_search {
            return Some(Raw(Num(i as f64)));
        }
    }

    Some(Raw(Num(-1.0)))
}

fn array_builtin_join(input: &Vec<JavaScript>, args: &[JavaScript]) -> Option<JavaScript> {
    let separator = if args.is_empty() {
        None
    } else {
        match args[0].clone() {
            Undefined => None,
            Raw(Str(s)) => Some(s.clone()),
            Array(a) => Some(flatten_array(&a, None)),
            any => Some(any.to_string()),
        }
    };

    let flatten = flatten_array(input, separator);
    Some(Raw(Str(flatten)))
}

/*/// # `.pop()`
/// Removes and returns the last element <br>
/// _(mutates array)_
fn array_builtin_pop(input: &Vec<JavaScript>, _args: &[JavaScript]) -> Option<JavaScript> {
    if input.is_empty() {
        Some(Undefined)
    } else {
        Some(input[input.len() - 1].clone())
    }
}*/

/*/// # `.push(thing1, ..., thingX)`
/// Adds elements to end<br>
/// returns the new length<br>
/// _(mutates array)_
fn array_builtin_push(input: &Vec<JavaScript>, args: &[JavaScript]) -> Option<JavaScript> {
    Some(Raw(Num((input.len() + args.len()) as f64)))
}*/

/*/// # `.reverse()`
/// Reverse othe order of the array and also return it<br>
/// _(mutates array)_
fn array_builtin_reverse(input: &Vec<JavaScript>, _args: &[JavaScript]) -> Option<JavaScript> {
    let new_array = input.iter().rev().cloned().collect();
    Some(Array(new_array))
}*/

/// # `.toReversed()`
/// `.reverse()` but create a copy so it does *not mutate* the original array
fn array_builtin_to_reversed(input: &Vec<JavaScript>, _args: &[JavaScript]) -> Option<JavaScript> {
    let new_array = input.iter().rev().cloned().collect();
    Some(Array(new_array))
}

/*/// # `.shift()`
/// Removes and returns first element<br>
/// _(mutates array)_
fn array_builtin_shift(input: &Vec<JavaScript>, _args: &[JavaScript]) -> Option<JavaScript> {
    if input.is_empty() {
        Some(Undefined)
    } else {
        Some(input[0].clone())
    }
}*/

/// # `.slice(start[, end])`
///  Handles negative indices<br>
/// no params = `.slice(0)`<br>
/// Extracts from `n1` to `n2` (exclusive)<br>
/// returns a copy
fn array_builtin_slice(input: &Vec<JavaScript>, args: &[JavaScript]) -> Option<JavaScript> {
    let len = input.len() as isize;

    let start = match args.first().map(|a| a.as_js_num()) {
        Some(Raw(Num(n))) => n as isize,
        _ => 0,
    };

    let end = match args.get(1).map(|a| a.as_js_num()) {
        Some(Raw(Num(n))) => n as isize,
        _ => len,
    };

    let start = if start < 0 {
        (len + start).max(0) as usize
    } else {
        start as usize
    };
    let end = if end < 0 {
        (len + end).max(0) as usize
    } else {
        end as usize
    };

    if start >= end || start >= input.len() {
        return Some(Array(vec![]));
    }

    let end = end.min(input.len());
    let result = input[start..end].to_vec();
    Some(Array(result))
}

/*/// # `.sort([customSortFn])`
/// Sort the array (`to_string` based ??)<br>
/// _(mutates array)_
fn array_builtin_sort(input: &Vec<JavaScript>, args: &[JavaScript]) -> Option<JavaScript> {
    if let Some(Function { .. }) = args.first() {
        warn!("The sort() builtin does not handle custom comparator functions. Skipping...");
        return None;
    }
    let mut new_array = input.clone();
    new_array.sort_by(|a, b| as_known_string(a).cmp(&as_known_string(b)));
    Some(Array(new_array))
}*/

/// # `.toSorted([customSortFn])`
/// `.sort([customSortFn])` but create a copy so it does *not mutate* the original array
fn array_builtin_to_sorted(input: &Vec<JavaScript>, args: &[JavaScript]) -> Option<JavaScript> {
    if let Some(Function { .. }) = args.first() {
        warn!("The sort() builtin does not handle custom comparator functions. Skipping...");
        return None;
    }
    let mut new_array = input.clone();
    new_array.sort_by(|a, b| as_known_string(a).cmp(&as_known_string(b)));
    Some(Array(new_array))
}

/*/// # `.unshift(thing1, ..., thingX)`
/// Adds elements to start<br>
/// returns new length<br>
/// _(mutates array)_
fn array_builtin_unshift(input: &Vec<JavaScript>, args: &[JavaScript]) -> Option<JavaScript> {
    Some(Raw(Num((args.len() + input.len()) as f64)))
}*/

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

#[cfg(test)]
mod tests_js_array {
    use super::*;
    use crate::js::build_javascript_tree;
    use crate::js::forward::Forward;
    use crate::js::integer::{ParseInt, PosNeg, Substract};
    use crate::js::iterator::IteratorBuiltins;
    use crate::js::linter::Linter;
    use crate::js::objects::object::ObjectField;
    use crate::js::specials::{AddSubSpecials, ParseSpecials};
    use crate::js::string::BracketCharAt;
    use crate::js::string::ParseString;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseString::default(),
            ParseArray::default(),
            ParseSpecials::default(),
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
        assert_eq!(deobfuscate("var x = [0, 1, 2].includes()"), "var x = false");
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, undefined].includes()"),
            "var x = true"
        );
        assert_eq!(deobfuscate("var x = [0, 1, 2].includes(2)"), "var x = true");
        assert_eq!(
            deobfuscate("var x = [0,1,2].includes('2')"),
            "var x = false"
        );
    }

    #[test]
    fn test_builtin_index_of() {
        assert_eq!(deobfuscate("var x = [0, 1, 2].indexOf()"), "var x = -1");
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, undefined].indexOf()"),
            "var x = 3"
        );
        assert_eq!(deobfuscate("var x = [0, 1, 2].indexOf(2)"), "var x = 2");
        assert_eq!(deobfuscate("var x = [0,1,2].indexOf('2')"), "var x = -1");
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 0, 1, 2].indexOf(2)"),
            "var x = 2"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 0, 1, 2].indexOf(2, 2)"),
            "var x = 2"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 0, 1, 2].indexOf(2, 3)"),
            "var x = 5"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 0, 1, 2].indexOf(2, '??')"),
            "var x = 2"
        );
    }

    #[test]
    fn test_builtin_last_index_of() {
        assert_eq!(deobfuscate("var x = [0, 1, 2].lastIndexOf()"), "var x = -1");
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, undefined].lastIndexOf()"),
            "var x = 3"
        );
        assert_eq!(deobfuscate("var x = [0, 1, 2].lastIndexOf(2)"), "var x = 2");
        assert_eq!(
            deobfuscate("var x = [0,1,2].lastIndexOf('2')"),
            "var x = -1"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 0, 1, 2].lastIndexOf(2)"),
            "var x = 5"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 0, 1, 2].lastIndexOf(2, 2)"),
            "var x = 2"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 0, 1, 2].lastIndexOf(2, 3)"),
            "var x = 2"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 0, 1, 2].lastIndexOf(2, '??')"),
            "var x = -1"
        );
    }

    /*#[test]
    fn test_builtin_pop() {
        assert_eq!(deobfuscate("var x = [0].pop()"), "var x = 0");
        assert_eq!(deobfuscate("var x = [].pop()"), "var x = undefined");
    }*/

    /*#[test]
    fn test_builtin_push() {
        assert_eq!(deobfuscate("var x = [0,1,2,3].push()"), "var x = 4");
        assert_eq!(deobfuscate("var x = [0,1,2,3].push(4)"), "var x = 5");
        assert_eq!(
            deobfuscate("var x = [0,1,2,3].push(undefined)"),
            "var x = 5"
        );
        assert_eq!(
            deobfuscate("var x = [0,1,2,3].push(4,5,6,7,8,9)"),
            "var x = 10"
        );
    }*/

    /*#[test]
    fn test_convert_builtin_to_string() {
        assert_eq!(
            deobfuscate("var x = [0].pop + ''"),
            "var x = 'function pop() { [native code] }'"
        );
        assert_eq!(
            deobfuscate("var x = undefined + [0].pop"),
            "var x = 'undefinedfunction pop() { [native code] }'"
        );
    }*/

    /*#[test]
    fn test_builtin_reverse() {
        assert_eq!(
            deobfuscate("var x = [0,1,2,3].reverse()"),
            "var x = [3, 2, 1, 0]"
        );
        assert_eq!(deobfuscate("var x = [0].reverse()"), "var x = [0]");
        assert_eq!(deobfuscate("var x = [].reverse()"), "var x = []");
    }*/

    #[test]
    fn test_builtin_to_reversed() {
        assert_eq!(
            deobfuscate("var x = [0,1,2,3].toReversed()"),
            "var x = [3, 2, 1, 0]"
        );
        assert_eq!(deobfuscate("var x = [0].toReversed()"), "var x = [0]");
        assert_eq!(deobfuscate("var x = [].toReversed()"), "var x = []");
    }

    /*#[test]
    fn test_builtin_shift() {
        assert_eq!(deobfuscate("var x = [0].shift()"), "var x = 0");
        assert_eq!(deobfuscate("var x = [].shift()"), "var x = undefined");
    }*/

    #[test]
    fn test_builtin_slice() {
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5].slice(1, 4);"),
            "var x = [1, 2, 3];"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5].slice(2);"),
            "var x = [2, 3, 4, 5];"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5].slice(-3);"),
            "var x = [3, 4, 5];"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5].slice(-4, -1);"),
            "var x = [2, 3, 4];"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5].slice(2, 1);"),
            "var x = [];"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5].slice(10);"),
            "var x = [];"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5].slice();"),
            "var x = [0, 1, 2, 3, 4, 5];"
        );
    }

    /*#[test]
    fn test_builtin_sort() {
        assert_eq!(
            deobfuscate("var x = [0, 8, 7, 3].sort()"),
            "var x = [0, 3, 7, 8]"
        );
        assert_eq!(deobfuscate("var x = [0].sort()"), "var x = [0]");
        assert_eq!(deobfuscate("var x = [].sort()"), "var x = []");
        assert_eq!(
            deobfuscate("var x = [9, 10, 11].sort()"),
            "var x = [10, 11, 9]"
        ); // to_string moment...
    }*/

    #[test]
    fn test_builtin_to_sorted() {
        assert_eq!(
            deobfuscate("var x = [0, 8, 7, 3].toSorted()"),
            "var x = [0, 3, 7, 8]"
        );
        assert_eq!(deobfuscate("var x = [0].toSorted()"), "var x = [0]");
        assert_eq!(deobfuscate("var x = [].toSorted()"), "var x = []");
        assert_eq!(
            deobfuscate("var x = [9, 10, 11].toSorted()"),
            "var x = [10, 11, 9]"
        ); // to_string moment...
    }

    /*#[test]
    fn test_builtin_unshift() {
        assert_eq!(deobfuscate("var x = [0,1,2,3].unshift()"), "var x = 4");
        assert_eq!(deobfuscate("var x = [0,1,2,3].unshift(4)"), "var x = 5");
        assert_eq!(
            deobfuscate("var x = [0,1,2,3].unshift(undefined)"),
            "var x = 5"
        );
        assert_eq!(
            deobfuscate("var x = [0,1,2,3].unshift(4,5,6,7,8,9)"),
            "var x = 10"
        );
    }*/

    #[test]
    fn test_builtin_values() {
        assert_eq!(
            deobfuscate("var x = [0, 1, 2].values()"),
            "var x = [object Array Iterator]"
        );

        let result = deobfuscate("var x = [0, 1, 2].values().next()");
        if result.starts_with("var x = {v") {
            assert_eq!(result, "var x = {value: 0, done: false}");
        } else {
            assert_eq!(result, "var x = {done: false, value: 0}");
        }

        assert_eq!(
            deobfuscate("var x = [0, 1, 2].values().next().value"),
            "var x = 0"
        );
    }

    #[test]
    fn test_builtin_length() {
        assert_eq!(deobfuscate("var x = [0, 1, 2].length"), "var x = 3");
        assert_eq!(deobfuscate("var x = [].length"), "var x = 0");
    }
}
