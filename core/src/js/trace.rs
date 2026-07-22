use crate::error::MinusOneResult;
use crate::js::linter::Linter;
use crate::js::{JavaScript, JavaScriptRuleSet};
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, Node, NodeMut};

pub struct Step {
    pub index: usize,
    pub node_id: usize,
    pub kind: &'static str,
    pub start: usize,
    pub end: usize,
    pub source: String,
}

fn find_root<'a, T>(node: Node<'a, T>) -> Node<'a, T> {
    let mut current = node;
    while let Some(parent) = current.parent() {
        current = parent;
    }
    current
}

pub struct TracingRuleSet<'a> {
    inner: JavaScriptRuleSet<'a>,
    pub steps: Vec<Step>,
}

impl<'a> TracingRuleSet<'a> {
    pub fn new(inner: JavaScriptRuleSet<'a>) -> Self {
        Self {
            inner,
            steps: Vec::new(),
        }
    }
}

impl<'a> RuleMut<'a> for TracingRuleSet<'a> {
    type Language = JavaScript;

    fn enter(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        flow: ControlFlow,
    ) -> MinusOneResult<()> {
        self.inner.enter(node, flow)
    }

    fn leave(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        flow: ControlFlow,
    ) -> MinusOneResult<()> {
        let before = node.view().data().cloned();
        self.inner.leave(node, flow)?;
        let after = node.view().data().cloned();

        let changed = match (&before, &after) {
            (None, Some(_)) => true,
            (Some(a), Some(b)) => a != b,
            _ => false,
        };

        if changed {
            let root = find_root(node.view());
            let mut linter = Linter::default();
            root.apply(&mut linter)?;

            self.steps.push(Step {
                index: self.steps.len(),
                node_id: node.id(),
                kind: node.view().kind(),
                start: node.view().start_abs(),
                end: node.view().end_abs(),
                source: linter.output,
            });
        }

        Ok(())
    }
}
