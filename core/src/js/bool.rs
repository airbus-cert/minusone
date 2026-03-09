use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::*;
use crate::js::Value::Bool;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use js::Value::Num;
use log::{debug, trace, warn};

/// Parses JavaScript numeric literals (decimal, hex, octal, binary) into `Raw(Num(_))`.
#[derive(Default)]
pub struct ParseBool;

impl<'a> RuleMut<'a> for ParseBool {
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
        if view.kind() != "true" && view.kind() != "false" {
            return Ok(());
        }

        trace!("ParseBool (L): boolean {}", view.kind());
        node.reduce(Raw(Bool(view.kind() == "true")));

        Ok(())
    }
}

/// This rule will infer unary ! operations on booleans.
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::bool::{NotBool, ParseBool};
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = !true;").unwrap();
/// tree.apply_mut(&mut (ParseBool::default(), NotBool::default())).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "var x = false;");
/// ```

#[derive(Default)]
pub struct NotBool;

impl<'a> RuleMut<'a> for NotBool {
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

        if let (Some(op), Some(value)) = (view.child(0), view.child(1)) {
            if op.text()? == "!" {
                debug!("Data for NotBool (L): {}", value.kind());
                match value.data() {
                    Some(Raw(Bool(b))) => {
                        trace!("NotBool (L): !{} => {}", b, !*b);
                        node.reduce(Raw(Bool(!*b)));
                    }
                    Some(Raw(Num(n))) => {
                        trace!("NotBool (L): !{} => {}", n, *n == 0);
                        node.reduce(Raw(Bool(*n == 0)));
                    }
                    Some(Array(_)) => {
                        trace!("NotBool (L): !array => false");
                        node.reduce(Raw(Bool(false)));
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }
}

/// This rule will infer boolean algebra operations (&&, ||).
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::bool::{BoolAlgebra, ParseBool};
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = true && false || true;").unwrap();
/// tree.apply_mut(&mut (ParseBool::default(), BoolAlgebra::default())).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "var x = true;");
/// ```
#[derive(Default)]
pub struct BoolAlgebra;

impl<'a> RuleMut<'a> for BoolAlgebra {
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
                (Some(Raw(Bool(a))), "&&", Some(Raw(Bool(b)))) => {
                    trace!("BoolAlgebra (L): {} && {} => {}", a, b, *a && *b);
                    node.reduce(Raw(Bool(*a && *b)));
                }
                (Some(Raw(Bool(a))), "||", Some(Raw(Bool(b)))) => {
                    trace!("BoolAlgebra (L): {} || {} => {}", a, b, *a || *b);
                    node.reduce(Raw(Bool(*a || *b)));
                }
                _ => {}
            }
        }

        Ok(())
    }
}
