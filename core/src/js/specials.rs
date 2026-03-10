use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::*;
use crate::js::Value::Bool;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use js::array::flatten_array;
use js::Value::{Num, Str};
use log::trace;

/// Parse specials
#[derive(Default)]
pub struct ParseSpecials;

impl<'a> RuleMut<'a> for ParseSpecials {
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
            "undefined" => {
                trace!("ParseSpecials (L): undefined");
                node.reduce(Undefined);
                return Ok(());
            }
            "NaN" => {
                trace!("ParseSpecials (L): NaN");
                node.reduce(NaN);
                return Ok(());
            }
            _ => {}
        }

        // detect [...]['at']
        if view.kind() == "subscript_expression" {
            if let (Some(array_node), Some(index_node)) = (view.child(0), view.child(2)) {
                if let (Some(Array(_)), Some(Raw(Str(index)))) =
                    (array_node.data(), index_node.data())
                {
                    if index == "at" {
                        trace!("ParseSpecials (L): array['at'] => Special At");
                        node.reduce(At);
                        return Ok(());
                    }
                }
            }
        }

        // detect ...['constructor'] can be string array At number...
        if view.kind() == "subscript_expression" {
            if let (Some(array_node), Some(index_node)) = (view.child(0), view.child(2)) {
                if let (Some(js), Some(Raw(Str(index)))) = (array_node.data(), index_node.data()) {
                    if index == "constructor" {
                        trace!("ParseSpecials (L): array['constructor'] => Special Constructor");
                        node.reduce(Constructor(Box::new(js.clone())));
                        return Ok(());
                    }
                }
            }
        }

        Ok(())
    }
}

/// This rule will infer add and sub on Undefined and NaN.
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::specials::AddSubSpecials;
/// use minusone::js::forward::Forward;
/// use minusone::js::integer::ParseInt;
/// use minusone::js::array::{ParseArray, CombineArrays, GetArrayElement};
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = ([1][2]) + [];").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(),
///     ParseArray::default(),
///     Forward::default(),
///     CombineArrays::default(),
///     GetArrayElement::default(),
///     AddSubSpecials::default()
/// )).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
/// assert_eq!(linter.output, "var x = 'undefined';");
/// ```

#[derive(Default)]
pub struct AddSubSpecials;

