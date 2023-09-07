use rule::RuleMut;
use tree::{NodeMut};
use error::MinusOneResult;
use ps::Value::Str;
use ps::Powershell;
use ps::Powershell::Raw;

#[derive(Default)]
pub struct ParseString;

impl<'a> RuleMut<'a> for ParseString {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        let node_view = node.view();

        match node_view.kind() {
            "expandable_string_literal" | "verbatim_string_characters" => {
                let value = String::from(node_view.text()?);
                // Parse string by removing the double quote
                node.set(Raw(Str(String::from(&value[1..value.len() - 1]))));
            }
            _ => ()
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct ConcatString;

impl<'a> RuleMut<'a> for ConcatString {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        let view = node.view();
        if view.kind() == "additive_expression"  {
            if let (Some(left_op), Some(operator), Some(right_op)) = (view.child(0), view.child(1), view.child(2)) {
                match (left_op.data(), operator.text()?, right_op.data()) {
                     (Some(Raw(Str(string_left))), "+", Some(Raw(Str(string_right)))) => node.set(Raw(Str(String::from(string_left) + string_right))),
                    _ => {}
                }
            }
        }
        Ok(())
    }
}