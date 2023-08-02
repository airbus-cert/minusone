use core::rule::RuleMut;
use ps::inferred::InferredType;
use core::tree::{NodeMut};

pub struct CharConcatRule;

impl CharConcatRule {
   pub fn new() -> Self {
        CharConcatRule {

        }
    }
}

impl<'a> RuleMut<'a> for CharConcatRule {
    type Language = InferredType;

    fn enter(&mut self, node: &mut NodeMut<'a, Self::Language>) {
        unimplemented!()
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) {
        unimplemented!()
    }
}