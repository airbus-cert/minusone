use scope::ScopeManager;
use ps::InferredValue;
use rule::{RuleMut};
use tree::{NodeMut, Node};
use error::MinusOneResult;
use std::env::current_exe;

pub struct Var {
    scope_manager : ScopeManager<InferredValue>
}

impl Default for Var {
    fn default() -> Self {
        Var {
            scope_manager: ScopeManager::new()
        }
    }
}

/// This function
fn is_left<T>(node: &Node<T>) -> bool {
    let mut current = node.parent();
    loop {
        if let Some(current_node) = current {
            if current_node.kind() == "left_assignment_expression" {
                return true;
            }
            current = current_node.parent();
        }
        else {
            return false;
        }
    }
}

fn find_variable_node<'a, T>(node: &Node<'a, T>) -> Option<Node<'a, T>> {
    for child in node.iter() {
        if child.kind() == "variable" {
            return Some(child);
        }
        else if let Some(new_node) = find_variable_node(&child){
            return Some(new_node)
        }
    }
    None
}

/// Forward will just forward inferred type in case of very simple
/// tree exploring
///
/// # Example
/// ```
/// extern crate tree_sitter;
/// extern crate tree_sitter_powershell;
/// extern crate minusone;
///
/// use minusone::tree::{HashMapStorage, Tree};
/// use minusone::ps::from_powershell_src;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::InferredValue::Number;
/// use minusone::ps::integer::ParseInt;
/// use minusone::ps::var::Var;
/// use minusone::ps::litter::PowershellLitter;
///
/// let mut tree = from_powershell_src("\
/// $foo = 4
/// Write-Debug $foo
/// ").unwrap();
/// tree.apply_mut(&mut (ParseInt::default(), Forward::default(), Var::default())).unwrap();
///
/// let mut ps_litter_view = PowershellLitter::new();
/// ps_litter_view.print(&tree.root().unwrap()).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\
/// $foo = 4
/// Write-Debug 4
/// ");
/// ```
impl<'a> RuleMut<'a> for Var {
    type Language = InferredValue;

    fn enter(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        let view = node.view();
        match view.kind() {
            "program" => self.scope_manager.reset(),
            "function_statement" => self.scope_manager.enter(),
            "}" => {
                if let Some(parent) = view.parent() {
                    if parent.kind() == "function_statement" {
                        self.scope_manager.leave()
                    }
                }
            },
            _ => ()
        }
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        let view = node.view();
        match view.kind() {
            "assignment_expression" => {
                // Assign var value if it's possible
                if let (Some(left), Some(right)) = (view.child(0), view.child(2)) {
                    if let Some(var) = find_variable_node(&left) {
                        if let Some(data) = right.data() {
                            self.scope_manager.current().assign(var.text()?, data.clone());
                        }
                        else {
                            // forget the value, we were not able to follow the value
                            self.scope_manager.current().forget(var.text()?);
                        }
                    }
                }
            },
            "variable" => {
                // check if we are not on the left part of an assignment expression
                // The powershell grammar doesn't make a difference node name for each part
                if view.parent_from_type("left_assignment_expression") == None {
                    if let Some(data) = self.scope_manager.current().get_current_var(view.text()?) {
                        node.set(data);
                    }
                }
            }
            _ => ()
        }
        Ok(())
    }
}