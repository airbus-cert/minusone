use crate::error::{Error, MinusOneResult};
use crate::ps::Powershell::{self, Array, Crypto, Null, Raw, Type};
use crate::ps::Value::{self, Bool, Num, Str};
use crate::ps::crypto::assign_aes_property;
use crate::ps::tool::StringTool;
use crate::regex::Regex;
use crate::rule::{Rule, RuleMut};
use crate::scope::ScopeManager;
use crate::tree::{BranchFlow, ControlFlow, Node, NodeMut};
use log::trace;
use std::collections::{BTreeMap, HashMap};
use std::ops::Add;

/// Var is a variable manager that will try to track
/// static var assignement and propagte it in the code
/// when it's possible
///
/// # Example
/// ```
/// use minusone::tree::{HashMapStorage, Tree};
/// use minusone::ps::build_powershell_tree;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::integer::ParseInt;
/// use minusone::ps::var::Var;
/// use minusone::ps::linter::Linter;
/// use minusone::ps::strategy::PowershellStrategy;
///
/// let mut tree = build_powershell_tree("\
/// $foo = 4
/// Write-Debug $foo\
/// ").unwrap();
/// tree.apply_mut_with_strategy(&mut (ParseInt::default(), Forward::default(), Var::default()), PowershellStrategy::default()).unwrap();
///
/// let mut ps_litter_view = Linter::default();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\
/// $foo = 4
/// Write-Debug 4\
/// ");
/// ```
pub struct Var {
    scope_manager: ScopeManager<Powershell>,
}

impl Var {
    fn reset_scope_manager(&mut self) {
        self.scope_manager.reset();
        vec![
            "args",
            "ConfirmPreference",
            "ConsoleFileName",
            "DebugPreference",
            "Error",
            "ErrorActionPreference",
            "ErrorView",
            "ExecutionContext",
            "FormatEnumerationLimit",
            "HOME",
            "Host",
            "InformationPreference",
            "input",
            "MaximumAliasCount",
            "MaximumDriveCount",
            "MaximumErrorCount",
            "MaximumFunctionCount",
            "MaximumHistoryCount",
            "MaximumVariableCount",
            "MyInvocation",
            "NestedPromptLevel",
            "null",
            "OutputEncoding",
            "PID",
            "PROFILE",
            "ProgressPreference",
            "PSBoundParameters",
            "PSCommandPath",
            "PSCulture",
            "PSDefaultParameterValues",
            "PSEdition",
            "PSEmailServer",
            "PSHOME",
            "PSScriptRoot",
            "PSSessionApplicationName",
            "PSSessionConfigurationName",
            "PSSessionOption",
            "PSUICulture",
            "PSVersionTable",
            "PWD",
            "ShellId",
            "StackTrace",
            "VerbosePreference",
            "WarningPreference",
            "WhatIfPreference",
        ]
        .iter()
        .for_each(|s| {
            self.scope_manager
                .current_mut()
                .assign(s, Powershell::Unknown, false)
        });
    }
    fn forget_assigned_var<T>(
        &mut self,
        node: &Node<T>,
        is_ongoing_transaction: bool,
    ) -> MinusOneResult<()> {
        for child in node.iter() {
            if child.kind() == "variable" {
                if child
                    .get_parent_of_types(vec![
                        "left_assignment_expression",
                        "pre_increment_expression",
                        "pre_decrement_expression",
                        "post_increment_expression",
                        "post_decrement_expression",
                    ])
                    .is_some()
                    && let Some(var_name) = Var::extract(child.text()?)
                {
                    self.scope_manager
                        .current_mut()
                        .forget(&var_name, is_ongoing_transaction);
                }
            } else {
                self.forget_assigned_var(&child, is_ongoing_transaction)?;
            }
        }

        Ok(())
    }

    /// Extract variable name from a variable declaration \
    /// If a provider is set, match only `variable`
    ///
    /// # Example
    ///
    /// $a => a \
    /// ${var-1} => var-1
    ///
    pub fn extract(var: &str) -> Option<String> {
        let var = var.to_lowercase();
        let re_simple =
            Regex::new(r"\$(?<provider>([a-zA-Z]+):)?(?<name>[a-zA-Z0-9_?:]+)").unwrap();
        let re_braced =
            Regex::new(r"\$\{(?<provider>([a-zA-Z]+):)?(?<name>([^`\}]|`.)+)\}").unwrap();

        if let Some(cap) = re_simple.captures(&var).or(re_braced.captures(&var))
            && let Some(name) = cap.name("name")
        {
            if let Some(provider) = cap.name("provider")
                && provider.as_str() != "variable"
            {
                return None;
            }
            return Some(name.as_str().to_string());
        }

        None
    }

