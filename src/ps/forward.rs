use core::rule::RuleMut;
use ps::inferred::InferredType;
use core::tree::NodeMut;

#[derive(Default)]
pub struct Forward;

impl<'a> RuleMut<'a> for Forward {
    type Language = InferredType;

    fn enter(&mut self, node: &mut NodeMut<'a, Self::Language>) {
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) {
        if node.view().kind() == "unary_expression" {
            if node.view().child_count() == 1 {
                if let Some(child_data) = node.view().child(0).as_ref() {
                    *(node.as_mut()) = Some(child_data.clone());
                }
            }
        }
    }
}