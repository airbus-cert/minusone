use tree_sitter::{Node, TreeCursor};
use std::collections::HashMap;

pub trait ComponentDb<T> {
    fn init_from(&mut self, node: Node);
    fn get_node_data(&self, node: Node) -> &Option<T>;
    fn get_node_data_mut(&mut self, node: Node) -> &mut Option<T>;
}

pub type ComponentHashMap<T> = HashMap<usize, Option<T>>;

impl<T> ComponentDb<T> for ComponentHashMap<T> {

    fn init_from(&mut self, node: Node) {
        self.insert(node.id(), None);
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.init_from(child);
        }
    }

    fn get_node_data(&self, node: Node) -> &Option<T> {
        self.get(&node.id()).unwrap()
    }

    fn get_node_data_mut(&mut self, node: Node) -> &mut Option<T> {
        self.get_mut(&node.id()).unwrap()
    }
}
