use scope::ScopeManager;
use ps::Powershell;
use rule::{RuleMut};
use tree::{NodeMut, Node};
use error::{MinusOneResult, Error};
use ps::Powershell::{Raw, Bool};
use ps::Value::{Str, Num};


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
/// use minusone::ps::linter::Linter;
///
/// let mut tree = from_powershell_src("\
/// $foo = 4
/// Write-Debug $foo\
/// ").unwrap();
/// tree.apply_mut(&mut (ParseInt::default(), Forward::default(), Var::default())).unwrap();
///
/// let mut ps_litter_view = Linter::new();
/// ps_litter_view.print(&tree.root().unwrap()).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\
/// $foo = 4
/// write-debug 4\
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
            if let Some(parent) = child.parent() {
                if parent.kind() == "unary_expression" {
                    return Some(child);
                }
            }
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
            "function_statement" | "statement_block" => self.scope_manager.enter(),
            "}" => {
                if let Some(parent) = view.parent() {
                    if ["function_statement", "statement_block"].contains(&parent.kind()) {
                        self.scope_manager.leave();
                    }
                }
            },
            // in the enter function because pre increment before assigned
            "pre_increment_expression" => {
                if let Some(variable) = view.child(1).ok_or(Error::invalid_child())?.child(0) {
                    if let Some(Raw(Num(v))) = self.scope_manager.current().get_var_mut(variable.text()?.to_lowercase().as_str()) {
                        *v = *v + 1;
                    }
                }
            },
            "pre_decrement_expression" => {
                if let Some(variable) = view.child(1).ok_or(Error::invalid_child())?.child(0) {
                    if let Some(Raw(Num(v))) = self.scope_manager.current().get_var_mut(variable.text()?.to_lowercase().as_str()) {
                        *v = *v - 1;
                    }
                }
            }
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
                        // make assignment
                        if let Some(data) = right.data() {
                            // If assignment is done on already known variable in stack
                            // we have to forget it because we will infer var from another scope
                            if self.scope_manager.is_known_in_stack(var.text()?.to_lowercase().as_str()) {
                                self.scope_manager.forget_everywhere(var.text()?.to_lowercase().as_str())
                            }
                            self.scope_manager.current().assign(var.text()?.to_lowercase().as_str(), data.clone());
                        }
                        // forget the value, we were not able to follow the value
                        else {
                            self.scope_manager.current().forget(var.text()?.to_lowercase().as_str());
                        }
                    }
                }
            },
            "variable" => {
                // check if we are not on the left part of an assignment expression
                // already handle by the previous case
                // We also exclude member_access for now
                if view.get_parent_of_types(vec!["left_assignment_expression", "member_access"]) == None {
                    // Try to assign variable member
                    if let Some(data) = self.scope_manager.current().get_var_mut(view.text()?.to_lowercase().as_str()) {
                        node.set(data.clone());
                    }
                }
            },
            // pre_increment_expression is saf eto forward due to the enter management
            "pre_increment_expression" | "pre_decrement_expression" => {
                if let Some(expression) = view.child(1) {
                    if let Some(expression_data) = expression.data() {
                        node.set(expression_data.clone())
                    }
                }
            },
            _ => ()
        }
        Ok(())
    }
}


/// Static Var rule is used to replace
/// Variable by its static and predictable value
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
/// use minusone::ps::linter::Linter;
///
/// let mut tree = from_powershell_src("\
/// $foo = 4
/// Write-Debug $foo\
/// ").unwrap();
/// tree.apply_mut(&mut (ParseInt::default(), Forward::default(), Var::default())).unwrap();
///
/// let mut ps_litter_view = Linter::new();
/// ps_litter_view.print(&tree.root().unwrap()).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\
/// $foo = 4
/// write-debug 4\
/// ");
/// ```
#[derive(Default)]
pub struct StaticVar;

impl<'a> RuleMut<'a> for StaticVar {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        let view = node.view();
        if view.kind() == "variable" {
            match view.text()?.to_lowercase().as_str() {
                "$shellid" => {
                    node.set(Raw(Str(String::from("Microsoft.Powershell"))))
                },
                "$?" => {
                    node.set(Bool(true))
                }
                _ => ()
            }
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
        // (program
        //  (statement_list inferred_type: None)
        //   (pipeline inferred_type: None
        //    (command inferred_type: None
        //     (command_name inferred_type: None)
        //     (command_elements inferred_type: None)
        //      (variable inferred_type: Some(Number(4)))))))))
        assert_eq!(*tree.root().unwrap()// program
            .child(0).unwrap() // statement_list
            .child(1).unwrap() // pipeline
            .child(0).unwrap() //command
            .child(1).unwrap() // command_elements
            .child(0).unwrap()// variable
            .data().expect("Expecting inferred type"), Raw(Num(4))
        );
    }

    #[test]
    fn test_unfollow_var_use_unknow_var() {
        let mut tree = from_powershell_src("$foo = $toto\nWrite-Debug $foo").unwrap();

        tree.apply_mut(&mut (ParseInt::default(), Forward::default(), Var::default())).unwrap();

        // We are waiting for
        // Write-Debug 4
        // (program
        //  (statement_list inferred_type: None)
        //   (pipeline inferred_type: None
        //    (command inferred_type: None
        //     (command_name inferred_type: None)
        //     (command_elements inferred_type: None)
        //      (variable inferred_type: Some(Number(4)))))))))
        assert_eq!(tree.root().unwrap()
            .child(0).unwrap()// statement_list
            .child(1).unwrap()// pipeline
            .child(0).unwrap()//command
            .child(1).unwrap()// command_elements
            .child(0).unwrap()// variable
            .data(), None
        );
    }

    #[test]
    fn test_static_var_shell_id() {
        let mut tree = from_powershell_src("$shellid").unwrap();

        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            StaticVar::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()// program
            .child(0).unwrap() // statement_list
            .child(0).unwrap() // pipeline
            .data().expect("Expecting inferred type"), Raw(Str("Microsoft.Powershell".to_string()))
        );
    }
}