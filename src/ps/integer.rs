use core::rule::RuleMut;
use tree_sitter::Node;
use core::tree::ComponentDb;
use ps::inferred::InferredType;

#[derive(Default)]
pub struct ParseInt;

impl RuleMut for ParseInt {
    type Language = InferredType;

    fn enter(&mut self, node: Node, component: &mut dyn ComponentDb<Self::Language>) {
    }

    fn leave(&mut self, node: Node, component: &mut dyn ComponentDb<Self::Language>) {
        if node.kind() != "integer_literal" {
            return
        }

        let mut data = component.get_node_data_mut(node);
        *data = Some(InferredType::Number(4));
    }
}