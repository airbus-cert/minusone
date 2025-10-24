use crate::error::MinusOneResult;
use crate::rule::Rule;
use crate::tree::Node;
use std::fmt::Debug;

/// A debug view is used to print the tree nodes
/// with associated inferred type
pub struct DebugView<T> {
    tab_space: u32,
    _use: Option<T>,
}

impl<T> Default for DebugView<T> {
    fn default() -> Self {
        Self {
            tab_space: 0,
            _use: None,
        }
    }
}

/// A non mutable rule is enough to print the state of the
/// inferred tree
///
/// # Example
/// ```
/// extern crate tree_sitter;
/// extern crate tree_sitter_powershell;
/// use minusone::tree::{HashMapStorage, Tree};
/// use minusone::ps::{build_powershell_tree, Powershell};
/// use minusone::debug::DebugView;
///
/// let mut tree = build_powershell_tree("4").unwrap();
/// let mut debub_view = DebugView::default();
/// tree.apply(&mut debub_view).unwrap(); // it will print you the tree over the console
///
/// ```
impl<'a, T> Rule<'a> for DebugView<T>
where
    T: Debug,
{
    type Language = T;

    /// During the top down travel we will manage tab
    /// increment and general print
    fn enter(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<bool> {
        println!();

        for _ in 0..self.tab_space {
            print!(" ");
        }

        print!("({} inferred_type: {:?}", node.kind(), node.data());

        self.tab_space += 1;
        Ok(true)
    }

    /// During the down to top travel we will manage the tab decrement
    fn leave(&mut self, _node: &Node<'a, Self::Language>) -> MinusOneResult<()> {
        print!(")");
        self.tab_space -= 1;
        Ok(())
    }
}
