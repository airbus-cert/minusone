use rule::RuleMut;
use ps::InferredValue;
use tree::NodeMut;
use error::MinusOneResult;
use ps::InferredValue::Number;

#[derive(Default)]
pub struct ParseInt;

impl<'a> RuleMut<'a> for ParseInt {
    type Language = InferredValue;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        if node.view().kind() == "integer_literal" {
            if let Ok(integer) = node.view().text() {
                if let Ok(number) = integer.parse::<i32>() {
                    node.set(Number(number));
                }
            }
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct AddInt;

impl<'a> RuleMut<'a> for AddInt {
    type Language = InferredValue;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        let node_view = node.view();
        if node_view.kind() == "additive_expression"  {
            if let (Some(left_op), Some(operator), Some(right_op)) = (node_view.child(0), node_view.child(1), node_view.child(2)) {
                match (left_op.data(), operator.text()?, right_op.data()) {
                    (Some(Number(number_left)), "+", Some(Number(number_right))) => node.set(Number(number_left + number_right)),
                    (Some(Number(number_left)), "-", Some(Number(number_right))) => node.set(Number(number_left - number_right)),
                    _ => {}
                }
            }
        }
        Ok(())
    }
}