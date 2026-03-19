use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::rule::RuleMut;
use crate::scope::ScopeManager;
use crate::tree::{BranchFlow, ControlFlow, Node, NodeMut};
use log::trace;

/// Var is a variable manager that will try to track
/// static variable assignments and propagate them in the code
/// when possible. It respects JavaScript rules:
///
/// |           | *global stored* | *fn scopped* | *bloc scopped* | *mutable* | *redeclarable* |
/// |-----------|:---------------:|:------------:|:--------------:|:---------:|:--------------:|
/// | **var**   | yes             | yes          | no             | yes       | yes            |
/// | **let**   | no              | yes          | yes            | yes       | no             |
/// | **const** | no              | yes          | yes            | no        | no             |
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::forward::Forward;
/// use minusone::js::integer::ParseInt;
/// use minusone::js::string::ParseString;
/// use minusone::js::var::Var;
/// use minusone::js::linter::Linter;
/// use minusone::js::strategy::JavaScriptStrategy;
///
/// let mut tree = build_javascript_tree("var a = 'hello'; console.log(a);").unwrap();
/// tree.apply_mut_with_strategy(
///     &mut (ParseString::default(), Forward::default(), Var::default()),
///     JavaScriptStrategy::default(),
/// ).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "var a = 'hello'; console.log('hello');");
/// ```
pub struct Var {
    scope_manager: ScopeManager<JavaScript>,
}

impl Default for Var {
    fn default() -> Self {
        Var {
            scope_manager: ScopeManager::default(),
        }
    }
}

impl Var {
    fn forget_assigned_var<T>(&mut self, node: &Node<T>) -> MinusOneResult<()> {
        for child in node.iter() {
            match child.kind() {
                "identifier" => {
                    // if identifier is on the left side of an assignment
                    if child
                        .get_parent_of_types(vec![
                            "assignment_expression",
                            "augmented_assignment_expression",
                            "update_expression",
                        ])
                        .is_some()
                    {
                        let var_name = child.text()?.to_string();
                        self.scope_manager
                            .current_mut()
                            .forget(&var_name, node.is_ongoing_transaction());
                    }
                }
                "variable_declarator" => {
                    // declaration in an unpredictable branch: forget the name
                    if let Some(name_node) = child.named_child("name") {
                        if name_node.kind() == "identifier" {
                            let var_name = name_node.text()?.to_string();
                            self.scope_manager
                                .current_mut()
                                .forget(&var_name, node.is_ongoing_transaction());
                        }
                    }
                }
                _ => {
                    self.forget_assigned_var(&child)?;
                }
            }
        }
        Ok(())
    }

    fn is_write_target(node: &Node<JavaScript>) -> bool {
        let mut current = node.parent();
        while let Some(parent) = current {
            match parent.kind() {
                "variable_declarator" => {
                    if let Some(name_child) = parent.child(0) {
                        return node.start_abs() >= name_child.start_abs()
                            && node.end_abs() <= name_child.end_abs();
                    }
                }
                "assignment_expression" | "augmented_assignment_expression" => {
                    if let Some(left) = parent.child(0) {
                        return node.start_abs() >= left.start_abs()
                            && node.end_abs() <= left.end_abs();
                    }
                }
                "update_expression" => {
                    return true;
                }
                _ => {}
            }

            current = parent.parent();
        }

        false
    }
}

impl<'a> RuleMut<'a> for Var {
    type Language = JavaScript;

