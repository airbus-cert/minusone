use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use log::trace;

/// Forward inferred type in the most simple cases
#[derive(Default)]
pub struct Forward;

impl Forward {
    fn is_safe_to_discard(data: &JavaScript) -> bool {
        matches!(
            data,
            JavaScript::Raw(_) | JavaScript::Undefined | JavaScript::Null | JavaScript::NaN
        )
    }
}

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
        let mut forwarded = None;

        if view.kind() == "parenthesized_expression"
            && let Some(expression) = view.child(1)
            && let Some(expression_data) = expression.data()
        {
            forwarded = Some(expression_data.clone());
        } else if view.kind() == "sequence_expression" {
            let parts: Vec<_> = view.iter().filter(|child| child.kind() != ",").collect();

            if parts.len() >= 2
                && let Some(last_data) = parts.last().and_then(|part| part.data())
                && parts[..parts.len() - 1]
                    .iter()
                    .all(|part| part.data().is_some_and(Self::is_safe_to_discard))
            {
                forwarded = Some(last_data.clone());
            }
        }

        if let Some(data) = forwarded {
            trace!("Forward (L): Forwarding data to parent: {:?}", data);
            node.reduce(data)
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
