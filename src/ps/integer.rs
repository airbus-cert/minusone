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
        if let Ok(integer) = node.view().text() {
            if let Ok(number) = integer.parse::<i32>() {
                node.set(InferredValue::Number(number));
            }
        }
    }
}

#[derive(Default)]
pub struct AddInt;

impl<'a> RuleMut<'a> for AddInt {
    type Language = InferredValue;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) {
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) {
        if node.view().kind() != "additive_expression" {
            return
        }

        let node_view = node.view();

        match (node_view.child(0).data(), node_view.child(1).text().unwrap(), node_view.child(2).data()) {
            (Some(InferredValue::Number(number_left)), "+", Some(InferredValue::Number(number_right))) => node.set(InferredValue::Number(number_left + number_right)),
            _ => {}
        }
    }
}