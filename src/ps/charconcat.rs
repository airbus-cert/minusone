use rule::RuleMut;
use ps::InferredValue;
use tree::{NodeMut};
use error::MinusOneResult;

pub struct CharConcatRule;

impl CharConcatRule {
   pub fn new() -> Self {
        CharConcatRule {

        }
    }
}

impl<'a> RuleMut<'a> for CharConcatRule {
    type Language = InferredValue;

    fn enter(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        unimplemented!()
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        unimplemented!()
    }
}