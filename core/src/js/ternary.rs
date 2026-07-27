use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use log::trace;

/// Infer ternary operations
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::comparator::LooseEq;
/// use minusone::js::integer::ParseInt;
/// use minusone::js::linter::Linter;
/// use minusone::js::ternary::Ternary;
///
/// let mut tree = build_javascript_tree("var x = 1 == 1 ? 2 : 3;").unwrap();
/// tree.apply_mut(&mut (ParseInt::default(), LooseEq::default(), Ternary::default())).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "var x = 2;");
/// ```
#[derive(Default)]
pub struct Ternary;

impl<'a> RuleMut<'a> for Ternary {
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
        if view.kind() != "ternary_expression" {
            return Ok(());
        }

        if let (
            Some(condition),
            Some(left_symbol),
            Some(consequence),
            Some(right_symbol),
            Some(alternative),
        ) = (
            view.child(0),
            view.child(1),
            view.child(2),
            view.child(3),
            view.child(4),
        ) {
            match match (
                condition.data(),
                left_symbol.text()?,
                consequence.data(),
                right_symbol.text()?,
                alternative.data(),
            ) {
                (Some(condition), "?", Some(consequence), ":", Some(alternative)) => {
                    let result = if condition.as_bool() {
                        consequence
                    } else {
                        alternative
                    };
                    trace!(
                        "Ternary (L): {} ? {} : {} => {}",
                        condition, consequence, alternative, result
                    );
                    Some(result.clone())
                }
                _ => None,
            } {
                Some(result) => node.reduce(result),
                None => {}
            }
        }

        Ok(())
    }
}
