use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use log::trace;

/// Forward inferred type in the most simple cases
#[derive(Default)]
pub struct Forward;

impl<'a> RuleMut<'a> for Forward {
    type Language = JavaScript;

    /// Nothing to do during top down exploration
    fn enter(
        &mut self,
        _node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        Ok(())
    }

    /// Forward the inferred type to the top node
    fn leave(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        let view = node.view();
        match view.kind() {
            "parenthesized_expression" => {
                if let Some(expression) = view.child(1) {
                    if let Some(expression_data) = expression.data() {
                        trace!(
                            "Forward (L): Forwarding data from child to parent: {:?}",
                            expression_data
                        );
                        node.reduce(expression_data.clone())
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }
}
