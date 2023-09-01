use tree_sitter::{Node as TreeNode, Tree as TreeSitter};
use std::collections::HashMap;
use rule::{RuleMut, Rule};
use std::str::Utf8Error;
use error::{MinusOneResult};

/// Node components are stored following
/// a storage pattern
pub trait Storage {
    type Component;

    /// Retrieve the associated data for a node
    ///
    /// This is the non mutable interface
    /// The date is optional
    fn get(&self, node: TreeNode) -> Option<&Self::Component>;

    /// Retrieve the associated date for a node, and allow update
    ///
    /// This is the mutable version
    fn set(&mut self, node: TreeNode, data: Self::Component);

    /// Start a update round
    fn start(&mut self);

    /// End an update round
    fn end(&mut self) -> bool;
}

/// A possible implementation of storage that
/// use Hash map as link between node id and data
pub struct HashMapStorage<T> {
    map : HashMap<usize, T>,
    is_updated : bool
}

/// Default trait is used for the tree implementation
impl<T> Default for HashMapStorage<T> {
    fn default() -> Self {
        Self {
            map : HashMap::new(),
            is_updated: false
        }
    }
}


/// Storage implementation for the HashMap storage
impl<T> Storage for HashMapStorage<T> {
    type Component = T;

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
    /// use minusone::tree::{Storage, HashMapStorage};
    ///
    /// let mut parser = Parser::new();
    /// parser.set_language(powershell_language()).unwrap();
    ///
    /// let ts_tree = parser.parse("4+5", None).unwrap();
    /// let mut storage = HashMapStorage::<u32>::default();
    ///
    /// assert_eq!(storage.get(ts_tree.root_node()), None)
    /// ```
    fn get(&self, node: TreeNode) -> Option<&Self::Component> {
        if ! self.map.contains_key(&node.id()) {
            return None;
        }
        Some(self.map.get(&node.id()).unwrap())
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
    /// use minusone::tree::{Storage, HashMapStorage};
    ///
    /// let mut parser = Parser::new();
    /// parser.set_language(powershell_language()).unwrap();
    ///
    /// let ts_tree = parser.parse("4+5", None).unwrap();
    /// let mut storage = HashMapStorage::<u32>::default();
    ///
    /// storage.set(ts_tree.root_node(), 42);
    ///
    /// assert_eq!(storage.get(ts_tree.root_node()), Some(&42))
    /// ```
    fn set(&mut self, node: TreeNode, data: Self::Component) {
        self.map.insert(node.id(), data);
        self.is_updated = true;
    }

    /// use to monitor if the storage was updated
    /// between the start and the end function
    ///
    /// # Example
    /// ```
    /// extern crate tree_sitter;
    /// extern crate tree_sitter_powershell;
    /// extern crate minusone;
    ///
    /// use tree_sitter::{Parser, Language};
    /// use tree_sitter_powershell::language as powershell_language;
    /// use minusone::tree::{Storage, HashMapStorage};
    ///
    /// let mut parser = Parser::new();
    /// parser.set_language(powershell_language()).unwrap();
    ///
    /// let ts_tree = parser.parse("4+5", None).unwrap();
    /// let mut storage = HashMapStorage::<u32>::default();
    ///
    /// storage.start();
    /// storage.set(ts_tree.root_node(), 42);
    ///
    /// assert_eq!(storage.end(), true)
    /// ```
    fn start(&mut self) {
        self.is_updated = false;
    }

    fn end(&mut self) -> bool {
        self.is_updated
    }
}

/// An interface to manage mutablility of one Node
///
/// The idea is to visit the entire tree, allow view any nodes
/// But only change value of the current one
pub struct NodeMut<'a, T> {
    /// The current tree-sitter node
    inner: TreeNode<'a>,

    /// reference to the original source code
    source: &'a [u8],

    /// Reference to the storage
    storage: &'a mut dyn Storage<Component=T>
}