impl<'a> RuleMut<'a> for AddSubSpecials {
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
            if op.kind() == "+" {
                match (left.data(), right.data()) {
                    (Some(Array(array)), Some(Undefined)) => {
                        if array.is_empty() {
                            trace!("AddSubSpecials (L): [] + undefined => 'undefined'");
                            node.reduce(Raw(Str("undefined".to_string())));
                        } else {
                            trace!(
                                "AddSubSpecials (L): [{}] + undefined => '[..]undefined'",
                                array
                                    .iter()
                                    .map(|v| v.to_string())
                                    .collect::<Vec<_>>()
                                    .join(",")
                            );
                            let array_str = flatten_array(array);
                            node.reduce(Raw(Str(format!("{}undefined", array_str))));
                        }
                    }
                    (Some(Undefined), Some(Array(array))) => {
                        if array.is_empty() {
                            trace!("AddSubSpecials (R): undefined + [] => 'undefined'");
                            node.reduce(Raw(Str("undefined".to_string())));
                        } else {
                            trace!(
                                "AddSubSpecials (R): undefined + [{}] => 'undefined[..]'",
                                array
                                    .iter()
                                    .map(|v| v.to_string())
                                    .collect::<Vec<_>>()
                                    .join(",")
                            );
                            let array_str = flatten_array(array);
                            node.reduce(Raw(Str(format!("undefined{}", array_str))));
                        }
                    }
                    (Some(Array(array)), Some(NaN)) => {
                        if array.is_empty() {
                            trace!("AddSubSpecials (L): [] + NaN => 'NaN'");
                            node.reduce(Raw(Str("NaN".to_string())));
                        } else {
                            trace!(
                                "AddSubSpecials (L): [{}] + NaN => '[..]NaN'",
                                array
                                    .iter()
                                    .map(|v| v.to_string())
                                    .collect::<Vec<_>>()
                                    .join(",")
                            );
                            let array_str = flatten_array(array);
                            node.reduce(Raw(Str(format!("{}NaN", array_str))));
                        }
                    }
                    (Some(NaN), Some(Array(array))) => {
                        if array.is_empty() {
                            trace!("AddSubSpecials (R): NaN + [] => 'NaN'");
                            node.reduce(Raw(Str("NaN".to_string())));
                        } else {
                            trace!(
                                "AddSubSpecials (R): NaN + [{}] => 'NaN[..]'",
                                array
                                    .iter()
                                    .map(|v| v.to_string())
                                    .collect::<Vec<_>>()
                                    .join(",")
                            );
                            let array_str = flatten_array(array);
                            node.reduce(Raw(Str(format!("NaN{}", array_str))));
                        }
                    }
                    (Some(Undefined), Some(Raw(Num(n)))) => {
                        trace!("AddSubSpecials (L): undefined + {} => NaN", n);
                        node.reduce(NaN);
                    }
                    (Some(Raw(Num(n))), Some(Undefined)) => {
                        trace!("AddSubSpecials (R): {} + undefined => NaN", n);
                        node.reduce(NaN);
                    }
                    (Some(Undefined), Some(Raw(Bool(b)))) => {
                        trace!("AddSubSpecials (R): undefined + {} => NaN", b);
                        node.reduce(NaN);
                    }
                    (Some(Raw(Bool(b))), Some(Undefined)) => {
                        trace!("AddSubSpecials (L): {} + undefined => NaN", b);
                        node.reduce(NaN);
                    }
                    (Some(Undefined), Some(Raw(Str(s)))) => {
                        trace!("AddSubSpecials (R): undefined + '{}' => 'undefined'", s);
                        node.reduce(Raw(Str("undefined".to_string())));
                    }
                    (Some(Raw(Str(s))), Some(Undefined)) => {
                        trace!("AddSubSpecials (L): '{}' + undefined => 'undefined'", s);
                        node.reduce(Raw(Str("undefined".to_string())));
                    }
                    (Some(NaN), Some(Raw(Str(s)))) => {
                        trace!("AddSubSpecials (R): NaN + '{}' => 'NaN'", s);
                        node.reduce(Raw(Str("NaN".to_string())));
                    }
                    (Some(Raw(Str(s))), Some(NaN)) => {
                        trace!("AddSubSpecials (L): '{}' + NaN => 'NaN'", s);
                        node.reduce(Raw(Str("NaN".to_string())));
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }
}

/// This rule will infer add on At.
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::specials::{AtTrick, ParseSpecials};
/// use minusone::js::string::ParseString;
/// use minusone::js::array::ParseArray;
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = []['at'] + '';").unwrap();
/// tree.apply_mut(&mut (
///     ParseSpecials::default(), ParseString::default(), ParseArray::default(), AtTrick::default()
/// )).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
/// assert_eq!(linter.output, "var x = 'function at() { [native code] }';");
/// ```
#[derive(Default)]
pub struct AtTrick;

impl<'a> RuleMut<'a> for AtTrick {
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
            if op.kind() == "+" {
                match (left.data(), right.data()) {
                    (Some(At), Some(Raw(Str(s)))) => {
                        trace!(
                            "AtTrick: []['at'] + '{}' => 'function at() {{ [native code] }}'",
                            s
                        );
                        node.reduce(Raw(Str(format!("function at() {{ [native code] }}{}", s))));
                    }
                    (Some(Raw(Str(s))), Some(At)) => {
                        trace!(
                            "AtTrick: '{}' + []['at'] => 'function at() {{ [native code] }}'",
                            s
                        );
                        node.reduce(Raw(Str(format!("{}function at() {{ [native code] }}", s))));
                    }
                    (Some(At), Some(Array(array))) => {
                        let array_str = flatten_array(array);
                        let array_join = array
                            .iter()
                            .map(|v| v.to_string())
                            .collect::<Vec<_>>()
                            .join(",");
                        trace!(
                            "AtTrick: []['at'] + [{}] => 'function at() {{ [native code] }}[{}]'",
                            array_join,
                            array_join
                        );
                        node.reduce(Raw(Str(format!(
                            "function at() {{ [native code] }}{}",
                            array_str
                        ))));
                    }
                    (Some(Array(array)), Some(At)) => {
                        let array_str = flatten_array(array);
                        let array_join = array
                            .iter()
                            .map(|v| v.to_string())
                            .collect::<Vec<_>>()
                            .join(",");
                        trace!(
                            "AtTrick: [{}] + []['at'] => '[{}]function at() {{ [native code] }}'",
                            array_join,
                            array_join
                        );
                        node.reduce(Raw(Str(format!(
                            "{}function at() {{ [native code] }}",
                            array_str
                        ))));
                    }
                    (Some(At), Some(Raw(Bool(b)))) => {
                        trace!(
                            "AtTrick: []['at'] + {} => 'function at() {{ [native code] }}{}'",
                            b,
                            b
                        );
                        node.reduce(Raw(Str(format!("function at() {{ [native code] }}{}", b))));
                    }
                    (Some(Raw(Bool(b))), Some(At)) => {
                        trace!(
                            "AtTrick: {} + []['at'] => '{}function at() {{ [native code] }}'",
                            b,
                            b
                        );
                        node.reduce(Raw(Str(format!("{}function at() {{ [native code] }}", b))));
                    }
                    (Some(NaN), Some(At)) => {
                        trace!("AtTrick: NaN + []['at'] => 'NaNfunction at() {{ [native code] }}'");
                        node.reduce(Raw(Str(format!("NaNfunction at() {{ [native code] }}"))));
                    }
                    (Some(At), Some(NaN)) => {
                        trace!("AtTrick: []['at'] + NaN => 'function at() {{ [native code] }}NaN'");
                        node.reduce(Raw(Str(format!("function at() {{ [native code] }}NaN"))));
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }
}

/// This rule will infer add on Constructor.
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::specials::{ConstructorTrick, ParseSpecials};
/// use minusone::js::string::ParseString;
/// use minusone::js::array::ParseArray;
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = []['constructor'] + '';").unwrap();
/// tree.apply_mut(&mut (
///     ParseSpecials::default(), ParseString::default(), ParseArray::default(), ConstructorTrick::default()
/// )).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
/// assert_eq!(linter.output, "var x = 'function Array() { [native code] }';");
/// ```
#[derive(Default)]
pub struct ConstructorTrick;

impl<'a> RuleMut<'a> for ConstructorTrick {
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
            if op.kind() == "+" {
                match (left.data(), right.data()) {
                    (Some(Constructor(constructor)), Some(Raw(Str(s)))) => {
                        trace!(
                            "ConstructorTrick: []['constructor'] + '{}' => '{}'",
                            s,
                            constructor_to_string(constructor)
                        );
                        node.reduce(Raw(Str(format!(
                            "{}{}",
                            constructor_to_string(constructor),
                            s
                        ))));
                    }
                    (Some(Raw(Str(s))), Some(Constructor(constructor))) => {
                        trace!(
                            "ConstructorTrick: '{}' + []['constructor'] => '{}'",
                            s,
                            constructor_to_string(constructor)
                        );
                        node.reduce(Raw(Str(format!(
                            "{}{}",
                            s,
                            constructor_to_string(constructor)
                        ))));
                    }
                    (Some(Constructor(constructor)), Some(Array(array))) => {
                        let array_str = flatten_array(array);
                        let array_join = array
                            .iter()
                            .map(|v| v.to_string())
                            .collect::<Vec<_>>()
                            .join(",");
                        trace!(
                            "ConstructorTrick: []['constructor'] + [{}] => '{}[{}]'",
                            array_join,
                            constructor_to_string(constructor),
                            array_join
                        );
                        node.reduce(Raw(Str(format!(
                            "{}{}",
                            constructor_to_string(constructor),
                            array_str
                        ))));
                    }
                    (Some(Array(array)), Some(Constructor(constructor))) => {
                        let array_str = flatten_array(array);
                        let array_join = array
                            .iter()
                            .map(|v| v.to_string())
                            .collect::<Vec<_>>()
                            .join(",");
                        trace!(
                            "ConstructorTrick: [{}] + []['constructor'] => '[{}]{}'",
                            array_join,
                            array_join,
                            constructor_to_string(constructor)
                        );
                        node.reduce(Raw(Str(format!(
                            "{}{}",
                            array_str,
                            constructor_to_string(constructor)
                        ))));
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }
}

fn constructor_to_string(constructor: &JavaScript) -> String {
    let fn_name = constructor_to_name(constructor);

    format!("function {fn_name}() {{ [native code] }}")
}

fn constructor_to_name(constructor: &JavaScript) -> String {
    match constructor {
        Undefined => "undefined".to_string(),
        NaN => "Number".to_string(),
        At => "Function".to_string(),
        Raw(v) => match v {
            Num(_) => "Number".to_string(),
            Str(_) => "String".to_string(),
            Bool(_) => "Boolean".to_string(),
        },
        Array(_) => "Array".to_string(),
        Constructor(inner) => constructor_to_name(inner),
    }
}

/// Infer constructor special accesse `''['constructor']['name']` => `'String'`
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::specials::{ParseSpecials, ConstructorAccessTrick};
/// use minusone::js::string::ParseString;
/// use minusone::js::array::ParseArray;
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = ''['constructor']['name'];").unwrap();
/// tree.apply_mut(&mut (
///     ParseString::default(), ParseArray::default(), ParseSpecials::default(), ConstructorAccessTrick::default()
/// )).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
/// assert_eq!(linter.output, "var x = 'String';");
/// ```
#[derive(Default)]
pub struct ConstructorAccessTrick;

impl<'a> RuleMut<'a> for ConstructorAccessTrick {
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
            if let (Some(Constructor(constructor)), Some(Raw(Str(index)))) =
                (array_node.data(), index_node.data())
            {
                if index == "name" {
                    trace!(
                        "ConstructorAccessTrick: []['constructor']['name'] => '{}'",
                        constructor_to_name(constructor)
                    );
                    node.reduce(Raw(Str(constructor_to_name(constructor))));
                }
            }
        }

        Ok(())
    }
}
