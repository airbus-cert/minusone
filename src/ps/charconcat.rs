use core::rule::Rule;
use core::entity::Entity;
use ps::inferred::InferredType;

pub struct CharConcatRule;

impl CharConcatRule {
   pub fn new() -> Self {
        CharConcatRule {
            
        }
    }
}

impl Rule for CharConcatRule {
    type Language = InferredType;

    fn enter(&self, entity: &mut Entity<Self::Language>) {
        unimplemented!()
    }

    fn leave(&self, entity: &mut Entity<Self::Language>) {
        unimplemented!()
    }
}