/// NodeMut methods
impl<'a, T> NodeMut<'a, T> {
    pub fn new(root: TreeNode<'a>, source: &'a [u8], storage: &'a mut dyn Storage<Component=T>) -> Self {
        Self {
            inner: root,
            source,
            storage
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
    /// use minusone::tree::{Storage, HashMapStorage, NodeMut};
    ///
    /// let mut parser = Parser::new();
    /// parser.set_language(powershell_language()).unwrap();
    ///
    /// let source = "4+5";
    /// let ts_tree = parser.parse(source, None).unwrap();
    ///
    /// let mut storage = HashMapStorage::<u32>::default();
    ///
    /// let mut node = NodeMut::new(ts_tree.root_node(), source.as_bytes(), &mut storage);
    ///
    /// let node_view = node.view();
    ///
    /// assert_eq!(node_view.kind(), "program");
    /// ```
    pub fn view(&self) -> Node<T> {
        Node::new(self.inner, self.source, self.storage)
    }

    /// Set a data to a node
    ///
    /// # Example
    /// ```
    /// extern crate tree_sitter;
    /// extern crate tree_sitter_powershell;
    /// extern crate minusone;
    ///
    /// use tree_sitter::{Parser, Language};
    /// use tree_sitter_powershell::language as powershell_language;
    /// use minusone::tree::{Storage, HashMapStorage, NodeMut};
    ///
    /// let mut parser = Parser::new();
    /// parser.set_language(powershell_language()).unwrap();
    ///
    /// let source = "4+5";
    /// let ts_tree = parser.parse(source, None).unwrap();
    ///
    /// let mut storage = HashMapStorage::<u32>::default();
    ///
    /// let mut node = NodeMut::new(ts_tree.root_node(), source.as_bytes(), &mut storage);
    ///
    /// node.set(42);
    ///
    /// assert_eq!(node.view().data(), Some(&42));
    /// ```
    pub fn set(&mut self, data: T) {
        self.storage.set(self.inner, data)
    }
}

/// Interface use to visit a tree
/// with mutability capability on node component (not on the node itself)
pub trait VisitMut<'a, T> {
    fn visit(&mut self, node: &mut NodeMut<'a, T>) -> MinusOneResult<()>;
}


/// We implement the VisitMut Trait for RuleMut (Rule that want mutability)
///
/// # Example
/// ```
/// extern crate tree_sitter;
/// extern crate tree_sitter_powershell;
/// extern crate minusone;
///
/// use tree_sitter::{Parser, Language};
/// use tree_sitter_powershell::language as powershell_language;
/// use minusone::tree::{Storage, HashMapStorage, NodeMut, VisitMut};
/// use minusone::rule::RuleMut;
/// use minusone::ps::InferredValue;
/// use minusone::error::MinusOneResult;
///
/// #[derive(Default)]
/// pub struct LazzyRule;
///
/// impl<'a> RuleMut<'a> for LazzyRule {
///     type Language = u32;
///
///     fn enter(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
///         if node.view().kind() == "program" {
///             node.set(42)
///         }
///         Ok(())
///     }
///
///     fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
///         Ok(())
///     }
/// }
///
/// let mut parser = Parser::new();
/// parser.set_language(powershell_language()).unwrap();
///
/// let source = "4";
/// let ts_tree = parser.parse(source, None).unwrap();
///
/// let mut storage = HashMapStorage::<u32>::default();
///
/// let mut node = NodeMut::new(ts_tree.root_node(), source.as_bytes(), &mut storage);
///
/// LazzyRule::default().visit(&mut node);
/// assert_eq!(node.view().data(), Some(&42));
/// ```
impl<'a, T, X> VisitMut<'a, T> for X where X : RuleMut<'a, Language = T>{
    fn visit(&mut self, node: &mut NodeMut<'a, T>) -> MinusOneResult<()> {
        self.enter(node)?;
        let mut cursor = node.inner.walk();
        let current_node = node.inner;
        for child in node.inner.children(&mut cursor) {
            node.inner = child;
            self.visit(node)?;
        }

        node.inner = current_node;
        self.leave(node)?;
        Ok(())
    }
}

/// A node view use to explore the tree
/// without mutability
pub struct Node<'a, T> {
    /// The inner tree-sitter node
    node: TreeNode<'a>,
    /// Source reference
    source: &'a [u8],
    /// The associate component storage
    storage: &'a dyn Storage<Component=T>
}

/// Two nodes are equals if they have the same node id
impl<'a, T> PartialEq for Node<'a, T> {
    fn eq(&self, other: &Self) -> bool {
        self.node.id() == other.node.id()
    }
}

