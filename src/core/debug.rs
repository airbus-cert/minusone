use core::rule::Rule;
use core::tree::{Node};
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

impl<'a> Rule<'a> for DebugView {
    type Language = InferredType;

    fn enter(&mut self, node: &Node<'a, Self::Language>) {
        println!();

        for i in 0..self.tab_space {
            print!(" ");
        }

        print!("({} inferred_type: {:?}", node.kind(), node.as_ref());

        self.tab_space += 1;
    }

    fn leave(&mut self, node: &Node<'a, Self::Language>) {

        print!(")");
        self.tab_space -= 1;
    }
}