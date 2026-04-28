use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::*;
use crate::js::Value::*;
use crate::js::functions::function::function_value_from_node;
use crate::js::globals::inject_js_globals;
use crate::js::utils::is_write_target;
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
#[derive(Default)]
pub struct Var {
    scope_manager: ScopeManager<JavaScript>,
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
                    if let Some(name_node) = child.named_child("name")
                        && name_node.kind() == "identifier"
                    {
                        let var_name = name_node.text()?.to_string();
                        self.scope_manager
                            .current_mut()
                            .forget(&var_name, node.is_ongoing_transaction());
                    }
                }
                _ => {
                    self.forget_assigned_var(&child)?;
                }
            }
        }
        Ok(())
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
                inject_js_globals(self.scope_manager.current_mut(), false);
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
                if let Some(parent) = view.parent()
                    && parent.kind() == "statement_block"
                {
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
                if let Some(name_node) = view.named_child("name")
                    && name_node.kind() == "identifier"
                {
                    let var_name = name_node.text()?.to_string();
                    if let Some(value_node) = view.named_child("value")
                        && let Some(data) = value_node.data()
                    {
                        trace!("Var (L): Assigning variable '{}' = {:?}", var_name, data);
                        self.scope_manager.current_mut().assign(
                            &var_name,
                            data.clone(),
                            node.is_ongoing_transaction(),
                        );
                    }
                    // variable_declaration = var, lexical_declaration = let/const
                    if let Some(parent) = view.parent()
                        && parent.kind() == "variable_declaration"
                    {
                        self.scope_manager.current_mut().set_non_local(&var_name);
                    }
                }
            }
            // reassignment
            "assignment_expression" => {
                if let (Some(left), Some(right)) = (view.child(0), view.child(2))
                    && left.kind() == "identifier"
                {
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
            // function test() {} / function* test() {}
            "function_declaration" | "generator_function_declaration" => {
                if let Some(name_node) = view.named_child("name")
                    && name_node.kind() == "identifier"
                {
                    let var_name = name_node.text()?.to_string();
                    let value = view
                        .data()
                        .cloned()
                        .or_else(|| function_value_from_node(&view));

                    let Some(value) = value else {
                        return Ok(());
                    };

                    trace!(
                        "Var (L): Assigning function declaration '{}' = {:?}",
                        var_name, value
                    );
                    self.scope_manager.current_mut().assign(
                        &var_name,
                        value,
                        node.is_ongoing_transaction(),
                    );

                    if let Some(parent) = view.parent()
                        && parent.kind() == "program"
                    {
                        self.scope_manager.current_mut().set_non_local(&var_name);
                    }
                }
            }
            // x++, x--, ++x, --x
            "update_expression" => {
                // keep for loop conservative
                if let Some(parent) = view.parent()
                    && parent.kind() == "for_statement"
                    && let Some(increment) = parent.named_child("increment")
                    && view.start_abs() >= increment.start_abs()
                    && view.end_abs() <= increment.end_abs()
                {
                    for i in 0..view.child_count() {
                        if let Some(child) = view.child(i)
                            && child.kind() == "identifier"
                        {
                            let var_name = child.text()?.to_string();
                            self.scope_manager
                                .current_mut()
                                .forget(&var_name, node.is_ongoing_transaction());
                            break;
                        }
                    }
                    return Ok(());
                }

                let (is_increment, is_prefix, var_name) = {
                    let (Some(first), Some(second)) = (view.child(0), view.child(1)) else {
                        return Ok(());
                    };

                    let first_text = first.text()?;
                    if first_text == "++" || first_text == "--" {
                        if second.kind() != "identifier" {
                            return Ok(());
                        }
                        (first_text == "++", true, second.text()?.to_string())
                    } else {
                        if second.text()? != "++" && second.text()? != "--" {
                            return Ok(());
                        }
                        if first.kind() != "identifier" {
                            return Ok(());
                        }
                        (second.text()? == "++", false, first.text()?.to_string())
                    }
                };

                if let Some(current_value) =
                    self.scope_manager.current().get_var(&var_name).cloned()
                {
                    match current_value.as_js_num() {
                        Raw(Num(n)) => {
                            let updated = if is_increment { n + 1.0 } else { n - 1.0 };
                            self.scope_manager.current_mut().assign(
                                &var_name,
                                Raw(Num(updated)),
                                node.is_ongoing_transaction(),
                            );
                            node.reduce(Raw(Num(if is_prefix { updated } else { n })));
                            trace!(
                                "Var (L): update '{}' from {} to {} (prefix={})",
                                var_name, n, updated, is_prefix
                            );
                        }
                        NaN => {
                            self.scope_manager.current_mut().assign(
                                &var_name,
                                NaN,
                                node.is_ongoing_transaction(),
                            );
                            node.reduce(NaN);
                        }
                        _ => unreachable!(
                            "The result of as_js_num should be either Raw(Num(x)) or NaN"
                        ),
                    }
                } else {
                    self.scope_manager
                        .current_mut()
                        .forget(&var_name, node.is_ongoing_transaction());
                }
            }
            // read
            "identifier" => {
                if !is_write_target(&view) {
                    if let Some(parent) = view.parent()
                        && parent.kind() == "call_expression"
                        && parent
                            .named_child("function")
                            .map(|f| {
                                f.start_abs() == view.start_abs() && f.end_abs() == view.end_abs()
                            })
                            .unwrap_or(false)
                    {
                        return Ok(());
                    }

                    if matches!(view.data(), Some(Object { .. })) {
                        return Ok(());
                    }

                    let var_name = view.text()?.to_string();
                    if let Some(data) = self.scope_manager.current().get_var(&var_name) {
                        if matches!(data, JavaScript::Object { .. }) {
                            return Ok(());
                        }

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