impl<'a, T> Node<'a, T> {
    pub fn new(node: TreeNode<'a>, source: &'a [u8], storage: &'a dyn Storage<Component=T>) -> Self {
        Self {
            node,
            source,
            storage
        }
    }

    pub fn child(&self, index: usize) -> Option<Node<'a, T>> {
        let mut current = 0;
        for child in self.iter() {
            // ignore extra node when requesting child at particumar index
            if child.is_extra() {
                continue;
            }
            if current == index {
                return Some(child);
            }
            current += 1;
        }
        return None;
    }

    pub fn named_child(&self, index: &str) -> Option<Node<'a, T>> {
        self.node.child_by_field_name(index).map(|node| Node::new(node, self.source, self.storage))
    }

    pub fn iter(&self) -> NodeIterator<'a, T> {
        NodeIterator::new(
            Self::new(self.node, self.source, self.storage),
            0,
            None,
            1
        )
    }

    pub fn range(&self, start: Option<usize>, end: Option<usize>, gap: Option<usize>) -> NodeIterator<'a, T> {
        NodeIterator::new(
            Self::new(self.node, self.source, self.storage),
            start.unwrap_or(0),
            end,
            gap.unwrap_or(1)
        )
    }

    pub fn kind(&self) -> &'static str {
        self.node.kind()
    }

    pub fn is_extra(&self) -> bool {
        self.node.is_extra()
    }

    pub fn child_count(&self) -> usize {
        // we have to count only usable node
        let mut result = 0;
        for _ in self.iter() {
            result += 1;
        }
        result
    }

    pub fn data(&self) -> Option<&T>{
        self.storage.get(self.node)
    }

    pub fn text(&self) -> Result<&str, Utf8Error>{
        self.node.utf8_text(self.source)
    }

    pub fn parent(&self) -> Option<Node<'a, T>> {
        self.node.parent().map(|node| Self::new(node, self.source, self.storage))
    }
}

pub struct NodeIterator<'a, T> {
    inner: Node<'a, T>,
    index: usize,
    end: Option<usize>,
    gap : usize
}

impl<'a, T> NodeIterator<'a, T> {
    fn new(node: Node<'a, T>, start: usize, end: Option<usize>, gap: usize) -> Self{
        Self {
            inner: node,
            index : start,
            end,
            gap
        }
    }
}

impl<'a, T> Iterator for NodeIterator<'a, T> {
    type Item = Node<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(end) = self.end {
            if self.index > end {
                return None;
            }
        }

        match self.inner.node.child(self.index) {
            Some(node) => {
                self.index += self.gap;
                Some(Node::new(node, self.inner.source, self.inner.storage))
            },
            None => None
        }
    }
}

pub trait Visit<'a, T> {
    fn visit(&mut self, node: Node<'a, T>) -> MinusOneResult<()>;
}

impl<'a, T, X> Visit<'a, T> for X where X : Rule<'a, Language = T>{
    fn visit(&mut self, node: Node<'a, T>) -> MinusOneResult<()> {
        if self.enter(&node)? {
            for child in node.iter() {
                self.visit(child)?;
            }
            self.leave(&node)?;
        }
        Ok(())
    }
}

pub struct Tree<'a, S : Storage> {
    storage: S,
    tree_sitter: TreeSitter,
    source: &'a[u8]
}

impl<'a, S> Tree<'a, S> where S : Storage + Default {
    pub fn new(source: &'a[u8], tree_sitter: TreeSitter) -> Self {
        Self {
            storage: S::default(),
            tree_sitter,
            source
        }
    }

    pub fn apply_mut<'b>(&'b mut self, rule: &mut (impl RuleMut<'b, Language=S::Component> + Sized)) -> MinusOneResult<()>{
        let mut node = NodeMut::new(self.tree_sitter.root_node(), self.source, &mut self.storage);
        rule.visit(&mut node)
    }

    pub fn apply<'b>(&'b self, rule: &mut (impl Rule<'b, Language=S::Component> + Sized)) -> MinusOneResult<()> {
        rule.visit(Node::new(self.tree_sitter.root_node(), self.source, &self.storage))
    }

    pub fn root(&self) -> MinusOneResult<Node<S::Component>> {
        Ok(Node::new(self.tree_sitter.root_node(), self.source, &self.storage))
    }
}
