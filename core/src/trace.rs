use crate::tree::Node;

#[derive(Clone)]
pub struct Step {
    pub phase: &'static str,
    pub rule: String,
    pub kind: &'static str,
    pub start: usize,
    pub end: usize,
    pub source: String,
    pub old: String,
    pub new: String,
    pub has_node_diff: bool,
}

pub fn push_text_step(
    steps: &mut Vec<Step>,
    phase: &'static str,
    rule: &str,
    current: &str,
    record_all: bool,
) {
    if !record_all && steps.last().is_some_and(|s| s.source == current) {
        return;
    }

    steps.push(Step {
        phase,
        rule: rule.to_string(),
        kind: "program",
        start: 0,
        end: current.len(),
        source: current.to_string(),
        old: String::new(),
        new: String::new(),
        has_node_diff: false,
    });
}

#[allow(clippy::too_many_arguments)]
pub fn push_main_step(
    steps: &mut Vec<Step>,
    rule: &str,
    kind: &'static str,
    start: usize,
    end: usize,
    source: String,
    old: String,
    new: String,
    record_all: bool,
) {
    if !record_all && steps.last().is_some_and(|s| s.source == source) {
        return;
    }

    steps.push(Step {
        phase: "main",
        rule: rule.to_string(),
        kind,
        start,
        end,
        source,
        old,
        new,
        has_node_diff: true,
    });
}

pub enum Stepper {
    Js(crate::js::step::JsStepper),
    Ps(crate::ps::step::PsStepper),
}

impl Stepper {
    pub fn next(&mut self) -> Option<Step> {
        match self {
            Stepper::Js(s) => s.next(),
            Stepper::Ps(s) => s.next(),
        }
    }
}

impl Iterator for Stepper {
    type Item = Step;

    fn next(&mut self) -> Option<Step> {
        Stepper::next(self)
    }
}

pub fn find_root<'a, T>(node: Node<'a, T>) -> Node<'a, T> {
    let mut current = node;
    while let Some(parent) = current.parent() {
        current = parent;
    }
    current
}
