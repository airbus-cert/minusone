use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::*;
use crate::js::Value::*;
use crate::js::functions::function::function_value_from_node;
use crate::js::globals::inject_js_globals;
use crate::js::r#loop::*;
use crate::js::utils::is_write_target;
use crate::rule::RuleMut;
use crate::scope::ScopeManager;
use crate::tree::{BranchFlow, ControlFlow, Node, NodeMut};
use log::{trace, warn};

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

    fn snapshot_scope(&self) -> String {
        let scope = self.scope_manager.current();
        let mut out = String::new();
        for name in scope.get_var_names() {
            if let Some(value) = scope.get_var(&name) {
                out.push_str(&format!("var {name} = {value};\n"));
            }
        }
        out
    }

    fn parse_array_index(node: &Node<JavaScript>) -> Option<usize> {
        if let Some(data) = node.data() {
            return match data {
                Raw(Num(n)) if n.is_finite() && n.fract() == 0.0 && *n >= 0.0 => Some(*n as usize),
                Raw(Str(s)) => s.parse::<usize>().ok(),
                _ => None,
            };
        }

        let text = node.text().ok()?;
        if text.chars().all(|c| c.is_ascii_digit()) {
            text.parse::<usize>().ok()
        } else {
            None
        }
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
                clear_for_loop_results();
            }
            // fn scopes: entering -> new scope
            "function_declaration"
            | "function"
            | "arrow_function"
            | "method_definition"
            | "generator_function_declaration"
            | "generator_function" => {
                self.scope_manager.enter();
                if let Some(body) = view.named_child("body") {
                    self.forget_assigned_var(&body)?;
                }
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
                        | "generator_function" => {}
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
            "for_statement" if is_for_loop_enabled() => {
                if for_depth_get() >= MAX_FOR_DEPTH {
                    return Ok(());
                }
                if let Some(body) = view.named_child("body") {
                    if body_has_bail_node(&body) {
                        return Ok(());
                    }
                }
                let Some((init_src, cond_src, update_src, body_src)) = extract_for_parts(&view)
                else {
                    return Ok(());
                };
                let scope_snapshot = self.snapshot_scope();
                for_depth_inc();
                let result = simulate_for_loop(
                    &scope_snapshot,
                    &init_src,
                    &cond_src,
                    &update_src,
                    &body_src,
                );
                for_depth_dec();
                if let Some(final_vars) = result {
                    trace!(
                        "ForLoop: simulated for_statement id={}, {} vars",
                        node.id(),
                        final_vars.len()
                    );
                    store_for_loop_result(node.id(), final_vars);
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
            // ForLoop stores final variable state in a thread-local side-channel
            "for_statement" => {
                if let Some(final_vars) = take_for_loop_result(node.id()) {
                    for (name, value) in final_vars {
                        trace!("ForLoop: assigning '{}' = {:?} after loop", name, value);
                        self.scope_manager.current_mut().assign(
                            &name,
                            value,
                            node.is_ongoing_transaction(),
                        );
                    }
                }
            }
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
                    } else if left.kind() == "subscript_expression" {
                        let Some(base_node) = left.named_child("object").or_else(|| left.child(0))
                        else {
                            return Ok(());
                        };
                        if base_node.kind() != "identifier" {
                            return Ok(());
                        }

                        let base_name = base_node.text()?.to_string();
                        let index_node = left.named_child("index").or_else(|| left.child(2));
                        let index = index_node.and_then(|node| Self::parse_array_index(&node));
                        let rhs_data = right.data().cloned().or_else(|| {
                            if right.kind() == "identifier" {
                                right.text().ok().and_then(|name| {
                                    self.scope_manager.current().get_var(name).cloned()
                                })
                            } else {
                                None
                            }
                        });

                        match (index, rhs_data) {
                            (Some(index), Some(value)) => {
                                if let Some(Array(arr)) =
                                    self.scope_manager.current_mut().get_var_mut(&base_name)
                                {
                                    if index >= arr.len() {
                                        arr.resize(index + 1, Undefined);
                                    }
                                    arr[index] = value;
                                    trace!(
                                        "Var (L): Updating array '{}' index {}",
                                        base_name, index
                                    );
                                } else {
                                    self.scope_manager
                                        .current_mut()
                                        .forget(&base_name, node.is_ongoing_transaction());
                                }
                            }
                            _ => {
                                self.scope_manager
                                    .current_mut()
                                    .forget(&base_name, node.is_ongoing_transaction());
                            }
                        }
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
                        if matches!(data, Object { .. }) {
                            return Ok(());
                        }

                        if matches!(data, Array(_)) {
                            if let Some(member_expr) = view.parent()
                                && member_expr.kind() == "member_expression"
                                && member_expr
                                    .named_child("object")
                                    .map(|o| {
                                        o.start_abs() == view.start_abs()
                                            && o.end_abs() == view.end_abs()
                                    })
                                    .unwrap_or(false)
                                && let Some(call_expr) = member_expr.parent()
                                && call_expr.kind() == "call_expression"
                                && let Some(prop) = member_expr.named_child("property")
                                && matches!(
                                    prop.text().unwrap_or(""),
                                    "pop"
                                        | "push"
                                        | "shift"
                                        | "unshift"
                                        | "splice"
                                        | "sort"
                                        | "reverse"
                                        | "fill"
                                        | "copyWithin"
                                )
                            {
                                if matches!(
                                    prop.text().unwrap_or(""),
                                    "pop" | "shift" | "splice" | "sort" | "reverse" | "copyWithin"
                                ) {
                                    if let Array(arr) = data {
                                        // fns that doesn't add elements in an empty array keep it as is
                                        if arr.is_empty() {
                                            return Ok(());
                                        }
                                    }
                                }
                                trace!(
                                    "Var (L): Propagating array '{}' before mutating call .{}(), then forgetting",
                                    var_name,
                                    prop.text().unwrap_or("?")
                                );
                                warn!(
                                    "Dropped {} because a mutable call `{}.{}(...)` is occurring on it. This means that the deobfuscation will be less effective",
                                    var_name,
                                    var_name,
                                    prop.text().unwrap_or("?")
                                );
                                node.set(data.clone());
                                self.scope_manager
                                    .current_mut()
                                    .forget(&var_name, node.is_ongoing_transaction());
                                return Ok(());
                            }
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
