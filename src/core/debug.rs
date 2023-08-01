use core::rule::Rule;
use tree_sitter::Node;
use core::tree::ComponentDb;
use std::fmt::Debug;
use ps::inferred::InferredType;

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

impl Rule for DebugView {
    type Language = InferredType;

    fn enter(&mut self, node: Node, component: &dyn ComponentDb<Self::Language>) {
        println!();

        for i in 0..self.tab_space {
            print!(" ");
        }

        print!("({} inferred_type: {:?}", node.kind(), component.get_node_data(node).as_ref());

        self.tab_space += 1;
    }

    fn leave(&mut self, node: Node, component: &dyn ComponentDb<Self::Language>) {

        print!(")");
        self.tab_space -= 1;
    }
}