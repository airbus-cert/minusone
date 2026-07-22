use crate::tree::Node;


pub struct Step {
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

pub fn push_main_step(
    steps: &mut Vec<Step>,
    rule: &str,
    kind: &'static str,
    start: usize,
    end: usize,
    source: String,
) {
    if steps.last().is_some_and(|s| s.source == source) {
        return;
    }

    steps.push(Step {
        phase: "main",
        rule: rule.to_string(),
        kind,
        start,
        end,
        source,
    });
}

pub fn find_root<'a, T>(node: Node<'a, T>) -> Node<'a, T> {
    let mut current = node;
    while let Some(parent) = current.parent() {
        current = parent;
    }
    current
}
