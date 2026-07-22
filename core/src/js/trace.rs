use crate::error::MinusOneResult;
use crate::js::linter::Linter;
use crate::js::{JavaScript, JavaScriptRuleSet};
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, Node, NodeMut};


pub struct Step {
    // pre/main/post
    pub phase: &'static str,
    pub rule: String,
    pub kind: &'static str,
    pub start: usize,
    pub end: usize,
    pub source: String,
}

pub fn push_text_step(steps: &mut Vec<Step>, phase: &'static str, rule: &str, current: &str) {
    if steps.last().is_some_and(|s| s.source == current) {
        return;
    }

    steps.push(Step {
        phase,
        rule: rule.to_string(),
        kind: "program",
        start: 0,
        end: current.len(),
        source: current.to_string(),
    });
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
        let steps = &mut self.steps;
        self.inner.leave_traced(node, flow, move |node, rule_name| {
            let root = find_root(node.view());
            let mut linter = Linter::default();
            root.apply(&mut linter)?;

            steps.push(Step {
                phase: "main",
                rule: rule_name.to_string(),
                kind: node.view().kind(),
                start: node.view().start_abs(),
                end: node.view().end_abs(),
                source: linter.output,
            });
            Ok(())
        })
    }
}
