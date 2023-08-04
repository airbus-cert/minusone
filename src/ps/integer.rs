use rule::RuleMut;
use ps::InferredValue;
use tree::NodeMut;

#[derive(Default)]
pub struct ParseInt;

impl<'a> RuleMut<'a> for ParseInt {
    type Language = InferredValue;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) {
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) {
        if node.view().kind() != "integer_literal" {
            return
        }

        *node.as_mut() = Some(InferredValue::Number(4));
    }
}