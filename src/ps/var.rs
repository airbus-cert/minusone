use scope::ScopeManager;
use ps::InferredValue;
use rule::{RuleMut, Rule};
use tree::{NodeMut, Node};
use error::MinusOneResult;

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
fn is_left<T>(node: Node<T>) -> bool {
    let mut current = node;
    loop {
        if let Some(parent_node) = current.parent() {
            if parent_node.kind() == "left_assignment_expression" {
                return true;
            }
            current = parent_node;
        }
        else {
            return false;
        }
    }
}

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
                    self.scope_manager.current().assign(&left, &right)?;
                }
            },
            "variable" => {
                // check if we are not on the left part of an assignment expression
                // The powershell grammar doesn't have a different node name for each part
                if !is_left(view) {
                    self.scope_manager.current().attach(node)?;
                }
            }
            _ => ()
        }
        Ok(())
    }
}