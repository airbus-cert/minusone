use tree_sitter::{Node, TreeCursor};
use std::collections::HashMap;
use core::rule::{RuleEngineMut, RuleMut};

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

pub struct NodeMut<'a, T> {
    node: Node<'a>,
    data: &'a mut ComponentHashMap<T>
}

impl<'a, T> NodeMut<'a, T> {
    pub fn new(root: Node<'a>, data: &'a mut ComponentHashMap<T>) -> Self {
        Self {
            node: root,
            data
        }
    }

    pub fn view(&self) -> NodeView<T> {
        NodeView::new(self.node, self.data)
    }

    pub fn borrow_for(&'a mut self, node: Node<'a>) -> Self {
        Self {
            node,
            data: self.data
        }
    }
}

impl<'a, T> AsMut<Option<T>> for NodeMut<'a, T> {
    fn as_mut(&mut self) -> &mut Option<T> {
        self.data.get_node_data_mut(self.node)
    }
}


pub trait RuleMut2<'a> {
    type Language;
    fn enter(&mut self, node: &mut NodeMut<'a, Self::Language>);
    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>);
}

pub trait VisitMut<'a, T> {
    fn visit(&mut self, node: &mut NodeMut<'a, T>);
}

impl<'a, T> VisitMut<'a, T> for RuleMut2<'a, Language = T> {
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

pub struct NodeView<'a, T> {
    node: Node<'a>,
    data: &'a ComponentHashMap<T>
}

impl<'a, T> NodeView<'a, T> {
    pub fn new(node: Node<'a>, data: &'a ComponentHashMap<T>) -> Self {
        Self {
            node,
            data
        }
    }

    pub fn child(&self, index: usize) -> NodeView<T> {
        Self::new(self.node.child(index).unwrap(), self.data)
    }
}


impl<'a, T>  AsRef<Option<T>> for NodeView<'a, T>{
    fn as_ref(&self) -> &Option<T> {
        self.data.get_node_data(self.node)
    }
}
