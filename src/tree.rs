use tree_sitter::{Node as TreeNode, Tree as TreeSitter};
use std::collections::HashMap;
use rule::{RuleMut, Rule};
use std::str::Utf8Error;
use error::{MinusOneResult};
use std::ops;
use std::ops::BitOr;

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
    pub inner: TreeNode<'a>,

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

    fn apply(&mut self, rule: &mut (impl RuleMut<'a, Language=T>)) -> MinusOneResult<()> {
        rule.enter(self, BranchFlow::Unpredictable)?;
        let mut cursor = self.inner.walk();
        let current_node = self.inner;
        for child in self.inner.children(&mut cursor) {
            self.inner = child;
            self.apply(rule)?;
        }

        self.inner = current_node;
        rule.leave(self, BranchFlow::Unpredictable)?;
        Ok(())
    }

    fn apply_with_strategy(&mut self, rule: &mut (impl RuleMut<'a, Language=T>), strategy: &impl Strategy<T>, flow: BranchFlow) -> MinusOneResult<()> {
        let mut computed_flow = flow;
        match strategy.control(self.view())? {
            // Execution flow detected
            ControlFlow::Branch(new_flow) => {
                // We are computing the new statue of the execution flow
                // If it's predictable or not
                computed_flow = computed_flow | new_flow;
            },
            ControlFlow::Break => return Ok(()),
            ControlFlow::Continue => ()
        }

        rule.enter(self, computed_flow)?;

        let mut cursor = self.inner.walk();
        let current_node = self.inner;
        for child in self.inner.children(&mut cursor) {
            self.inner = child;
            self.apply_with_strategy(rule, strategy, computed_flow)?;
        }

        self.inner = current_node;

        rule.leave(self, computed_flow)?;
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
        for i in 0..self.node.child_count() {
            let child = self.node.child(i);
            if child == None {
                break
            }

            // ignore extra node when requesting child at particumar index
            if child.unwrap().is_extra() {
                continue;
            }

            if current == index {
                return Some(Node::new(child.unwrap(), self.source, self.storage));
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


    pub fn get_parent_of_types(&self, kinds: Vec<&str>) -> Option<Node<'a, T>> {
        let mut current = self.parent();
        loop {
            if let Some(current_node) = current {
                if kinds.contains(&current_node.kind()) {
                    return Some(current_node);
                }
                current = current_node.parent();
            }
            else {
                return None;
            }
        }
    }

    fn apply(&self, rule: &mut (impl Rule<'a, Language=T>)) -> MinusOneResult<()> {
        rule.enter(self)?;
        for child in self.iter() {
            child.apply(rule)?;
        }
        rule.leave(self)?;

        Ok(())
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
            if self.index >= end {
                return None;
            }
        }

        match self.inner.child(self.index) {
            Some(node) => {
                self.index += self.gap;
                Some(node)
            },
            None => None
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum BranchFlow {
    Predictable,
    Unpredictable
}

impl ops::BitOr<BranchFlow> for BranchFlow {
    type Output = BranchFlow;

    fn bitor(self, rhs: BranchFlow) -> Self::Output {
        match (self, rhs) {
            (BranchFlow::Predictable, BranchFlow::Predictable) => BranchFlow::Predictable,
            (BranchFlow::Predictable, BranchFlow::Unpredictable) => BranchFlow::Unpredictable,
            (BranchFlow::Unpredictable, BranchFlow::Predictable) => BranchFlow::Unpredictable,
            (BranchFlow::Unpredictable, BranchFlow::Unpredictable) => BranchFlow::Unpredictable,
        }
    }
}

pub enum ControlFlow {
    Branch(BranchFlow),
    Break,
    Continue
}

pub trait Strategy<T> {
    fn control(&self, node: Node<T>) -> MinusOneResult<ControlFlow>;
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
        node.apply(rule)
    }

    pub fn apply_mut_with_strategy<'b>(&'b mut self, rule: &mut (impl RuleMut<'b, Language=S::Component> + Sized), strategy: impl Strategy<S::Component>) -> MinusOneResult<()>{
        let mut node = NodeMut::new(self.tree_sitter.root_node(), self.source, &mut self.storage);
        node.apply_with_strategy(rule, &strategy, BranchFlow::Predictable)
    }

    pub fn apply<'b>(&'b self, rule: &mut (impl Rule<'b, Language=S::Component> + Sized)) -> MinusOneResult<()> {
        let mut node = Node::new(self.tree_sitter.root_node(), self.source, &self.storage);
        node.apply(rule)
    }

    pub fn root(&self) -> MinusOneResult<Node<S::Component>> {
        Ok(Node::new(self.tree_sitter.root_node(), self.source, &self.storage))
    }
}
