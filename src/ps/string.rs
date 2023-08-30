use rule::RuleMut;
use ps::InferredValue;
use tree::{NodeMut};
use error::MinusOneResult;
use ps::InferredValue::Str;

#[derive(Default)]
pub struct ParseString;

impl<'a> RuleMut<'a> for ParseString {
    type Language = InferredValue;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        let node_view = node.view();

        match node_view.kind() {
            "expandable_string_literal" | "verbatim_string_characters" => {
                let value = String::from(node_view.text()?);
                // Parse string by removing the double quote
                node.set(InferredValue::Str(String::from(&value[1..value.len() - 1])));
            }
            _ => ()
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct ConcatString;

impl<'a> RuleMut<'a> for ConcatString {
    type Language = InferredValue;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        let node_view = node.view();
        if node_view.kind() == "additive_expression"  {
            if let (Some(left_op), Some(operator), Some(right_op)) = (node_view.child(0), node_view.child(1), node_view.child(2)) {
                match (left_op.data(), operator.text()?, right_op.data()) {
                    (Some(Str(string_left)), "+", Some(Str(string_right))) => node.set(Str(String::from(string_left) + string_right)),
                    _ => {}
                }
            }
        }
        Ok(())
    }
}