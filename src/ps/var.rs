use scope::ScopeManager;
use ps::Powershell;
use rule::{RuleMut};
use tree::{NodeMut, Node};
use error::MinusOneResult;


/// Var is a variable manager that will try to track
/// static var assignement and propagte it in the code
/// when it's possible
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
/// use minusone::ps::integer::ParseInt;
/// use minusone::ps::var::Var;
/// use minusone::ps::litter::Litter;
///
/// let mut tree = from_powershell_src("\
/// $foo = 4
/// Write-Debug $foo\
/// ").unwrap();
/// tree.apply_mut(&mut (ParseInt::default(), Forward::default(), Var::default())).unwrap();
///
/// let mut ps_litter_view = Litter::new();
/// ps_litter_view.print(&tree.root().unwrap()).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\
/// $foo = 4
/// Write-Debug 4\
/// ");
/// ```
pub struct Var {
    scope_manager : ScopeManager<Powershell>
}

impl Default for Var {
    fn default() -> Self {
        Var {
            scope_manager: ScopeManager::new()
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

impl<'a> RuleMut<'a> for Var {
    type Language = Powershell;

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
                if view.get_parent_of_type("left_assignment_expression") == None {
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

#[cfg(test)]
mod test {
    use super::*;
    use ps::from_powershell_src;
    use ps::integer::ParseInt;
    use ps::forward::Forward;
    use ps::Powershell::Raw;
    use ps::Value::Num;


    #[test]
    fn test_static_replacement() {
        let mut tree = from_powershell_src("$foo = 4\nWrite-Debug $foo").unwrap();

        tree.apply_mut(&mut (ParseInt::default(), Forward::default(), Var::default())).unwrap();

        // We are waiting for
        // Write-Debug 4
        // (pipeline inferred_type: None
        //   (command inferred_type: None
        //    (command_name inferred_type: None)
        //    (variable inferred_type: Some(Number(4))))))
        assert_eq!(*tree.root().unwrap()
            .child(1).unwrap() // pipeline
            .child(0).unwrap() //command
            .child(1).unwrap()// variable
            .data().expect("Expecting inferred type"), Raw(Num(4))
        );
    }

    #[test]
    fn test_unfollow_var_use_unknow_var() {
        let mut tree = from_powershell_src("$foo = $toto\nWrite-Debug $foo").unwrap();

        tree.apply_mut(&mut (ParseInt::default(), Forward::default(), Var::default())).unwrap();

        // We are waiting for
        // Write-Debug 4
        // (pipeline inferred_type: None
        //   (command inferred_type: None
        //    (command_name inferred_type: None)
        //    (variable inferred_type: Some(Number(4))))))
        assert_eq!(tree.root().unwrap()
            .child(1).unwrap() // pipeline
            .child(0).unwrap() //command
            .child(1).unwrap()// variable
            .data(), None
        );
    }
}