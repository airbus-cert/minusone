use tree_sitter::{Node as TreeNode, TreeCursor};
use std::collections::HashMap;
use core::rule::{RuleMut, Rule};

pub trait ComponentDb<T> {
    fn init_from(&mut self, node: TreeNode);
    fn get_node_data(&self, node: TreeNode) -> &Option<T>;
    fn get_node_data_mut(&mut self, node: TreeNode) -> &mut Option<T>;
}

pub type ComponentHashMap<T> = HashMap<usize, Option<T>>;

impl<T> ComponentDb<T> for ComponentHashMap<T> {

    fn init_from(&mut self, node: TreeNode) {
        self.insert(node.id(), None);
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.init_from(child);
        }
    }

    fn get_node_data(&self, node: TreeNode) -> &Option<T> {
        self.get(&node.id()).unwrap()
    }

    fn get_node_data_mut(&mut self, node: TreeNode) -> &mut Option<T> {
        self.get_mut(&node.id()).unwrap()
    }
}

pub struct NodeMut<'a, T> {
    node: TreeNode<'a>,
    data: &'a mut ComponentHashMap<T>
}

impl<'a, T> NodeMut<'a, T> {
    pub fn new(root: TreeNode<'a>, data: &'a mut ComponentHashMap<T>) -> Self {
        Self {
            node: root,
            data
        }
    }

    pub fn view(&self) -> Node<T> {
        Node::new(self.node, self.data)
    }
}

impl<'a, T> AsMut<Option<T>> for NodeMut<'a, T> {
    fn as_mut(&mut self) -> &mut Option<T> {
        self.data.get_node_data_mut(self.node)
    }
}

pub trait VisitMut<'a, T> {
    fn visit(&mut self, node: &mut NodeMut<'a, T>);
}

impl<'a, T, X : RuleMut<'a, Language = T>> VisitMut<'a, T> for X {
    fn visit(&mut self, node: &mut NodeMut<'a, T>) {
        self.enter(node);
        let mut cursor = node.node.walk();
        let current_node = node.node;
        for child in node.node.children(&mut cursor) {
            node.node = child;
            self.visit(node);
        }

        node.node = current_node;
        self.leave(node);
    }
}

pub struct Node<'a, T> {
    node: TreeNode<'a>,
    data: &'a ComponentHashMap<T>
}

impl<'a, T> Node<'a, T> {
    pub fn new(node: TreeNode<'a>, data: &'a ComponentHashMap<T>) -> Self {
        Self {
            node,
            data
        }
    }

    pub fn child(&self, index: usize) -> Node<'a, T> {
        Self::new(self.node.child(index).unwrap(), self.data)
    }

    pub fn iter(&self) -> NodeIterator<'a, T> {
        NodeIterator::new(Self::new(self.node, self.data))
    }

    pub fn kind(&self) -> &'static str {
        self.node.kind()
    }

    pub fn child_count(&self) -> usize {
        self.node.child_count()
    }
}

pub struct NodeIterator<'a, T> {
    node: Node<'a, T>,
    index: usize
}

impl<'a, T> NodeIterator<'a, T> {
    fn new(node: Node<'a, T>) -> Self{
        Self {
            node,
            index : 0
        }
    }
}

impl<'a, T> Iterator for NodeIterator<'a, T> {
    type Item = Node<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        let current_index = self.index;
        self.index += 1;

        if current_index >= self.node.node.child_count() {
            return None;
        }
        Some(self.node.child(current_index))
    }
}


impl<'a, T>  AsRef<Option<T>> for Node<'a, T>{
    fn as_ref(&self) -> &Option<T> {
        self.data.get_node_data(self.node)
    }
}

pub trait Visit<'a, T> {
    fn visit(&mut self, node: Node<'a, T>);
}

impl<'a, T, X: Rule<'a, Language = T>> Visit<'a, T> for X {
    fn visit(&mut self, node: Node<'a, T>) {
        self.enter(&node);
        for child in node.iter() {
            self.visit(child);
        }
        self.leave(&node);
    }
}