    /// Resolve the name of a variable pattern given the current scope
    ///
    /// Use for patterns used by variable, get-variable, set-variable, get-childitem...
    fn resolve_wildcarded(&self, variable_name: String) -> Option<String> {
        if variable_name.contains("*") {
            let re = Regex::new(&format!("^{}$", variable_name.replace("*", ".*"))).unwrap();
            let current_scope = self.scope_manager.current();
            let var_names = current_scope.get_var_names();
            let matches: Vec<_> = var_names
                .iter()
                .filter(|&var_name| re.is_match(var_name))
                .collect();

            if matches.len() == 1 {
                Some(matches[0].clone())
            } else {
                None
            }
        } else {
            Some(variable_name)
        }
    }

    fn hashmap(variable_name: String, data: &Value) -> Powershell {
        Powershell::HashMap(BTreeMap::from([
            (Str("name".to_string()), Str(variable_name)),
            (Str("value".to_string()), data.clone()),
        ]))
    }
}

impl Default for Var {
    fn default() -> Self {
        let mut new = Var {
            scope_manager: ScopeManager::default(),
        };
        new.reset_scope_manager();
        new
    }
}

pub fn find_variable_node<'a, T>(node: &Node<'a, T>) -> Option<Node<'a, T>> {
    for child in node.iter() {
        if child.kind() == "variable" {
            if let Some(parent) = child.parent()
                && parent.kind() == "unary_expression"
            {
                return Some(child);
            }
        } else if let Some(new_node) = find_variable_node(&child) {
            return Some(new_node);
        }
    }
    None
}

fn find_member_assignment<'a, T>(node: &Node<'a, T>) -> Option<(Node<'a, T>, Node<'a, T>)> {
    for child in node.iter() {
        if child.kind() == "member_access"
            && let (Some(obj), Some(member_name)) = (child.child(0), child.child(2))
            && obj.kind() == "variable"
        {
            return Some((obj, member_name));
        } else if let Some(found) = find_member_assignment(&child) {
            return Some(found);
        }
    }
    None
}

impl<'a> RuleMut<'a> for Var {
    type Language = Powershell;

    fn enter(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        flow: ControlFlow,
    ) -> MinusOneResult<()> {
        if !node.is_ongoing_transaction() {
            self.scope_manager.flush_transaction();
        }

        let view = node.view();
        match view.kind() {
            "program" => self.reset_scope_manager(),
            "function_statement" => self.scope_manager.enter(),
            "}" => {
                if let Some(parent) = view.parent()
                    && (parent.kind() == "statement_block" || parent.kind() == "function_statement")
                {
                    self.scope_manager.leave();
                }
            }

            // Each time I start an unpredictable branch I forget all assigned var in this block
            "statement_block" => {
                // record var block during new statement blocks
                self.scope_manager.enter();
                if flow == ControlFlow::Continue(BranchFlow::Unpredictable) {
                    self.forget_assigned_var(&view, node.is_ongoing_transaction())?;
                }
            }

            // in the enter function because pre increment before assigned
            "pre_increment_expression" | "pre_decrement_expression" => {
                if let Some(variable) = view.child(1).ok_or(Error::invalid_child())?.child(0)
                    && let Some(var_name) = Var::extract(variable.text()?)
                {
                    if let Some(Raw(Num(v))) =
                        self.scope_manager.current_mut().get_var_mut(&var_name)
                    {
                        if view.kind() == "pre_increment_expression" {
                            *v += 1;
                        } else {
                            *v -= 1;
                        }
                    } else {
                        self.scope_manager
                            .current_mut()
                            .forget(&var_name, node.is_ongoing_transaction())
                    }
                }
            }
            _ => (),
        }
        Ok(())
    }

