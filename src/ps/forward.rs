use rule::RuleMut;
use ps::InferredValue;
use tree::NodeMut;

#[derive(Default)]
pub struct Forward;

impl<'a> RuleMut<'a> for Forward {
    type Language = InferredValue;

    fn enter(&mut self, node: &mut NodeMut<'a, Self::Language>) {
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) {
        if node.view().child_count() == 1 {
            match node.view().kind() {
                "unary_expression" | "array_literal_expression" |
                "range_expression" | "format_expression" |
                "multiplicative_expression" => {
                    if let Some(child_data) = node.view().child(0).data() {
                        node.set(child_data.clone());
                    }
                }
                _ => {}
            }
        }
    }
}