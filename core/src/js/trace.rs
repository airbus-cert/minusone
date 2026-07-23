use crate::error::MinusOneResult;
use crate::js::linter::Linter;
use crate::js::{JavaScript, JavaScriptRuleSet};
use crate::rule::RuleMut;
pub use crate::trace::{Step, push_text_step};
use crate::trace::{find_root, push_main_step};
use crate::tree::{ControlFlow, NodeMut};


pub struct TracingRuleSet<'a> {
    inner: JavaScriptRuleSet<'a>,
    pub steps: Vec<Step>,
    record_all: bool,
}

impl<'a> TracingRuleSet<'a> {
    pub fn new(inner: JavaScriptRuleSet<'a>, record_all: bool) -> Self {
        Self {
            inner,
            steps: Vec::new(),
            record_all,
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
        let steps = &mut self.steps;
        let record_all = self.record_all;
        self.inner.leave_traced(
            node,
            flow,
            |node| {
                let mut linter = Linter::default();
                node.apply(&mut linter)?;
                Ok(linter.output)
            },
            move |node, rule_name, old, new| {
                let root = find_root(node.view());
                let mut linter = Linter::default();
                root.apply(&mut linter)?;

                push_main_step(
                    steps,
                    rule_name,
                    node.view().kind(),
                    node.view().start_abs(),
                    node.view().end_abs(),
                    linter.output,
                    old,
                    new,
                    record_all,
                );
                Ok(())
            },
        )
    }
}
