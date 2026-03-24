use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::*;
use crate::js::Value::Bool;
use crate::js::Value::{Num, Str};
use crate::js::array::flatten_array;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
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
            "identifier" => {
                if view.data() == None && view.text()? == "NaN" {
                    trace!("ParseSpecials (L): NaN");
                    node.reduce(NaN);
                    return Ok(());
                }
                if view.data() == None && view.text()? == "null" {
                    trace!("ParseSpecials (L): null");
                    node.reduce(Null);
                    return Ok(());
                }
            }
            _ => {}
        }

        Ok(())
    }
}

/// Infer `+` and `-` on Undefined and NaN.
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
                            let array_str = flatten_array(array, None);
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
                            let array_str = flatten_array(array, None);
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
                            let array_str = flatten_array(array, None);
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
                            let array_str = flatten_array(array, None);
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
                        trace!(
                            "AddSubSpecials (R): undefined + '{}' => 'undefined{}'",
                            s, s
                        );
                        node.reduce(Raw(Str(format!("undefined{}", s))));
                    }
                    (Some(Raw(Str(s))), Some(Undefined)) => {
                        trace!(
                            "AddSubSpecials (L): '{}' + undefined => '{}undefined'",
                            s, s
                        );
                        node.reduce(Raw(Str(format!("{}undefined", s))));
                    }
                    (Some(NaN), Some(Raw(Str(s)))) => {
                        trace!("AddSubSpecials (R): NaN + '{}' => 'NaN{}'", s, s);
                        node.reduce(Raw(Str(format!("NaN{}", s))));
                    }
                    (Some(Raw(Str(s))), Some(NaN)) => {
                        trace!("AddSubSpecials (L): '{}' + NaN => '{}NaN'", s, s);
                        node.reduce(Raw(Str(format!("{}NaN", s))));
                    }
                    _ => {}
                }
            }
        }


        Ok(())
    }
}

#[cfg(test)]
mod tests_js_specials {
    use crate::js::array::*;
    use crate::js::bool::ParseBool;
    use crate::js::build_javascript_tree;
    use crate::js::forward::Forward;
    use crate::js::integer::ParseInt;
    use crate::js::linter::Linter;
    use crate::js::specials::*;
    use crate::js::string::ParseString;

    fn deobfuscate_specials(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseString::default(),
            ParseBool::default(),
            ParseArray::default(),
            ParseSpecials::default(),
            AddSubSpecials::default(),
            CombineArrays::default(),
            GetArrayElement::default(),
            Forward::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_parse_specials() {
        assert_eq!(
            deobfuscate_specials("var x = undefined;"),
            "var x = undefined;"
        );
        assert_eq!(deobfuscate_specials("var x = NaN;"), "var x = NaN;");
    }

    #[test]
    fn test_empty_array_plus_undefined() {
        assert_eq!(
            deobfuscate_specials("var x = ([1][2]) + [];"),
            "var x = 'undefined';"
        );
    }

    #[test]
    fn test_empty_array_plus_nan() {
        assert_eq!(deobfuscate_specials("var x = [] + NaN;"), "var x = 'NaN';");
    }

    #[test]
    fn test_undefined_plus_number_gives_nan() {
        assert_eq!(
            deobfuscate_specials("var x = undefined + 1;"),
            "var x = NaN;"
        );
    }

    #[test]
    fn test_special_plus_string() {
        assert_eq!(
            deobfuscate_specials("var x = undefined + 'hello';"),
            "var x = 'undefinedhello';"
        );
        assert_eq!(
            deobfuscate_specials("var x = 'cheese' + NaN;"),
            "var x = 'cheeseNaN';"
        );
    }

    #[test]
    fn test_array_plus_special() {
        assert_eq!(
            deobfuscate_specials("var x = [1, 2] + undefined;"),
            "var x = '1,2undefined';"
        );
        assert_eq!(
            deobfuscate_specials("var x = [1, 2] + NaN;"),
            "var x = '1,2NaN';"
        );
    }
}
