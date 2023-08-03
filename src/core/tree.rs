use tree_sitter::{Node as TreeNode};
use std::collections::HashMap;
use core::rule::{RuleMut, Rule};

/// Node components are stored following
/// a storage pattern
pub trait Storage {
    type Component;

    /// Reserve component data for each node
    ///
    /// The parameter is usually the root node of the tree
    fn reserve(&mut self, root: TreeNode);

    /// Retrieve the associated data for a node
    ///
    /// This is the non mutable interface
    /// The date is optional
    fn get(&self, node: TreeNode) -> &Option<Self::Component>;

    /// Retrieve the associated date for a node, and allow update
    ///
    /// This is the mutable version
    fn get_mut(&mut self, node: TreeNode) -> &mut Option<Self::Component>;
}

/// A possible implementation of storage that
/// use Hash map as link between node id and data
pub struct HashMapStorage<T>(HashMap<usize, Option<T>>);

/// Default trait is used for the tree implementation
impl<T> Default for HashMapStorage<T> {
    fn default() -> Self {
        Self {
            0: HashMap::new()
        }
    }
}


/// Storage implementation for the HashMap storage
impl<T> Storage for HashMapStorage<T> {
    type Component = T;

    /// It will crate an entry for each node
    /// into the hashmap
    fn reserve(&mut self, root: TreeNode) {
        self.0.insert(root.id(), None);
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            self.reserve(child);
        }
    }

    /// It will get a reference to the associated data of the node
    ///
    /// # Example
    /// ```
    /// extern crate tree_sitter;
    /// extern crate tree_sitter_powershell;
    /// extern crate minusone;
    ///
    /// use tree_sitter::{Parser, Language};
    /// use tree_sitter_powershell::language as powershell_language;
    /// use minusone::core::tree::{Storage, HashMapStorage};
    ///
    /// let mut parser = Parser::new();
    /// parser.set_language(powershell_language()).unwrap();
    ///
    /// let ts_tree = parser.parse("4+5", None).unwrap();
    /// let mut storage = HashMapStorage::<u32>::default();
    /// storage.reserve(ts_tree.root_node());
    ///
    /// assert_eq!(*storage.get(ts_tree.root_node()), None)
    /// ```
    fn get(&self, node: TreeNode) -> &Option<Self::Component> {
        self.0.get(&node.id()).unwrap()
    }

    /// It will get a reference to the associated data of the node
    ///
    /// # Example
    /// ```
    /// extern crate tree_sitter;
    /// extern crate tree_sitter_powershell;
    /// extern crate minusone;
    ///
    /// use tree_sitter::{Parser, Language};
    /// use tree_sitter_powershell::language as powershell_language;
    /// use minusone::core::tree::{Storage, HashMapStorage};
    ///
    /// let mut parser = Parser::new();
    /// parser.set_language(powershell_language()).unwrap();
    ///
    /// let ts_tree = parser.parse("4+5", None).unwrap();
    /// let mut storage = HashMapStorage::<u32>::default();
    /// storage.reserve(ts_tree.root_node());
    ///
    /// *storage.get_mut(ts_tree.root_node()) = Some(42);
    ///
    /// assert_eq!(*storage.get(ts_tree.root_node()), Some(42))
    /// ```
    fn get_mut(&mut self, node: TreeNode) -> &mut Option<Self::Component> {
        self.0.get_mut(&node.id()).unwrap()
    }
}

/// An interface to manage mutablility of one Node
///
/// The idea is to visit the entire tree, allow view any nodes
/// But only change value of the current one
pub struct NodeMut<'a, T> {
    /// The current tree-sitter node
    inner: TreeNode<'a>,

    /// Reference to the storage
    storage: &'a mut dyn Storage<Component=T>
}

/// NodeMut methods
impl<'a, T> NodeMut<'a, T> {
    pub fn new(root: TreeNode<'a>, data: &'a mut dyn Storage<Component=T>) -> Self {
        Self {
            inner: root,
            storage: data
        }
    }

    /// A const view of the mutable node
    /// Use to read and navigate over the tree
    ///
    /// # Example
    /// ```
    /// extern crate tree_sitter;
    /// extern crate tree_sitter_powershell;
    /// extern crate minusone;
    ///
    /// use tree_sitter::{Parser, Language};
    /// use tree_sitter_powershell::language as powershell_language;
    /// use minusone::core::tree::{Storage, HashMapStorage, NodeMut};
    ///
    /// let mut parser = Parser::new();
    /// parser.set_language(powershell_language()).unwrap();
    ///
    /// let ts_tree = parser.parse("4+5", None).unwrap();
    ///
    /// let mut storage = HashMapStorage::<u32>::default();
    /// storage.reserve(ts_tree.root_node());
    ///
    /// let mut node = NodeMut::new(ts_tree.root_node(), &mut storage);
    ///
    /// let node_view = node.view();
    ///
    /// assert_eq!(node_view.kind(), "program");
    /// ```
    pub fn view(&self) -> Node<T> {
        Node::new(self.inner, self.storage)
    }
}

impl<'a, T> AsMut<Option<T>> for NodeMut<'a, T> {
    fn as_mut(&mut self) -> &mut Option<T> {
        self.storage.get_mut(self.inner)
    }
}

pub trait VisitMut<'a, T> {
    fn visit(&mut self, node: &mut NodeMut<'a, T>);
}

impl<'a, T, X> VisitMut<'a, T> for X where X : RuleMut<'a, Language = T>{
    fn visit(&mut self, node: &mut NodeMut<'a, T>) {
        self.enter(node);
        let mut cursor = node.inner.walk();
        let current_node = node.inner;
        for child in node.inner.children(&mut cursor) {
            node.inner = child;
            self.visit(node);
        }

        node.inner = current_node;
        self.leave(node);
    }
}

pub struct Node<'a, T> {
    node: TreeNode<'a>,
    data: &'a dyn Storage<Component=T>
}

impl<'a, T> Node<'a, T> {
    pub fn new(node: TreeNode<'a>, data: &'a dyn Storage<Component=T>) -> Self {
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
        self.data.get(self.node)
    }
}

pub trait Visit<'a, T> {
    fn visit(&mut self, node: Node<'a, T>);
}

impl<'a, T, X> Visit<'a, T> for X where X : Rule<'a, Language = T>{
    fn visit(&mut self, node: Node<'a, T>) {
        self.enter(&node);
        for child in node.iter() {
            self.visit(child);
        }
        self.leave(&node);
    }
}

pub struct Tree<'a, S : Storage> {
    storage: S,
    root: TreeNode<'a>,
}

impl<'a, S> Tree<'a, S> where S : Storage + Default {
    pub fn new(root: TreeNode<'a>) -> Self {
        let mut storage = S::default();
        storage.reserve(root);

        Self {
            storage,
            root
        }
    }

    pub fn apply_mut<'b>(&'b mut self, mut rule: (impl RuleMut<'b, Language=S::Component> + Sized)) {
        let mut node = NodeMut::new(self.root, &mut self.storage);
        rule.visit(&mut node);
    }

    pub fn apply<'b>(&'b self, mut rule: (impl Rule<'b, Language=S::Component> + Sized)) {
        rule.visit(Node::new(self.root, &self.storage));
    }
}