    fn leave(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        flow: ControlFlow,
    ) -> MinusOneResult<()> {
        if !node.is_ongoing_transaction() {
            self.scope_manager.flush_transaction();
        }

        let view = node.view();
        match view.kind() {
            "assignment_expression" => {
                // Assign var value if it's possible
                if let (Some(left), Some(operator), Some(right)) =
                    (view.child(0), view.child(1), view.child(2))
                {
                    if let Some(var) = find_variable_node(&left)
                        && let Some(var_name) = Var::extract(var.text()?)
                    {
                        let scope = self.scope_manager.current_mut();
                        if let (current_value, Some(new_value)) =
                            (scope.get_var(&var_name), right.data())
                        {
                            // only predictable assignment is handled of local var
                            let is_local = scope.is_local(&var_name).unwrap_or(true);
                            if flow == ControlFlow::Continue(BranchFlow::Predictable) || is_local {
                                match assign_handler(current_value, operator, new_value) {
                                    Some(assign_value) => scope.assign(
                                        &var_name,
                                        assign_value,
                                        node.is_ongoing_transaction(),
                                    ),
                                    _ => scope.forget(&var_name, node.is_ongoing_transaction()),
                                }
                            }
                        }
                    } else if operator.text()? == "="
                        && let Some((obj, member_name)) = find_member_assignment(&left)
                        && let Some(var_name) = Var::extract(obj.text()?)
                        && let Some(new_value) = right.data()
                    {
                        // Property assignment on a tracked object, e.g. $aes.Key = $keyBytes
                        let member = member_name.text()?.to_string().normalize();
                        let scope = self.scope_manager.current_mut();
                        let is_local = scope.is_local(&var_name).unwrap_or(true);
                        if flow == ControlFlow::Continue(BranchFlow::Predictable) || is_local {
                            if let Some(Crypto(state)) = scope.get_var_mut(&var_name)
                                && !assign_aes_property(state, &member, new_value)
                            {
                                scope.forget(&var_name, node.is_ongoing_transaction());
                            }
                        }
                    }
                }
            }
            "variable" => {
                if let Some(var_name) = Var::extract(view.text()?) {
                    // forget variable with [ref] operator
                    if let Some(cast_expression) = view.get_parent_of_types(vec!["cast_expression"])
                        && let Some(Type(typename)) = cast_expression.child(0).unwrap().data()
                        && typename.to_lowercase() == "ref"
                    {
                        self.scope_manager
                            .current_mut()
                            .forget(&var_name, node.is_ongoing_transaction())
                    }

                    // check if we are not on the left part of an assignment expression
                    // already handle by the previous case
                    if view
                        .get_parent_of_types(vec!["left_assignment_expression"])
                        .is_none()
                    {
                        // Try to assign variable member
                        if let Some(data) = self.scope_manager.current_mut().get_var(&var_name) {
                            trace!("Var (L): Setting node with variable value: {:?}", data);
                            node.set(data.clone());
                        } else {
                            self.scope_manager
                                .current_mut()
                                .in_use(&var_name, node.is_ongoing_transaction());
                        }
                    }
                }
            }
            // pre_increment_expression is safe to forward due to the enter function handler
            "pre_increment_expression" | "pre_decrement_expression" => {
                if let Some(expression) = view.child(1)
                    && let Some(expression_data) = expression.data()
                {
                    trace!(
                        "Var (L): Setting node with pre-increment/decrement value: {:?}",
                        expression_data
                    );
                    node.set(expression_data.clone())
                }
            }
            // in the enter function because pre increment before assigned
            "post_increment_expression" | "post_decrement_expression" => {
                if let Some(variable) = view.child(0)
                    && let Some(var_name) = Var::extract(variable.text()?)
                {
                    let kind = view.kind();

                    if let Some(Raw(Num(v))) =
                        self.scope_manager.current_mut().get_var_mut(&var_name)
                    {
                        // we set the variable before ...
                        if let Some(variable_data) = variable.data() {
                            trace!(
                                "Var (L): Setting node with post-increment/decrement value: {:?}",
                                variable_data
                            );
                            node.set(variable_data.clone())
                        }
                        // ... assign it
                        if kind == "post_increment_expression" {
                            *v += 1;
                        } else {
                            *v -= 1;
                        }
                    } else {
                        self.scope_manager
                            .current_mut()
                            .forget(&var_name, node.is_ongoing_transaction())
                    }
                }
            }
            // Some function change the value of variables
            // [array]::reverse is handled
            "invokation_expression" => {
                if let (Some(type_lit), Some(op), Some(member_name), Some(args_list)) =
                    (view.child(0), view.child(1), view.child(2), view.child(3))
                {
                    match (
                        type_lit.data(),
                        op.text()?,
                        member_name.text()?.to_lowercase().as_str(),
                    ) {
                        (Some(Type(typename)), "::", m)
                            if (typename == "array" && m.to_lowercase() == "reverse") =>
                        {
                            // get the argument list if present
                            if let Some(argument_expression_list) =
                                args_list.named_child("argument_expression_list")
                                && let Some(arg_1) = argument_expression_list.child(0)
                                && let Some(var_name) = Var::extract(arg_1.text()?)
                                && let Some(Array(data)) =
                                    self.scope_manager.current_mut().get_var_mut(&var_name)
                            {
                                data.reverse();
                            }
                        }
                        _ => {
                            // Any array passed as param is forgotten
                            if let Some(argument_expression_list) =
                                args_list.named_child("argument_expression_list")
                            {
                                for arg in argument_expression_list.iter() {
                                    if let Some(var_name) = Var::extract(arg.text()?)
                                        && let Some(Array(_)) =
                                            self.scope_manager.current_mut().get_var(&var_name)
                                    {
                                        self.scope_manager
                                            .current_mut()
                                            .forget(&var_name, node.is_ongoing_transaction());
                                    }
                                }
                            }
                        }
                    }
                }
            }
            "command" => {
                if let Some(command_name) = view.child(0) {
                    match crate::ps::cmdlets::resolved_command_name(&command_name)?.as_str() {
                        "variable" => {
                            if let Some(command_elements) = view.child(1)
                                && let Some(variable_name) = command_elements.child(1)
                                && let Some(variable_name) =
                                    self.resolve_wildcarded(variable_name.text()?.to_lowercase())
                            {
                                if let Some(Raw(data)) =
                                    self.scope_manager.current().get_var(&variable_name)
                                {
                                    trace!(
                                        "Var (L): Setting node with variable hashmap: {:?}",
                                        Var::hashmap(variable_name.clone(), data)
                                    );
                                    node.set(Var::hashmap(variable_name, data));
                                } else {
                                    self.scope_manager
                                        .current_mut()
                                        .in_use(&variable_name, node.is_ongoing_transaction());
                                }
                            }
                        }
                        "get-variable" | "gv" => {
                            if let Some(command_elements) = view.child(1)
                                && let Some(variable) = command_elements.child(1)
                                && let Some(variable_name) =
                                    self.resolve_wildcarded(variable.text()?.to_lowercase())
                                && let Some(Raw(data)) =
                                    self.scope_manager.current().get_var(&variable_name)
                            {
                                let value_param =
                                    command_elements.child(3).is_some_and(|command_parameter| {
                                        command_parameter.kind() == "command_parameter"
                                            && command_parameter.text().is_ok_and(|text| {
                                                "-valueonly".starts_with(&text.to_lowercase())
                                            })
                                    });

                                if value_param {
                                    trace!(
                                        "Var (L): Setting node with raw variable value: {:?}",
                                        data
                                    );
                                    node.set(Raw(data.clone()));
                                } else {
                                    trace!(
                                        "Var (L): Setting node with variable hashmap: {:?}",
                                        Var::hashmap(variable_name.clone(), data)
                                    );
                                    node.set(Var::hashmap(variable_name, data));
                                }
                            }
                        }
                        "set-variable" | "sv" => {
                            if let Some(command_elements) = view.child(1)
                                && let (Some(variable_name_node), Some(variable_value_node)) =
                                    (command_elements.child(1), command_elements.child(3))
                                && let Some(Raw(variable_value)) = variable_value_node.data()
                                && let Some(variable_name) =
                                    if let Some(Raw(variable_name)) = variable_name_node.data() {
                                        Some(variable_name.to_string())
                                    } else if variable_name_node.kind() == "generic_token" {
                                        Some(variable_name_node.text()?.to_lowercase())
                                    } else {
                                        None
                                    }
                                && let Some(variable_name) = self.resolve_wildcarded(variable_name)
                            {
                                self.scope_manager.current_mut().assign(
                                    &variable_name,
                                    Powershell::Raw(variable_value.clone()),
                                    node.is_ongoing_transaction(),
                                );
                            }
                        }
                        "get-childitem" | "gci" | "ls" => {
                            if let Some(command_elements) = view.child(1)
                                && let Some(item_name_node) = command_elements.child(1)
                            {
                                let item_name = item_name_node.text()?.to_lowercase();
                                let re = Regex::new(r"^variable:\/?(.*)$").unwrap();
                                if let Some(variable_name) =
                                    re.captures(&item_name).and_then(|cap| cap.get(1))
                                    && let Some(variable_name) =
                                        self.resolve_wildcarded(variable_name.as_str().to_string())
                                    && let Some(Raw(data)) =
                                        self.scope_manager.current().get_var(&variable_name)
                                {
                                    trace!(
                                        "Var (L): Setting node with variable hashmap from get-childitem: {:?}",
                                        Var::hashmap(variable_name.clone(), data)
                                    );
                                    node.set(Var::hashmap(variable_name, data));
                                }
                            }
                        }
                        "set-item" | "si" => {
                            if let Some(command_elements) = view.child(1)
                                && let (Some(item_name_node), Some(item_value_node)) =
                                    (command_elements.child(1), command_elements.child(3))
                                && let Some(Raw(item_value)) = item_value_node.data()
                            {
                                let item_name = item_name_node.text()?.to_lowercase();
                                let re = Regex::new(r"^variable:\/?(.*)$").unwrap();
                                if let Some(variable_name) =
                                    re.captures(&item_name).and_then(|cap| cap.get(1))
                                    && let Some(variable_name) =
                                        self.resolve_wildcarded(variable_name.as_str().to_string())
                                {
                                    self.scope_manager.current_mut().assign(
                                        &variable_name,
                                        Powershell::Raw(item_value.clone()),
                                        node.is_ongoing_transaction(),
                                    );
                                }
                            }
                        }
                        _ => (),
                    }
                }
            }
            _ => (),
        }
        Ok(())
    }
}

