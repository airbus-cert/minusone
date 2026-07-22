use crate::error::MinusOneResult;
use crate::ps::linter::Linter;
use crate::ps::{Powershell, PowershellRuleSet};
use crate::rule::RuleMut;
pub use crate::trace::{Step, push_text_step};
use crate::trace::{find_root, push_main_step};
use crate::tree::{ControlFlow, NodeMut};

pub struct TracingRuleSet<'a> {
    inner: PowershellRuleSet<'a>,
    pub steps: Vec<Step>,
}

impl<'a> TracingRuleSet<'a> {
    pub fn new(inner: PowershellRuleSet<'a>) -> Self {
        Self {
            inner,
            steps: Vec::new(),
        }
    }
}

impl<'a> RuleMut<'a> for TracingRuleSet<'a> {
    type Language = Powershell;

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
        self.inner.leave_traced(node, flow, move |node, rule_name| {
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
            );
            Ok(())
        })
    }
}
