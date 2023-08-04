use rule::Rule;
use tree::{Node};
use ps::InferredValue;
use error::MinusOneResult;

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

    fn enter(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<()>{
        println!();

        for _ in 0..self.tab_space {
            print!(" ");
        }

        print!("({} inferred_type: {:?}", node.kind(), node.data());

        self.tab_space += 1;
        Ok(())
    }

    fn leave(&mut self, _node: &Node<'a, Self::Language>) -> MinusOneResult<()>{
        print!(")");
        self.tab_space -= 1;
        Ok(())
    }
}