fn assign_handler(
    current_value: Option<&Powershell>,
    operator: Node<'_, Powershell>,
    add_new: &Powershell,
) -> Option<Powershell> {
    match (current_value, operator.text().ok()?, add_new) {
        // Simple assignment that will erase previous data
        (_, "=", d) => Some(d.clone()),

        // += operator
        (Some(Raw(Num(v))), "+=", Raw(Num(n))) => Some(Raw(Num(v + n))),
        (Some(Raw(Num(v))), "+=", Raw(Str(n))) => n.parse::<i64>().ok().map(|n| Raw(Num(v + n))),
        (Some(Raw(Str(v))), "+=", Raw(Num(n))) => Some(Raw(Str(v.clone().add(&n.to_string())))),
        (Some(Raw(Str(v))), "+=", Raw(Str(n))) => Some(Raw(Str(v.clone().add(n)))),
        (Some(Array(values)), "+=", Array(new_values)) => {
            Some(Array([values.clone(), new_values.clone()].concat()))
        }
        // -= operator
        (Some(Raw(Num(v))), "-=", Raw(Num(n))) => Some(Raw(Num(v - n))),
        (Some(Raw(Num(v))), "-=", Raw(Str(n))) => n.parse::<i64>().ok().map(|n| Raw(Num(v - n))),
        (Some(Raw(Str(v))), "-=", Raw(Num(n))) => v.parse::<i64>().ok().map(|v| Raw(Num(v - n))),
        (Some(Raw(Str(v))), "-=", Raw(Str(n))) => {
            if let (Ok(v), Ok(n)) = (v.parse::<i64>(), n.parse::<i64>()) {
                Some(Raw(Num(v - n)))
            } else {
                None
            }
        }

        // *= operator
        (Some(Raw(Num(v))), "*=", Raw(Num(n))) => Some(Raw(Num(v * n))),
        (Some(Raw(Num(v))), "*=", Raw(Str(n))) => n.parse::<i64>().ok().map(|n| Raw(Num(v * n))),
        (Some(Raw(Str(v))), "*=", Raw(Num(n))) => Some(Raw(Str(v.repeat(*n as usize)))),
        (Some(Raw(Str(v))), "*=", Raw(Str(n))) => {
            n.parse::<usize>().ok().map(|n| Raw(Str(v.repeat(n))))
        }

        // /= operator
        (Some(Raw(Num(v))), "/=", Raw(Num(n))) => Some(Raw(Num(v / n))),
        (Some(Raw(Num(v))), "/=", Raw(Str(n))) => n.parse::<i64>().ok().map(|n| Raw(Num(v / n))),
        (Some(Raw(Str(v))), "/=", Raw(Num(n))) => v.parse::<i64>().ok().map(|v| Raw(Num(v / n))),
        (Some(Raw(Str(v))), "/=", Raw(Str(n))) => {
            if let (Ok(v), Ok(n)) = (v.parse::<i64>(), n.parse::<i64>()) {
                Some(Raw(Num(v / n)))
            } else {
                None
            }
        }

        // %= operator
        (Some(Raw(Num(v))), "%=", Raw(Num(n))) => Some(Raw(Num(v % n))),
        (Some(Raw(Num(v))), "%=", Raw(Str(n))) => n.parse::<i64>().ok().map(|n| Raw(Num(v % n))),
        (Some(Raw(Str(v))), "%=", Raw(Num(n))) => v.parse::<i64>().ok().map(|v| Raw(Num(v % n))),
        (Some(Raw(Str(v))), "%=", Raw(Str(n))) => {
            if let (Ok(v), Ok(n)) = (v.parse::<i64>(), n.parse::<i64>()) {
                Some(Raw(Num(v % n)))
            } else {
                None
            }
        }

        _ => None,
    }
}

