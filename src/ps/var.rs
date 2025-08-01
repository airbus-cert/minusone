use error::{Error, MinusOneResult};
use ps::Powershell;
use ps::Powershell::{Array, Null, Raw, Type};
use ps::Value::{self, Bool, Num, Str};
use regex::Regex;
use rule::{Rule, RuleMut};
use scope::ScopeManager;
use std::collections::{BTreeMap, HashMap};
use std::ops::Add;
use tree::{BranchFlow, ControlFlow, Node, NodeMut};

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
/// let mut ps_litter_view = Linter::new();
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
                .assign(s, Powershell::Unknown)
        });
    }
    fn forget_assigned_var<T>(&mut self, node: &Node<T>) -> MinusOneResult<()> {
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
                {
                    if let Some(var_name) = Var::extract(child.text()?) {
                        self.scope_manager.current_mut().forget(&var_name);
                    }
                }
            } else {
                self.forget_assigned_var(&child)?;
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

        if let Some(cap) = re_simple.captures(&var).or(re_braced.captures(&var)) {
            if let Some(name) = cap.name("name") {
                if let Some(provider) = cap.name("provider") {
                    if provider.as_str() != "variable" {
                        return None;
                    }
                }
                return Some(name.as_str().to_string());
            }
        }

        return None;
    }

    /// Resolve the name of a variable pattern given the current scope
    ///
    /// Use for patterns used by variable, get-variable, set-variable, get-childitem...
    fn resolve_wildcarded(&self, variable_name: String) -> Option<String> {
        if variable_name.contains("*") {
            let re = Regex::new(&*format!("^{}$", variable_name.replace("*", ".*"))).unwrap();
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
            scope_manager: ScopeManager::new(),
        };
        new.reset_scope_manager();
        new
    }
}

