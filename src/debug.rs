use rule::Rule;
use tree::{Node};
use ps::InferredValue;

pub struct DebugView {
    tab_space: u32
}

impl DebugView {
    pub fn new() -> Self {
        DebugView {
            tab_space: 0
        }
    }
}

impl<'a> Rule<'a> for DebugView {
    type Language = InferredValue;

    fn enter(&mut self, node: &Node<'a, Self::Language>) {
        println!();

        for _ in 0..self.tab_space {
            print!(" ");
        }

        print!("({} inferred_type: {:?}", node.kind(), node.data());

        self.tab_space += 1;
    }

    fn leave(&mut self, _node: &Node<'a, Self::Language>) {
        print!(")");
        self.tab_space -= 1;
    }
}