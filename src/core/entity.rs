use tree_sitter::{Node, TreeCursor};

pub struct Entity<T> {
    pub inferred_type: Option<T>,
    pub children: Vec<Entity<T>>
}

impl<T> Entity<T> {
    pub fn new() -> Self {
        Entity {
            inferred_type: None,
            children: vec![]
        }
    }
}

impl<'a, T> From<Node<'a>> for Entity<T> {
    fn from(value: Node) -> Self {
        let mut result = Self::new();
        let mut tree_cursor= value.walk();

        for child in value.children(&mut tree_cursor) {
            result.children.push(Self::from(child))
        }
        result
    }
}