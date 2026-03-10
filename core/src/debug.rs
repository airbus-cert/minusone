use crate::error::MinusOneResult;
use crate::rule::Rule;
use crate::tree::Node;
use std::fmt::Debug;
use colored::Colorize;

/// A debug view is used to print the tree nodes
/// with associated inferred type
pub struct DebugView<T> {
    tab_depth: u32,
    tab_size: u32,
    with_text: bool,
    with_childs_count: bool,
    with_colors: bool,
    _use: Option<T>,
}

impl<T> Default for DebugView<T> {
    fn default() -> Self {
        Self {
            tab_depth: 0,
            tab_size: 2,
            with_text: true,
            with_childs_count: true,
            with_colors: true,
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
/// let mut debug_view = DebugView::default();
/// tree.apply(&mut debug_view).unwrap(); // it will print you the tree over the console
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

        for _ in 0..self.tab_depth * self.tab_size {
            print!(" ");
        }

        //print!("{}", "(".green());
        if self.with_colors {
            match self.tab_depth % 6 {
                0 => print!("{}", "(".green()),
                1 => print!("{}", "(".yellow()),
                2 => print!("{}", "(".blue()),
                3 => print!("{}", "(".red()),
                4 => print!("{}", "(".cyan()),
                _ => print!("{}", "(".magenta()),
            }
        } else {
            print!("(");
        }

        //print!("({} inferred_type: {:?} | childs : {} | text: {:?}", node.kind(), node.data(), node.child_count(), node.text());
        print!("{} inferred_type: {:?}", node.kind(), node.data());
        if self.with_childs_count {
            print!(" | childs : {}", node.child_count());
        }
        if self.with_text {
            print!(" | text: {:?}", node.text());
        }

        self.tab_depth += 1;
        Ok(true)
    }

    /// During the down to top travel we will manage the tab decrement
    fn leave(&mut self, _node: &Node<'a, Self::Language>) -> MinusOneResult<()> {
        if self.with_colors {
            match self.tab_depth % 6 {
                0 => print!("{}", ")".green()),
                1 => print!("{}", ")".yellow()),
                2 => print!("{}", ")".blue()),
                3 => print!("{}", ")".red()),
                4 => print!("{}", ")".cyan()),
                _ => print!("{}", ")".magenta()),
            }
        } else {
            print!(")");
        }
        self.tab_depth -= 1;
        Ok(())
    }
}
