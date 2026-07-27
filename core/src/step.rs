use crate::error::MinusOneResult;
use crate::tree::{BranchFlow, ControlFlow, Node, NodeMut};
use tree_sitter::Node as TreeNode;
use tree_sitter_traversal2::{Order, traverse};

pub enum LeaveOutcome<R> {
    Changed { result: R, resume_at: usize },
    Finished,
}

pub struct Walker<'a> {
    nodes: Vec<TreeNode<'a>>,
    idx: usize,
    stack: Vec<(TreeNode<'a>, usize, ControlFlow)>,
    control_flow: ControlFlow,
    resume_rule_at: usize,
}

impl<'a> Walker<'a> {
    pub fn new(root: TreeNode<'a>) -> Self {
        Walker {
            nodes: traverse(root.walk(), Order::Pre).collect(),
            idx: 0,
            stack: Vec::new(),
            control_flow: ControlFlow::Continue(BranchFlow::Predictable),
            resume_rule_at: 0,
        }
    }

    pub fn step<T, R>(
        &mut self,
        node_mut: &mut NodeMut<'a, T>,
        control: impl Fn(Node<T>) -> MinusOneResult<ControlFlow>,
        mut enter: impl FnMut(&mut NodeMut<'a, T>, ControlFlow) -> MinusOneResult<()>,
        mut leave_step: impl FnMut(
            &mut NodeMut<'a, T>,
            ControlFlow,
            usize,
        ) -> MinusOneResult<LeaveOutcome<R>>,
    ) -> MinusOneResult<Option<R>> {
        loop {
            if let Some(&(top, remaining, _)) = self.stack.last() {
                if remaining == 0 {
                    node_mut.inner = top;
                    if self.control_flow != ControlFlow::Break {
                        match leave_step(node_mut, self.control_flow, self.resume_rule_at)? {
                            LeaveOutcome::Changed { result, resume_at } => {
                                self.resume_rule_at = resume_at;
                                return Ok(Some(result));
                            }
                            LeaveOutcome::Finished => {
                                self.resume_rule_at = 0;
                            }
                        }
                    } else {
                        self.resume_rule_at = 0;
                    }

                    let (_, _, saved_flow) = self.stack.pop().unwrap();
                    self.control_flow = saved_flow;
                    if let Some(parent) = self.stack.last_mut() {
                        parent.1 -= 1;
                    }
                    continue;
                }
            }

            if self.idx >= self.nodes.len() {
                return Ok(None);
            }
            let node = self.nodes[self.idx];
            self.idx += 1;

            self.stack
                .push((node, node.child_count(), self.control_flow));

            node_mut.inner = node;
            self.control_flow = self.control_flow | control(node_mut.view())?;

            if self.control_flow != ControlFlow::Break {
                enter(node_mut, self.control_flow)?;
            }
        }
    }
}
