use rule::RuleMut;
use ps::InferredValue;
use tree::{NodeMut};
use error::MinusOneResult;

#[derive(Default)]
pub struct ParseString;

impl<'a> RuleMut<'a> for ParseString {
    type Language = InferredValue;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        let node_view = node.view();

        if node_view.kind() == "string_literal" {
            let value = String::from(node_view.text()?);
            node.set(InferredValue::String(String::from(&value[1..value.len() - 1])));
        }

        Ok(())
    }
}