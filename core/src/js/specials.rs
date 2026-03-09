use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::*;
use crate::js::Value::Bool;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use js::Value::{Num, Str};
use log::{debug, trace, warn};
use js::array::flatten_array;

/// This rule will infer add and sub on Undefined and NaN.
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::specials::AddSubSpecials;
/// use minusone::js::integer::ParseInt;
/// use minusone::js::array::{ParseArray, CombineArrays, GetArrayElement};
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = ([1][2]) + [];").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(), ParseArray::default(), CombineArrays::default(), GetArrayElement::default(), AddSubSpecials::default()
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

            debug!("AddSubSpecials: Left: {:?}, Op: {:?}, Right: {:?}", left.data(), op.kind(), right.data());

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
                    (Some(Array(array)), Some(NaN))  => {
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
                    (Some(NaN), Some(Array(array)))  => {
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
                    _ => {}
                }
            }
        }

        Ok(())
    }
}
