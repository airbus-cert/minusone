use core::rule::RuleMut;
use tree_sitter::Node;
use core::tree::ComponentDb;
use ps::inferred::InferredType;

#[derive(Default)]
pub struct Forward;

impl RuleMut for Forward {
    type Language = InferredType;

    fn enter(&mut self, node: Node, component: &mut dyn ComponentDb<Self::Language>) {
    }

    fn leave(&mut self, node: Node, component: &mut dyn ComponentDb<Self::Language>) {
        if node.kind() == "unary_expression" {
            if node.child_count() == 1 {
                if let Some(child_data) = component.get_node_data(node.child(0).unwrap()) {
                    *(component.get_node_data_mut(node)) = Some(child_data.clone());
                }
            }
        }
    }
}