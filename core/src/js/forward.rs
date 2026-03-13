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

#[cfg(test)]
pub mod tests_forward {
    use crate::js::build_javascript_tree;
    use crate::js::forward::Forward;
    use crate::js::integer::{AddInt, ParseInt};
    use crate::js::linter::Linter;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (ParseInt::default(), AddInt::default(), Forward::default()))
            .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_forward() {
        assert_eq!(deobfuscate("var x = ((1 + (((2)))))"), "var x = 3",);
    }
}
