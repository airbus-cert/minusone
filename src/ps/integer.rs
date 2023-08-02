use core::rule::RuleMut;
use ps::inferred::InferredType;
use core::tree::NodeMut;

#[derive(Default)]
pub struct ParseInt;

impl<'a> RuleMut<'a> for ParseInt {
    type Language = InferredType;

    fn enter(&mut self, node: &mut NodeMut<'a, Self::Language>) {
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) {
        if node.view().kind() != "integer_literal" {
            return
        }

        *node.as_mut() = Some(InferredType::Number(4));
    }
}