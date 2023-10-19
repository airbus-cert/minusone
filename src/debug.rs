use rule::Rule;
use tree::{Node};
use ps::Powershell;
use error::MinusOneResult;

/// A debug view is used to print the tree nodes
/// with associated inferred type
pub struct DebugView {
    tab_space: u32
}

impl DebugView {
    pub fn new() -> Self {
        DebugView {
            tab_space: 0
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
/// extern crate minusone;
///
/// use minusone::tree::{HashMapStorage, Tree};
/// use minusone::ps::build_powershell_tree;
/// use minusone::debug::DebugView;
///
/// let mut tree = build_powershell_tree("4").unwrap();
/// tree.apply(&mut DebugView::new()).unwrap(); // it will print you the tree over the console
///
/// ```
impl<'a> Rule<'a> for DebugView {
    type Language = Powershell;

    /// During the top down travel we will manage tab
    /// increment and general print
    fn enter(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<bool>{
        println!();

        for _ in 0..self.tab_space {
            print!(" ");
        }

        print!("({} inferred_type: {:?}", node.kind(), node.data());

        self.tab_space += 1;
        Ok(true)
    }

    /// During the down to top travel we will manage the tab decrement
    fn leave(&mut self, _node: &Node<'a, Self::Language>) -> MinusOneResult<()>{
        print!(")");
        self.tab_space -= 1;
        Ok(())
    }
}