/// Static Var rule is used to replace
/// Variable by its static and predictable value
///
/// # Example
/// ```
/// extern crate tree_sitter;
/// extern crate tree_sitter_powershell;
///
/// use minusone::tree::{HashMapStorage, Tree};
/// use minusone::ps::build_powershell_tree;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::integer::ParseInt;
/// use minusone::ps::var::Var;
/// use minusone::ps::linter::Linter;
/// use minusone::ps::strategy::PowershellStrategy;
///
/// let mut tree = build_powershell_tree("\
/// $foo = 4
/// Write-Debug $foo\
/// ").unwrap();
/// tree.apply_mut_with_strategy(&mut (ParseInt::default(), Forward::default(), Var::default()), PowershellStrategy::default()).unwrap();
///
/// let mut ps_litter_view = Linter::default();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\
/// $foo = 4
/// Write-Debug 4\
/// ");
/// ```
#[derive(Default)]
pub struct StaticVar;

impl<'a> RuleMut<'a> for StaticVar {
    type Language = Powershell;

    fn enter(
        &mut self,
        _node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        Ok(())
    }

    fn leave(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        let view = node.view();
        if view.kind() == "variable" {
            match view.text()?.to_lowercase().as_str() {
                "$shellid" => {
                    trace!("Var (L): Setting node with special variable $shellid");
                    node.set(Raw(Str(String::from("Microsoft.Powershell"))))
                }
                "$?" => {
                    trace!("Var (L): Setting node with special variable $?");
                    node.set(Raw(Bool(true)))
                }
                "$null" => {
                    trace!("Var (L): Setting node with special variable $null");
                    node.set(Null)
                }
                "$pshome" => {
                    trace!("Var (L): Setting node with special variable $pshome");
                    node.set(Raw(Str(String::from(
                        "C:\\Windows\\System32\\WindowsPowerShell\\v1.0",
                    ))))
                }
                "$verbosepreference" => {
                    trace!("Var (L): Setting node with special variable $verbosepreference");
                    node.set(Raw(Str(String::from("SilentlyContinue"))))
                }
                _ => (),
            }
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct UnusedVar {
    pub vars: HashMap<String, bool>,
}

impl UnusedVar {
    pub fn is_unused(&self, var_name: &str) -> bool {
        !self.vars.get(var_name).unwrap_or(&false)
    }
}

impl<'a> Rule<'a> for UnusedVar {
    type Language = ();

    fn enter(&mut self, _node: &Node<'a, Self::Language>) -> MinusOneResult<bool> {
        Ok(true)
    }

    fn leave(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<()> {
        if node.kind() == "variable"
            && let Some(var_name) = Var::extract(node.text()?)
            && node
                .get_parent_of_types(vec!["left_assignment_expression"])
                .is_none()
        {
            self.vars.insert(var_name, true);
        }
        Ok(())
    }
}
