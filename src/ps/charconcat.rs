use core::rule::RuleMut;
use ps::inferred::InferredType;
use tree_sitter::Node;
use core::tree::ComponentDb;

pub struct CharConcatRule;

impl CharConcatRule {
   pub fn new() -> Self {
        CharConcatRule {
            
        }
    }
}

impl RuleMut for CharConcatRule {
    type Language = InferredType;

    fn enter(&mut self, node: Node, component: &mut dyn ComponentDb<Self::Language>) {
        unimplemented!()
    }

    fn leave(&mut self, node: Node, component: &mut dyn ComponentDb<Self::Language>) {
        unimplemented!()
    }
}