pub fn find_variable_node<'a, T>(node: &Node<'a, T>) -> Option<Node<'a, T>> {
    for child in node.iter() {
        if child.kind() == "variable" {
            if let Some(parent) = child.parent() {
                if parent.kind() == "unary_expression" {
                    return Some(child);
                }
            }
        } else if let Some(new_node) = find_variable_node(&child) {
            return Some(new_node);
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
        let view = node.view();
        match view.kind() {
            "program" => self.reset_scope_manager(),
            "function_statement" => self.scope_manager.enter(),
            "}" => {
                if let Some(parent) = view.parent() {
                    if parent.kind() == "statement_block" || parent.kind() == "function_statement" {
                        self.scope_manager.leave();
                    }
                }
            },

            // Each time I start an unpredictable branch I forget all assigned var in this block
            "statement_block" => {
                // record var block during new statement blocks
                self.scope_manager.enter();
                if flow == ControlFlow::Continue(BranchFlow::Unpredictable) {
                    self.forget_assigned_var(&view)?;
                }
            }

            "while_statement" => {
                // before evaluate while condition
                // we need to forget all var that will be assigned in corresponding statement block
                self.forget_assigned_var(&view)?;
            }

            // in the enter function because pre increment before assigned
            "pre_increment_expression" | "pre_decrement_expression" => {
                if let Some(variable) = view.child(1).ok_or(Error::invalid_child())?.child(0) {
                    if let Some(var_name) = Var::extract(variable.text()?) {
                        if let Some(Raw(Num(v))) =
                            self.scope_manager.current_mut().get_var_mut(&var_name)
                        {
                            if view.kind() == "pre_increment_expression" {
                                *v += 1;
                            } else {
                                *v -= 1;
                            }
                        } else {
                            self.scope_manager.current_mut().forget(&var_name)
                        }
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
        let view = node.view();
        match view.kind() {
            "assignment_expression" => {
                // Assign var value if it's possible
                if let (Some(left), Some(operator), Some(right)) =
                    (view.child(0), view.child(1), view.child(2))
                {
                    if let Some(var) = find_variable_node(&left) {
                        if let Some(var_name) = Var::extract(var.text()?) {
                            let scope = self.scope_manager.current_mut();
                            if let (current_value, Some(new_value)) =
                                (scope.get_var(&var_name), right.data())
                            {
                                // disable anything from for_initializer
                                if !view
                                    .get_parent_of_types(vec!["for_initializer"])
                                    .is_none() {
                                    scope.forget(&var_name);
                                }
                                else {
                                    // only predictable assignment is handled of local var
                                    let is_local = scope.is_local(&var_name).unwrap_or(true);
                                    if flow == ControlFlow::Continue(BranchFlow::Predictable) || is_local {
                                        match assign_handler(current_value, operator, new_value) {
                                            Some(assign_value) => scope.assign(&var_name, assign_value),
                                            _ => scope.forget(&var_name),
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            "variable" => {
                if let Some(var_name) = Var::extract(view.text()?) {
                    // forget variable with [ref] operator
                    if let Some(cast_expression) = view.get_parent_of_types(vec!["cast_expression"])
                    {
                        if let Some(Type(typename)) = cast_expression.child(0).unwrap().data() {
                            if typename.to_lowercase() == "ref" {
                                self.scope_manager.current_mut().forget(&var_name)
                            }
                        }
                    }

                    // check if we are not on the left part of an assignment expression
                    // already handle by the previous case
                    if view
                        .get_parent_of_types(vec!["left_assignment_expression"])
                        .is_none()
                    {
                        // Try to assign variable member
                        if let Some(data) = self.scope_manager.current_mut().get_var(&var_name) {
                            node.set(data.clone());
                        } else {
                            self.scope_manager.current_mut().in_use(&var_name);
                        }
                    }
                }
            }
            // pre_increment_expression is safe to forward due to the enter function handler
            "pre_increment_expression" | "pre_decrement_expression" => {
                if let Some(expression) = view.child(1) {
                    if let Some(expression_data) = expression.data() {
                        node.set(expression_data.clone())
                    }
                }
            }
            // in the enter function because pre increment before assigned
            "post_increment_expression" | "post_decrement_expression" => {
                if let Some(variable) = view.child(0) {
                    if let Some(var_name) = Var::extract(variable.text()?) {
                        let kind = view.kind();

                        if let Some(Raw(Num(v))) =
                            self.scope_manager.current_mut().get_var_mut(&var_name)
                        {
                            // we set the variable before ...
                            if let Some(variable_data) = variable.data() {
                                node.set(variable_data.clone())
                            }
                            // ... assign it
                            if kind == "post_increment_expression" {
                                *v += 1;
                            } else {
                                *v -= 1;
                            }
                        } else {
                            self.scope_manager.current_mut().forget(&var_name)
                        }
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
                            {
                                if let Some(arg_1) = argument_expression_list.child(0) {
                                    if let Some(var_name) = Var::extract(arg_1.text()?) {
                                        if let Some(Array(data)) =
                                            self.scope_manager.current_mut().get_var_mut(&var_name)
                                        {
                                            data.reverse();
                                        }
                                    }
                                }
                            }
                        }
                        _ => {
                            // Any array passed as param is forgotten
                            if let Some(argument_expression_list) =
                                args_list.named_child("argument_expression_list")
                            {
                                for arg in argument_expression_list.iter() {
                                    if let Some(var_name) = Var::extract(arg.text()?) {
                                        if let Some(Array(_)) =
                                            self.scope_manager.current_mut().get_var(&var_name)
                                        {
                                            self.scope_manager.current_mut().forget(&var_name);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            "command" => {
                if let Some(command_name) = view.child(0) {
                    match command_name.text()?.to_lowercase().as_str() {
                        "variable" => {
                            if let Some(command_elements) = view.child(1) {
                                if let Some(variable_name) = command_elements.child(1) {
                                    if let Some(variable_name) = self
                                        .resolve_wildcarded(variable_name.text()?.to_lowercase())
                                    {
                                        if let Some(Raw(data)) =
                                            self.scope_manager.current().get_var(&variable_name)
                                        {
                                            node.set(Var::hashmap(variable_name, data));
                                        } else {
                                            self.scope_manager.current_mut().in_use(&variable_name);
                                        }
                                    }
                                }
                            }
                        }
                        "get-variable" | "gv" => {
                            if let Some(command_elements) = view.child(1) {
                                if let Some(variable) = command_elements.child(1) {
                                    if let Some(variable_name) =
                                        self.resolve_wildcarded(variable.text()?.to_lowercase())
                                    {
                                        if let Some(Raw(data)) =
                                            self.scope_manager.current().get_var(&variable_name)
                                        {
                                            let value_param = command_elements
                                                .child(3)
                                                .is_some_and(|command_parameter| {
                                                    command_parameter.kind() == "command_parameter"
                                                        && command_parameter.text().is_ok_and(
                                                            |text| {
                                                                "-valueonly".starts_with(
                                                                    &text.to_lowercase(),
                                                                )
                                                            },
                                                        )
                                                });

                                            if value_param {
                                                node.set(Raw(data.clone()));
                                            } else {
                                                node.set(Var::hashmap(variable_name, data));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        "set-variable" | "sv" => {
                            if let Some(command_elements) = view.child(1) {
                                if let (Some(variable_name_node), Some(variable_value_node)) =
                                    (command_elements.child(1), command_elements.child(3))
                                {
                                    if let Some(Raw(variable_value)) = variable_value_node.data() {
                                        if let Some(variable_name) =
                                            if let Some(Raw(variable_name)) =
                                                variable_name_node.data()
                                            {
                                                Some(variable_name.to_string())
                                            } else if variable_name_node.kind() == "generic_token" {
                                                Some(variable_name_node.text()?.to_lowercase())
                                            } else {
                                                None
                                            }
                                        {
                                            if let Some(variable_name) =
                                                self.resolve_wildcarded(variable_name)
                                            {
                                                self.scope_manager.current_mut().assign(
                                                    &variable_name,
                                                    Powershell::Raw(variable_value.clone()),
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        "get-childitem" | "gci" | "ls" => {
                            if let Some(command_elements) = view.child(1) {
                                if let Some(item_name_node) = command_elements.child(1) {
                                    let item_name = item_name_node.text()?.to_lowercase();
                                    let re = Regex::new(r"^variable:\/?(.*)$").unwrap();
                                    if let Some(variable_name) =
                                        re.captures(&item_name).and_then(|cap| cap.get(1))
                                    {
                                        if let Some(variable_name) = self
                                            .resolve_wildcarded(variable_name.as_str().to_string())
                                        {
                                            if let Some(Raw(data)) =
                                                self.scope_manager.current().get_var(&variable_name)
                                            {
                                                node.set(Var::hashmap(variable_name, data));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        "set-item" | "si" => {
                            if let Some(command_elements) = view.child(1) {
                                if let (Some(item_name_node), Some(item_value_node)) =
                                    (command_elements.child(1), command_elements.child(3))
                                {
                                    if let Some(Raw(item_value)) = item_value_node.data() {
                                        let item_name = item_name_node.text()?.to_lowercase();
                                        let re = Regex::new(r"^variable:\/?(.*)$").unwrap();
                                        if let Some(variable_name) =
                                            re.captures(&item_name).and_then(|cap| cap.get(1))
                                        {
                                            if let Some(variable_name) = self.resolve_wildcarded(
                                                variable_name.as_str().to_string(),
                                            ) {
                                                self.scope_manager.current_mut().assign(
                                                    &variable_name,
                                                    Powershell::Raw(item_value.clone()),
                                                );
                                            }
                                        }
                                    }
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
        (Some(Raw(Num(v))), "+=", Raw(Str(n))) => {
            n.parse::<i64>().ok().and_then(|n| Some(Raw(Num(v + n))))
        }
        (Some(Raw(Str(v))), "+=", Raw(Num(n))) => Some(Raw(Str(v.clone().add(&n.to_string())))),
        (Some(Raw(Str(v))), "+=", Raw(Str(n))) => Some(Raw(Str(v.clone().add(&n)))),

        // -= operator
        (Some(Raw(Num(v))), "-=", Raw(Num(n))) => Some(Raw(Num(v - n))),
        (Some(Raw(Num(v))), "-=", Raw(Str(n))) => {
            n.parse::<i64>().ok().and_then(|n| Some(Raw(Num(v - n))))
        }
        (Some(Raw(Str(v))), "-=", Raw(Num(n))) => {
            v.parse::<i64>().ok().and_then(|v| Some(Raw(Num(v - n))))
        }
        (Some(Raw(Str(v))), "-=", Raw(Str(n))) => {
            if let (Ok(v), Ok(n)) = (v.parse::<i64>(), n.parse::<i64>()) {
                Some(Raw(Num(v - n)))
            } else {
                None
            }
        }

        // *= operator
        (Some(Raw(Num(v))), "*=", Raw(Num(n))) => Some(Raw(Num(v * n))),
        (Some(Raw(Num(v))), "*=", Raw(Str(n))) => {
            n.parse::<i64>().ok().and_then(|n| Some(Raw(Num(v * n))))
        }
        (Some(Raw(Str(v))), "*=", Raw(Num(n))) => Some(Raw(Str(v.repeat(*n as usize)))),
        (Some(Raw(Str(v))), "*=", Raw(Str(n))) => n
            .parse::<usize>()
            .ok()
            .and_then(|n| Some(Raw(Str(v.repeat(n))))),

        // /= operator
        (Some(Raw(Num(v))), "/=", Raw(Num(n))) => Some(Raw(Num(v / n))),
        (Some(Raw(Num(v))), "/=", Raw(Str(n))) => {
            n.parse::<i64>().ok().and_then(|n| Some(Raw(Num(v / n))))
        }
        (Some(Raw(Str(v))), "/=", Raw(Num(n))) => {
            v.parse::<i64>().ok().and_then(|v| Some(Raw(Num(v / n))))
        }
        (Some(Raw(Str(v))), "/=", Raw(Str(n))) => {
            if let (Ok(v), Ok(n)) = (v.parse::<i64>(), n.parse::<i64>()) {
                Some(Raw(Num(v / n)))
            } else {
                None
            }
        }

        // %= operator
        (Some(Raw(Num(v))), "%=", Raw(Num(n))) => Some(Raw(Num(v % n))),
        (Some(Raw(Num(v))), "%=", Raw(Str(n))) => {
            n.parse::<i64>().ok().and_then(|n| Some(Raw(Num(v % n))))
        }
        (Some(Raw(Str(v))), "%=", Raw(Num(n))) => {
            v.parse::<i64>().ok().and_then(|v| Some(Raw(Num(v % n))))
        }
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
/// extern crate minusone;
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
/// let mut ps_litter_view = Linter::new();
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
                "$shellid" => node.set(Raw(Str(String::from("Microsoft.Powershell")))),
                "$?" => node.set(Raw(Bool(true))),
                "$null" => node.set(Null),
                "$pshome" => node.set(Raw(Str(String::from(
                    "C:\\Windows\\System32\\WindowsPowerShell\\v1.0",
                )))),
                "$verbosepreference" => node.set(Raw(Str(String::from("SilentlyContinue")))),
                _ => (),
            }
        }
        Ok(())
    }
}


#[derive(Default)]
pub struct UnusedVar {
    pub vars: HashMap<String, bool>
}

impl UnusedVar {
    pub fn is_unused(&self, var_name: &str) -> bool {
        !self.vars.get(var_name).unwrap_or(&false)
    }
}

impl<'a> Rule<'a> for UnusedVar {
    type Language = ();

    fn enter(
        &mut self,
        _node: &Node<'a, Self::Language>,
    ) -> MinusOneResult<bool> {
        Ok(true)
    }

    fn leave(
        &mut self,
        node: &Node<'a, Self::Language>,
    ) -> MinusOneResult<()> {
        if node.kind() == "variable" {
            if let Some(var_name) = Var::extract(node.text()?) {
                if node.get_parent_of_types(vec!["left_assignment_expression"]).is_none()
                {
                    self.vars.insert(var_name, true);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use ps::access::AccessHashMap;
    use ps::bool::ParseBool;
    use ps::build_powershell_tree;
    use ps::forward::Forward;
    use ps::hash::ParseHash;
    use ps::integer::{AddInt, ParseInt};
    use ps::strategy::PowershellStrategy;
    use ps::string::ParseString;

    #[test]
    fn test_static_replacement() {
        let mut tree = build_powershell_tree("$foo = 4\nWrite-Debug $foo").unwrap();

        tree.apply_mut_with_strategy(
            &mut (ParseInt::default(), Forward::default(), Var::default()),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        // Write-Debug 4
        // (program
        //  (statement_list inferred_type: None)
        //   (pipeline inferred_type: None
        //    (command inferred_type: None
        //     (command_name inferred_type: None)
        //     (command_elements inferred_type: None)
        //      (command_param_sep)
        //      (variable inferred_type: Some(Number(4)))))))))
        assert_eq!(
            *tree
                .root()
                .unwrap() // program
                .child(0)
                .unwrap() // statement_list
                .child(1)
                .unwrap() // pipeline
                .child(0)
                .unwrap() //command
                .child(1)
                .unwrap() // command_elements
                .child(1)
                .unwrap() // variable
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(4))
        );
    }

    #[test]
    fn test_unfollow_var_use_unknow_var() {
        let mut tree = build_powershell_tree("$foo = $toto\nWrite-Debug $foo").unwrap();

        tree.apply_mut_with_strategy(
            &mut (ParseInt::default(), Forward::default(), Var::default()),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        // Write-Debug 4
        // (program
        //  (statement_list inferred_type: None)
        //   (pipeline inferred_type: None
        //    (command inferred_type: None
        //     (command_name inferred_type: None)
        //     (command_elements inferred_type: None)
        //      (variable inferred_type: Some(Number(4)))))))))
        assert_eq!(
            tree.root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(1)
                .unwrap() // pipeline
                .child(0)
                .unwrap() //command
                .child(1)
                .unwrap() // command_elements
                .child(0)
                .unwrap() // variable
                .data(),
            None
        );
    }

    #[test]
    fn test_static_var_shell_id() {
        let mut tree = build_powershell_tree("$shellid").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                Forward::default(),
                StaticVar::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        assert_eq!(
            *tree
                .root()
                .unwrap() // program
                .child(0)
                .unwrap() // statement_list
                .child(0)
                .unwrap() // pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Str("Microsoft.Powershell".to_string()))
        );
    }

    #[test]
    fn test_unfollow_var_use_in_if_statement() {
        let mut tree =
            build_powershell_tree("$foo = 0\nif(unknown) { $foo = 5 }\n White-Debug $foo").unwrap();

        tree.apply_mut_with_strategy(
            &mut (ParseInt::default(), Forward::default(), Var::default()),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        // Write-Debug $foo
        // (program
        //  (statement_list inferred_type: None)
        //   (pipeline inferred_type: None
        //    (command inferred_type: None
        //     (command_name inferred_type: None)
        //     (command_elements inferred_type: None)
        //      (command_argument_sep)
        //      (variable inferred_type: None)))))))
        assert_eq!(
            tree.root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(2)
                .unwrap() // pipeline
                .child(0)
                .unwrap() //command
                .child(1)
                .unwrap() // command_elements
                .child(1)
                .unwrap() // variable
                .data(),
            None
        );
    }

    #[test]
    fn test_infer_var_use_in_if_statement_predictable() {
        let mut tree =
            build_powershell_tree("$foo = 0\nif($true) { $foo = 5 }\nWhite-Debug $foo").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                Forward::default(),
                Var::default(),
                ParseBool::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        // Write-Debug 5
        // (program
        //  (statement_list inferred_type: None)
        //   (pipeline inferred_type: None
        //    (command inferred_type: None
        //     (command_name inferred_type: None)
        //     (command_elements inferred_type: None)
        //      (command_argument_sep)
        //      (variable inferred_type: Some(Num(5)))))))
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(2)
                .unwrap() // pipeline
                .child(0)
                .unwrap() //command
                .child(1)
                .unwrap() // command_elements
                .child(1)
                .unwrap() // variable
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(5))
        );
    }

    #[test]
    fn test_infer_var_use_in_if_statement_predictable_false() {
        let mut tree =
            build_powershell_tree("$foo = 0\nif($false) { $foo = 5 }\nWhite-Debug $foo").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                Forward::default(),
                Var::default(),
                ParseBool::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        // Write-Debug 5
        // (program
        //  (statement_list inferred_type: None)
        //   (pipeline inferred_type: None
        //    (command inferred_type: None
        //     (command_name inferred_type: None)
        //     (command_elements inferred_type: None)
        //      (command_argument_sep)
        //      (variable inferred_type: Some(Num(5)))))))
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(2)
                .unwrap() // pipeline
                .child(0)
                .unwrap() //command
                .child(1)
                .unwrap() // command_elements
                .child(1)
                .unwrap() // variable
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(0))
        );
    }

    #[test]
    fn test_infer_var_use_in_if_else_statement_predictable() {
        let mut tree = build_powershell_tree(
            "$foo = 0\nif($false) { $foo = 5 }else { $foo = 8 }\nWhite-Debug $foo",
        )
        .unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                Forward::default(),
                Var::default(),
                ParseBool::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        // Write-Debug 5
        // (program
        //  (statement_list inferred_type: None)
        //   (pipeline inferred_type: None
        //    (command inferred_type: None
        //     (command_name inferred_type: None)
        //     (command_elements inferred_type: None)
        //      (command_argument_sep)
        //      (variable inferred_type: Some(Num(5)))))))
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(2)
                .unwrap() // pipeline
                .child(0)
                .unwrap() //command
                .child(1)
                .unwrap() // command_elements
                .child(1)
                .unwrap() // variable
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(8))
        );
    }

    #[test]
    fn test_infer_var_use_in_if_elseif_else_statement_predictable() {
        let mut tree = build_powershell_tree("$foo = 0\nif($false) { $foo = 5 }elseif($true) { $foo = 6 } else {$foo = 7}\nWhite-Debug $foo").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                Forward::default(),
                Var::default(),
                ParseBool::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        // Write-Debug 5
        // (program
        //  (statement_list inferred_type: None)
        //   (pipeline inferred_type: None
        //    (command inferred_type: None
        //     (command_name inferred_type: None)
        //     (command_elements inferred_type: None)
        //      (command_argument_sep)
        //      (variable inferred_type: Some(Num(5)))))))
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(2)
                .unwrap() // pipeline
                .child(0)
                .unwrap() //command
                .child(1)
                .unwrap() // command_elements
                .child(1)
                .unwrap() // variable
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(6))
        );
    }

    #[test]
    fn test_infer_var_use_in_if_elseif_else_statement_unpredictable() {
        let mut tree = build_powershell_tree("$foo = 0\nif($false) { $foo = 5 }elseif(unknown) { $foo = 6 } else {$foo = 7}\nWhite-Debug $foo").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                Forward::default(),
                Var::default(),
                ParseBool::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        // Write-Debug 5
        // (program
        //  (statement_list inferred_type: None)
        //   (pipeline inferred_type: None
        //    (command inferred_type: None
        //     (command_name inferred_type: None)
        //     (command_elements inferred_type: None)
        //      (command_argument_sep)
        //      (variable inferred_type: Some(Num(5)))))))
        assert_eq!(
            tree.root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(2)
                .unwrap() // pipeline
                .child(0)
                .unwrap() //command
                .child(1)
                .unwrap() // command_elements
                .child(1)
                .unwrap() // variable
                .data(),
            None
        );
    }

    #[test]
    fn test_infer_var_use_in_if_elseif_else_statement_predictable_in_else() {
        let mut tree = build_powershell_tree("$foo = 0\nif($false) { $foo = 5 }elseif($false) { $foo = 6 } else {$foo = 7}\nWhite-Debug $foo").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                Forward::default(),
                Var::default(),
                ParseBool::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        // Write-Debug 5
        // (program
        //  (statement_list inferred_type: None)
        //   (pipeline inferred_type: None
        //    (command inferred_type: None
        //     (command_name inferred_type: None)
        //     (command_elements inferred_type: None)
        //      (command_argument_sep)
        //      (variable inferred_type: Some(Num(5)))))))
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(2)
                .unwrap() // pipeline
                .child(0)
                .unwrap() //command
                .child(1)
                .unwrap() // command_elements
                .child(1)
                .unwrap() // variable
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(7))
        );
    }

    #[test]
    fn test_infer_var_use_in_while_statement_use_in_statement() {
        // var is used in the loop statement -> not inferred in the condition and forget
        let mut tree =
            build_powershell_tree("$a = 1\nwhile($a -gt 0) { $a = $a + 1 }\nWhite-Debug $a")
                .unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                Forward::default(),
                Var::default(),
                ParseBool::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        // Write-Debug 5
        // (program
        //  (statement_list inferred_type: None)
        //   (pipeline inferred_type: None
        //    (command inferred_type: None
        //     (command_name inferred_type: None)
        //     (command_elements inferred_type: None)
        //      (command_argument_sep)
        //      (variable inferred_type: Some(Num(5)))))))
        assert_eq!(
            tree.root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(2)
                .unwrap() // pipeline
                .child(0)
                .unwrap() //command
                .child(1)
                .unwrap() // command_elements
                .child(1)
                .unwrap() // variable
                .data(),
            None
        );
    }

    #[test]
    fn test_infer_var_use_in_while_statement_not_use_in_statement() {
        // var is used in the loop statement -> not inferred in the condition and forget
        let mut tree =
            build_powershell_tree("$a = 1\nwhile($a -gt 0) { $b = $a + 1 }\nWhite-Debug $a")
                .unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                Forward::default(),
                Var::default(),
                ParseBool::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        // Write-Debug 5
        // (program
        //  (statement_list inferred_type: None)
        //   (pipeline inferred_type: None
        //    (command inferred_type: None
        //     (command_name inferred_type: None)
        //     (command_elements inferred_type: None)
        //      (command_argument_sep)
        //      (variable inferred_type: Some(Num(5)))))))
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(2)
                .unwrap() // pipeline
                .child(0)
                .unwrap() //command
                .child(1)
                .unwrap() // command_elements
                .child(1)
                .unwrap() // variable
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(1))
        );
    }

    #[test]
    fn test_infer_var_use_in_function_statement() {
        // infer global var in function statement
        let mut tree = build_powershell_tree("$a = 1\nFunction invoke { $a }").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                Forward::default(),
                Var::default(),
                ParseBool::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        // Write-Debug 5
        // (program
        //  (statement_list inferred_type: None)
        //   (assignment_expression inferred_type: None) ...
        //   (function_statement inferred_type: None
        //    (function inferred_type: None)
        //    (function_name inferred_type: None)
        //    ({ inferred_type: None)
        //    (script_block inferred_type: None
        //     (script_block_body inferred_type: None
        //      (statement_list inferred_type: None
        //       (pipeline inferred_type: Some(Raw(Num(1)))
        //        (logical_expression inferred_type: Some(Raw(Num(1)))
        //         (bitwise_expression inferred_type: None
        //          (comparison_expression inferred_type: None
        //           (additive_expression inferred_type: None
        //            (multiplicative_expression inferred_type: None
        //             (format_expression inferred_type: None
        //              (range_expression inferred_type: None
        //               (array_literal_expression inferred_type: None
        //                (unary_expression inferred_type: None
        //                 (variable inferred_type: None))))))))))))))
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(1)
                .unwrap() // function_statement
                .child(3)
                .unwrap() //script_block
                .child(0)
                .unwrap() // script_block_body
                .child(0)
                .unwrap() // statement_list
                .child(0)
                .unwrap() // pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(1))
        );
    }

    #[test]
    fn test_wildcarded_variable() {
        // infer global var in function statement
        let mut tree = build_powershell_tree("sV my-var 1\n(varIable M*ar).vaLue").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                Forward::default(),
                Var::default(),
                ParseHash::default(),
                AccessHashMap::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        // (program inferred_type: None
        //  (statement_list inferred_type: None
        //   (pipeline inferred_type: None
        //    (command inferred_type: None
        //     (command_name inferred_type: None)
        //     (command_elements inferred_type: None
        //      (command_argument_sep inferred_type: None
        //       (  inferred_type: None))
        //      (generic_token inferred_type: None)
        //      (command_argument_sep inferred_type: None
        //       (  inferred_type: None))
        //      (array_literal_expression inferred_type: Some(Raw(Num(1)))
        //       (unary_expression inferred_type: None
        //        (integer_literal inferred_type: None
        //         (decimal_integer_literal inferred_type: None)))))))
        //   (pipeline inferred_type: Some(Raw(Num(1)))
        //    (logical_expression inferred_type: Some(Raw(Num(1)))
        //     (bitwise_expression inferred_type: None
        //      (comparison_expression inferred_type: None
        //       (additive_expression inferred_type: None
        //        (multiplicative_expression inferred_type: None
        //         (format_expression inferred_type: None
        //          (range_expression inferred_type: None
        //           (array_literal_expression inferred_type: None
        //            (unary_expression inferred_type: None
        //             (member_access inferred_type: None
        //              (parenthesized_expression inferred_type: Some(HashMap({Str("name"): Str("my-var"), Str("value"): Num(1)}))
        //               (( inferred_type: None)
        //               (pipeline inferred_type: None
        //                (command inferred_type: Some(HashMap({Str("name"): Str("my-var"), Str("value"): Num(1)}))
        //                 (command_name inferred_type: None)
        //                 (command_elements inferred_type: None
        //                  (command_argument_sep inferred_type: None
        //                   (  inferred_type: None))
        //                  (generic_token inferred_type: None))))
        //               () inferred_type: None))
        //              (. inferred_type: None)
        //              (member_name inferred_type: None
        //               (simple_name inferred_type: None)))))))))))))))
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(1)
                .unwrap() // pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(1))
        );
    }

    #[test]
    fn test_wildcarded_getvariable() {
        // infer global var in function statement
        let mut tree = build_powershell_tree("sV my-var 1\ngV M*ar -vaL").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                Forward::default(),
                Var::default(),
                ParseHash::default(),
                AccessHashMap::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        // (program inferred_type: None
        // (statement_list inferred_type: None
        //  (pipeline inferred_type: None
        //   (command inferred_type: None
        //    (command_name inferred_type: None)
        //    (command_elements inferred_type: None
        //     (command_argument_sep inferred_type: None
        //      (  inferred_type: None))
        //     (generic_token inferred_type: None)
        //     (command_argument_sep inferred_type: None
        //      (  inferred_type: None))
        //     (array_literal_expression inferred_type: Some(Raw(Num(1)))
        //      (unary_expression inferred_type: None
        //       (integer_literal inferred_type: None
        //        (decimal_integer_literal inferred_type: None)))))))
        //  (pipeline inferred_type: Some(Raw(Num(1)))
        //   (command inferred_type: Some(Raw(Num(1)))
        //    (command_name inferred_type: None)
        //    (command_elements inferred_type: None
        //     (command_argument_sep inferred_type: None
        //      (  inferred_type: None))
        //     (generic_token inferred_type: None)
        //     (command_argument_sep inferred_type: None
        //      (  inferred_type: None))
        //     (command_parameter inferred_type: None))))))

        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(1)
                .unwrap() // pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(1))
        );
    }

    #[test]
    fn test_wildcarded_getsetitem() {
        // infer global var in function statement
        let mut tree =
            build_powershell_tree("sv mYVAr 1\nsi variable:/M*ar 2\n(ls variable:*y*ar).value")
                .unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                Forward::default(),
                Var::default(),
                ParseHash::default(),
                AccessHashMap::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        // (program inferred_type: None
        // (statement_list inferred_type: None
        //  (pipeline inferred_type: None
        //   (command inferred_type: None
        //    (command_name inferred_type: None)
        //    (command_elements inferred_type: None
        //     (command_argument_sep inferred_type: None
        //      (  inferred_type: None))
        //     (generic_token inferred_type: None)
        //     (command_argument_sep inferred_type: None
        //      (  inferred_type: None))
        //     (array_literal_expression inferred_type: Some(Raw(Num(1)))
        //      (unary_expression inferred_type: None
        //       (integer_literal inferred_type: None
        //        (decimal_integer_literal inferred_type: None)))))))
        //  (pipeline inferred_type: None
        //   (command inferred_type: None
        //    (command_name inferred_type: None)
        //    (command_elements inferred_type: None
        //     (command_argument_sep inferred_type: None
        //      (  inferred_type: None))
        //     (generic_token inferred_type: None)
        //     (command_argument_sep inferred_type: None
        //      (  inferred_type: None))
        //     (array_literal_expression inferred_type: Some(Raw(Num(2)))
        //      (unary_expression inferred_type: None
        //       (integer_literal inferred_type: None
        //        (decimal_integer_literal inferred_type: None)))))))
        //  (pipeline inferred_type: Some(Raw(Num(2)))
        //   (logical_expression inferred_type: Some(Raw(Num(2)))
        //    (bitwise_expression inferred_type: None
        //     (comparison_expression inferred_type: None
        //      (additive_expression inferred_type: None
        //       (multiplicative_expression inferred_type: None
        //        (format_expression inferred_type: None
        //         (range_expression inferred_type: None
        //          (array_literal_expression inferred_type: None
        //           (unary_expression inferred_type: None
        //            (member_access inferred_type: None
        //             (parenthesized_expression inferred_type: Some(HashMap({Str("name"): Str("myvar"), Str("value"): Num(2)}))
        //              (( inferred_type: None)
        //              (pipeline inferred_type: None
        //               (command inferred_type: Some(HashMap({Str("name"): Str("myvar"), Str("value"): Num(2)}))
        //                (command_name inferred_type: None)
        //                (command_elements inferred_type: None
        //                 (command_argument_sep inferred_type: None
        //                  (  inferred_type: None))
        //                 (generic_token inferred_type: None))))
        //              () inferred_type: None))
        //             (. inferred_type: None)
        //             (member_name inferred_type: None
        //              (simple_name inferred_type: None)))))))))))))))

        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(2)
                .unwrap() // pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(2))
        );
    }

    #[test]
    fn test_add_assignment_operator_int_int() {
        // infer global var in function statement
        let mut tree = build_powershell_tree("$a=1;$a+=1;$a").unwrap();

        tree.apply_mut_with_strategy(
            &mut (ParseInt::default(), Forward::default(), Var::default()),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        // (program inferred_type: None
        //  (statement_list inferred_type: None
        //   (pipeline inferred_type: None
        //     ...)
        //   (empty_statement inferred_type: None
        //    ...)
        //   (pipeline inferred_type: None
        //    ...)
        //   (empty_statement inferred_type: None
        //    ...)
        //   (pipeline inferred_type: Some(Raw(Num(2)))  <--- correct infered type
        //    ...)

        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(4)
                .unwrap() // pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(2))
        );
    }
    #[test]
    fn test_add_assignment_operator_int_str() {
        // infer global var in function statement
        let mut tree = build_powershell_tree("$a=1;$a+=\"1\";$a").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                ParseString::default(),
                Forward::default(),
                Var::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(4)
                .unwrap() // pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(2))
        );
    }
    #[test]
    fn test_add_assignment_operator_str_int() {
        // infer global var in function statement
        let mut tree = build_powershell_tree("$a=\"1\";$a+=1;$a").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                ParseString::default(),
                Forward::default(),
                Var::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(4)
                .unwrap() // pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Str(String::from("11")))
        );
    }
    #[test]
    fn test_add_assignment_operator_str_str() {
        // infer global var in function statement
        let mut tree = build_powershell_tree("$a=\"1\";$a+=\"1\";$a").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                ParseString::default(),
                Forward::default(),
                Var::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(4)
                .unwrap() // pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Str(String::from("11")))
        );
    }

    #[test]
    fn test_sub_assignment_operator_int_int() {
        // infer global var in function statement
        let mut tree = build_powershell_tree("$a=1;$a-=1;$a").unwrap();

        tree.apply_mut_with_strategy(
            &mut (ParseInt::default(), Forward::default(), Var::default()),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(4)
                .unwrap() // pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(0))
        );
    }

    #[test]
    fn test_sub_assignment_operator_int_str() {
        // infer global var in function statement
        let mut tree = build_powershell_tree("$a=1;$a-=\"1\";$a").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                ParseString::default(),
                Forward::default(),
                Var::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(4)
                .unwrap() // pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(0))
        );
    }
    #[test]
    fn test_sub_assignment_operator_str_int() {
        // infer global var in function statement
        let mut tree = build_powershell_tree("$a=\"1\";$a-=1;$a").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                ParseString::default(),
                Forward::default(),
                Var::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(4)
                .unwrap() // pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(0))
        );
    }
    #[test]
    fn test_sub_assignment_operator_str_str() {
        // infer global var in function statement
        let mut tree = build_powershell_tree("$a=\"1\";$a-=\"1\";$a").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                ParseString::default(),
                Forward::default(),
                Var::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(4)
                .unwrap() // pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(0))
        );
    }

    #[test]
    fn test_mul_assignment_operator_int_int() {
        // infer global var in function statement
        let mut tree = build_powershell_tree("$a=2;$a*=3;$a").unwrap();

        tree.apply_mut_with_strategy(
            &mut (ParseInt::default(), Forward::default(), Var::default()),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(4)
                .unwrap() // pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(6))
        );
    }

    #[test]
    fn test_mul_assignment_operator_int_str() {
        // infer global var in function statement
        let mut tree = build_powershell_tree("$a=2;$a*=\"3\";$a").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                ParseString::default(),
                Forward::default(),
                Var::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(4)
                .unwrap() // pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(6))
        );
    }
    #[test]
    fn test_mul_assignment_operator_str_int() {
        // infer global var in function statement
        let mut tree = build_powershell_tree("$a=\"12\";$a*=3;$a").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                ParseString::default(),
                Forward::default(),
                Var::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(4)
                .unwrap() // pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Str(String::from("121212")))
        );
    }
    #[test]
    fn test_mul_assignment_operator_str_str() {
        // infer global var in function statement
        let mut tree = build_powershell_tree("$a=\"12\";$a*=\"3\";$a").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                ParseString::default(),
                Forward::default(),
                Var::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(4)
                .unwrap() // pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Str(String::from("121212")))
        );
    }

    #[test]
    fn test_div_assignment_operator_int_int() {
        // infer global var in function statement
        let mut tree = build_powershell_tree("$a=10;$a/=2;$a").unwrap();

        tree.apply_mut_with_strategy(
            &mut (ParseInt::default(), Forward::default(), Var::default()),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(4)
                .unwrap() // pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(5))
        );
    }

    #[test]
    fn test_div_assignment_operator_int_str() {
        // infer global var in function statement
        let mut tree = build_powershell_tree("$a=10;$a/=\"2\";$a").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseString::default(),
                ParseInt::default(),
                Forward::default(),
                Var::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        println!(
            "{:?}",
            tree.root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(4)
                .unwrap()
                .data()
        );

        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(4)
                .unwrap() // pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(5))
        );
    }
    #[test]
    fn test_div_assignment_operator_str_int() {
        // infer global var in function statement
        let mut tree = build_powershell_tree("$a=\"10\";$a/=2;$a").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                ParseString::default(),
                Forward::default(),
                Var::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(4)
                .unwrap() // pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(5))
        );
    }
    #[test]
    fn test_div_assignment_operator_str_str() {
        // infer global var in function statement
        let mut tree = build_powershell_tree("$a=\"10\";$a/=\"2\";$a").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                ParseString::default(),
                Forward::default(),
                Var::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(4)
                .unwrap() // pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(5))
        );
    }

    #[test]
    fn test_mod_assignment_operator_int_int() {
        // infer global var in function statement
        let mut tree = build_powershell_tree("$a=9;$a%=2;$a").unwrap();

        tree.apply_mut_with_strategy(
            &mut (ParseInt::default(), Forward::default(), Var::default()),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(4)
                .unwrap() // pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(1))
        );
    }

    #[test]
    fn test_mod_assignment_operator_int_str() {
        // infer global var in function statement
        let mut tree = build_powershell_tree("$a=9;$a%=\"2\";$a").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                ParseString::default(),
                Forward::default(),
                Var::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(4)
                .unwrap() // pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(1))
        );
    }
    #[test]
    fn test_mod_assignment_operator_str_int() {
        // infer global var in function statement
        let mut tree = build_powershell_tree("$a=\"9\";$a%=2;$a").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                ParseString::default(),
                Forward::default(),
                Var::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(4)
                .unwrap() // pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(1))
        );
    }
    #[test]
    fn test_mod_assignment_operator_str_str() {
        // infer global var in function statement
        let mut tree = build_powershell_tree("$a=\"9\";$a%=\"2\";$a").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                ParseString::default(),
                Forward::default(),
                Var::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        // We are waiting for
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(4)
                .unwrap() // pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(1))
        );
    }

    #[test]
    fn test_infer_local_var_type() {
        let mut tree = build_powershell_tree("try{$foo = 1;$foo + 2}catch{}").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                AddInt::default(),
                ParseString::default(),
                Forward::default(),
                Var::default(),
            ),
            PowershellStrategy::default(),
        )
            .unwrap();

        // We are waiting for
        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(0)
                .unwrap() // try_statement
                .child(1)
                .unwrap() // statement block
                .child(1)
                .unwrap() //statement list
                .child(2)
                .unwrap() //pipeline
                .data()
                .expect("Expecting inferred type"),
            Raw(Num(3))
        );
    }
}