    fn enter(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        flow: ControlFlow,
    ) -> MinusOneResult<()> {
        let view = node.view();
        match view.kind() {
            "program" => {
                self.scope_manager.reset();
            }
            // fn scopes: entering -> new scope
            "function_declaration"
            | "function"
            | "arrow_function"
            | "method_definition"
            | "generator_function_declaration"
            | "generator_function" => {
                self.scope_manager.enter();
            }
            // bloc scopes but NOT when parent is a function, because we already pushed a scope for the function itself
            "statement_block" => {
                if let Some(parent) = view.parent() {
                    match parent.kind() {
                        "function_declaration"
                        | "function"
                        | "arrow_function"
                        | "method_definition"
                        | "generator_function_declaration"
                        | "generator_function" => {
                            if flow == ControlFlow::Continue(BranchFlow::Unpredictable) {
                                self.forget_assigned_var(&view)?;
                            }
                        }
                        _ => {
                            // Non-function block: push a block scope
                            self.scope_manager.enter();
                            if flow == ControlFlow::Continue(BranchFlow::Unpredictable) {
                                self.forget_assigned_var(&view)?;
                            }
                        }
                    }
                } else {
                    self.scope_manager.enter();
                    if flow == ControlFlow::Continue(BranchFlow::Unpredictable) {
                        self.forget_assigned_var(&view)?;
                    }
                }
            }
            "}" => {
                if let Some(parent) = view.parent() {
                    match parent.kind() {
                        "statement_block" => {
                            if let Some(grandparent) = parent.parent() {
                                match grandparent.kind() {
                                    "function_declaration"
                                    | "function"
                                    | "arrow_function"
                                    | "method_definition"
                                    | "generator_function_declaration"
                                    | "generator_function" => {
                                        self.scope_manager.leave_function();
                                    }
                                    _ => {
                                        self.scope_manager.leave();
                                    }
                                }
                            } else {
                                self.scope_manager.leave();
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn leave(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        let view = node.view();
        match view.kind() {
            // var/let/const
            "variable_declarator" => {
                // child(0) = name (identifier), child(1) = "=", child(2) = value
                if let Some(name_node) = view.named_child("name") {
                    if name_node.kind() == "identifier" {
                        let var_name = name_node.text()?.to_string();
                        if let Some(value_node) = view.named_child("value") {
                            if let Some(data) = value_node.data() {
                                trace!("Var (L): Assigning variable '{}' = {:?}", var_name, data);
                                self.scope_manager.current_mut().assign(
                                    &var_name,
                                    data.clone(),
                                    node.is_ongoing_transaction(),
                                );
                            }
                        }
                        // variable_declaration = var, lexical_declaration = let/const
                        if let Some(parent) = view.parent() {
                            if parent.kind() == "variable_declaration" {
                                self.scope_manager.current_mut().set_non_local(&var_name);
                            }
                        }
                    }
                }
            }
            // reassignment
            "assignment_expression" => {
                if let (Some(left), Some(right)) = (view.child(0), view.child(2)) {
                    if left.kind() == "identifier" {
                        let var_name = left.text()?.to_string();
                        if let Some(data) = right.data() {
                            trace!("Var (L): Re-assigning variable '{}' = {:?}", var_name, data);
                            self.scope_manager.current_mut().assign(
                                &var_name,
                                data.clone(),
                                node.is_ongoing_transaction(),
                            );
                        } else {
                            // unknown, forget the variable
                            self.scope_manager
                                .current_mut()
                                .forget(&var_name, node.is_ongoing_transaction());
                        }
                    }
                }
            }
            // x++, x--, ++x, --x
            "update_expression" => {
                for i in 0..view.child_count() {
                    if let Some(child) = view.child(i) {
                        if child.kind() == "identifier" {
                            let var_name = child.text()?.to_string();
                            self.scope_manager
                                .current_mut()
                                .forget(&var_name, node.is_ongoing_transaction());
                            break;
                        }
                    }
                }
            }
            // read
            "identifier" => {
                if !Var::is_write_target(&view) {
                    if matches!(view.data(), Some(JavaScript::Object(_))) {
                        return Ok(());
                    }

                    let var_name = view.text()?.to_string();
                    if let Some(data) = self.scope_manager.current().get_var(&var_name) {
                        trace!("Var (L): Propagating variable '{}' = {:?}", var_name, data);
                        node.set(data.clone());
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::js::build_javascript_tree;
    use crate::js::forward::Forward;
    use crate::js::integer::ParseInt;
    use crate::js::linter::Linter;
    use crate::js::strategy::JavaScriptStrategy;
    use crate::js::string::ParseString;
    use crate::js::var::Var;

    fn deobfuscate(input: &str) -> String {
        // todo: same method on other js tests with logic scopes
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                ParseString::default(),
                Forward::default(),
                Var::default(),
            ),
            JavaScriptStrategy::default(),
        )
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_var_simple_string() {
        assert_eq!(
            deobfuscate("var a = 'hello'; console.log(a);"),
            "var a = 'hello'; console.log('hello');"
        );
    }

    #[test]
    fn test_var_simple_int() {
        assert_eq!(
            deobfuscate("var x = 42; console.log(x);"),
            "var x = 42; console.log(42);"
        );
    }

    #[test]
    fn test_let_simple() {
        assert_eq!(
            deobfuscate("let a = 'world'; console.log(a);"),
            "let a = 'world'; console.log('world');"
        );
    }

    #[test]
    fn test_const_simple() {
        assert_eq!(
            deobfuscate("const a = 'test'; console.log(a);"),
            "const a = 'test'; console.log('test');"
        );
    }

    #[test]
    fn test_var_function_scope() {
        assert_eq!(
            deobfuscate("function test() { var a = 'hello'; console.log(a); } console.log(a);"),
            "function test() { var a = 'hello'; console.log('hello'); } console.log(a);"
        );
    }

    #[test]
    fn test_var_reassignment() {
        assert_eq!(
            deobfuscate("var a = 'hello'; a = 'world'; console.log(a);"),
            "var a = 'hello'; a = 'world'; console.log('world');"
        );
    }

    #[test]
    fn test_var_unknown_reassignment() {
        assert_eq!(
            deobfuscate("var a = 'hello'; a = foo(); console.log(a);"),
            "var a = 'hello'; a = foo(); console.log(a);"
        );
    }

    #[test]
    fn test_multiple_vars() {
        assert_eq!(
            deobfuscate("var a = 'hello'; var b = 'world'; console.log(a, b);"),
            "var a = 'hello'; var b = 'world'; console.log('hello', 'world');"
        );
    }

    #[test]
    fn test_var_in_nested_block() {
        assert_eq!(
            deobfuscate("var x = 10; { console.log(x); }"),
            "var x = 10; { console.log(10); }"
        );
    }

    #[test]
    fn test_let_block_scope() {
        assert_eq!(
            deobfuscate("{ let x = 10; console.log(x); } console.log(x);"),
            "{ let x = 10; console.log(10); } console.log(x);"
        );
    }

    #[test]
    fn test_var_hoists_out_of_block() {
        assert_eq!(
            deobfuscate("{ var x = 10; } console.log(x);"),
            "{ var x = 10; } console.log(10);"
        );
    }

    #[test]
    fn test_let_does_not_hoist_out_of_block() {
        assert_eq!(
            deobfuscate("{ let x = 10; } console.log(x);"),
            "{ let x = 10; } console.log(x);"
        );
    }

    #[test]
    fn test_const_does_not_hoist_out_of_block() {
        assert_eq!(
            deobfuscate("{ const x = 10; } console.log(x);"),
            "{ const x = 10; } console.log(x);"
        );
    }
}
