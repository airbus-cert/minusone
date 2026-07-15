use crate::error::MinusOneResult;
use crate::js::JavaScript::*;
use crate::js::Value::{Bool, Str};
use crate::js::array::flatten_array;
use crate::js::build_javascript_tree;
use crate::js::strategy::JavaScriptStrategy;
use crate::js::utils::{get_positional_arguments, method_name};
use crate::js::{JavaScript, JavaScriptRuleSet};
use crate::rule::{RuleMut, RuleSetBuilderType};
use crate::tree::{ControlFlow, Node, NodeMut};
use log::{trace, warn};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

const MAX_MAP_FILTER_DEPTH: usize = 4;

thread_local! {
    static MAP_FILTER_DEPTH: Cell<usize> = const { Cell::new(0) };
}

#[derive(Clone, Copy, PartialEq)]
enum MapFilterKind {
    Map,
    Filter,
}

enum Callback {
    UserFunction {
        params: Vec<String>,
        body_source: String,
        free_var_bindings: String,
    },
    Native(NativeConversion),
}

enum NativeConversion {
    Number,
    String,
}

impl NativeConversion {
    fn apply(&self, element: &JavaScript) -> JavaScript {
        match self {
            NativeConversion::Number => element.as_js_num(),
            NativeConversion::String => Raw(Str(match element {
                Raw(Str(s)) => s.clone(),
                Array(a) => flatten_array(a, None),
                v => v.to_string(),
            })),
        }
    }
}

/// Infers deterministic `Array.prototype.map` and `Array.prototype.filter` calls
/// on array literals. Also implement callback that is a bare reference to `Number`
/// or `String` (e.g. `arr.map(Number)`)
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::integer::{AddInt, ParseInt};
/// use minusone::js::array::ParseArray;
/// use minusone::js::functions::function::ParseFunction;
/// use minusone::js::r#loop::ArrayMapFilter;
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = [0, 1, 2].map(e => e + 1);").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(), AddInt::default(), ParseArray::default(),
///     ParseFunction::default(), ArrayMapFilter::default()
/// )).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "var x = [1, 2, 3];");
/// ```
#[derive(Default)]
pub struct ArrayMapFilter;

impl ArrayMapFilter {
    fn unwrap_parens(mut node: Node<JavaScript>) -> Node<JavaScript> {
        while node.kind() == "parenthesized_expression"
            && let Some(inner) = node.child(1)
        {
            node = inner;
        }
        node
    }

    fn callback_params(cb: &Node<JavaScript>) -> Vec<String> {
        if let Some(param) = cb.named_child("parameter") {
            return param
                .text()
                .map(|s| vec![s.to_string()])
                .unwrap_or_default();
        }

        if let Some(params) = cb.named_child("parameters") {
            return params
                .iter()
                .filter(|child| child.kind() == "identifier")
                .filter_map(|child| child.text().ok().map(|s| s.to_string()))
                .collect();
        }

        vec![]
    }

    fn callback_body_source(body: &Node<JavaScript>) -> Option<String> {
        if body.kind() != "statement_block" {
            let expr = body.text().ok()?;
            return Some(format!("({expr})"));
        }

        let mut prefix_statements = Vec::new();
        let mut return_expr: Option<String> = None;
        let mut return_count = 0usize;

        for statement in body.iter() {
            match statement.kind() {
                "{" | "}" => {}
                "if_statement" | "while_statement" | "do_statement" | "for_statement"
                | "for_in_statement" | "switch_statement" | "try_statement" => return None,
                "return_statement" => {
                    return_count += 1;
                    if return_count > 1 {
                        return None;
                    }
                    for i in 0..statement.child_count() {
                        if let Some(c) = statement.child(i)
                            && c.kind() != "return"
                            && c.kind() != ";"
                        {
                            return_expr = c.text().ok().map(|s| s.to_string());
                            break;
                        }
                    }
                }
                _ => {
                    if let Ok(text) = statement.text() {
                        prefix_statements.push(text.to_string());
                    }
                }
            }
        }

        if return_count != 1 {
            return None;
        }

        prefix_statements.push(format!("({})", return_expr?));
        Some(prefix_statements.join("\n"))
    }

    fn collect_free_var_bindings(body: &Node<JavaScript>, params: &[String]) -> String {
        let mut seen: std::collections::HashSet<String> = params.iter().cloned().collect();
        let mut bindings = String::new();
        Self::collect_free_vars_rec(body, &mut seen, &mut bindings);
        bindings
    }

    fn collect_free_vars_rec(
        node: &Node<JavaScript>,
        seen: &mut std::collections::HashSet<String>,
        bindings: &mut String,
    ) {
        for child in node.iter() {
            if child.kind() == "identifier"
                && let Ok(name) = child.text()
                && !seen.contains(name)
                && let Some(value) = child.data()
            {
                bindings.push_str(&format!("var {name} = {value};\n"));
                seen.insert(name.to_string());
            }
            Self::collect_free_vars_rec(&child, seen, bindings);
        }
    }

    /// Builds `<free var bindings> var <element_param> = <element>; var <index_param> = <index>; var <array_param> = <array>; <body>`
    fn evaluate(
        free_var_bindings: &str,
        params: &[String],
        body_source: &str,
        element: &JavaScript,
        index: usize,
        input: &[JavaScript],
    ) -> Option<JavaScript> {
        let mut bindings = free_var_bindings.to_string();
        if let Some(p) = params.first() {
            bindings.push_str(&format!("var {p} = {element};\n"));
        }
        if let Some(p) = params.get(1) {
            bindings.push_str(&format!("var {p} = {index};\n"));
        }
        if let Some(p) = params.get(2) {
            bindings.push_str(&format!("var {p} = {};\n", Array(input.to_vec())));
        }

        let program_source = format!("{bindings}{body_source}");

        let mut tree = build_javascript_tree(&program_source).ok()?;
        tree.apply_mut_with_strategy(
            &mut JavaScriptRuleSet::new(RuleSetBuilderType::WithoutRules(vec![])),
            JavaScriptStrategy,
        )
        .ok()?;

        let root = tree.root().ok()?;
        let mut result = None;
        for statement in root.iter() {
            if statement.kind() == "expression_statement" {
                result = statement
                    .iter()
                    .find(|c| c.kind() != ";")
                    .and_then(|c| c.data().cloned());
            }
        }

        result
    }

    fn collect_locals(node: &Node<JavaScript>, locals: &mut std::collections::HashSet<String>) {
        for child in node.iter() {
            if child.kind() == "variable_declarator"
                && let Some(name_node) = child.named_child("name")
                && name_node.kind() == "identifier"
                && let Ok(name) = name_node.text()
            {
                locals.insert(name.to_string());
            }
            if matches!(
                child.kind(),
                "arrow_function" | "function" | "function_expression" | "function_declaration"
            ) {
                for name in Self::callback_params(&child) {
                    locals.insert(name);
                }
            }
            Self::collect_locals(&child, locals);
        }
    }

    fn assignment_target_base_name(node: &Node<JavaScript>) -> Option<String> {
        match node.kind() {
            "identifier" => node.text().ok().map(|s| s.to_string()),
            "member_expression" | "subscript_expression" => {
                let object = node.child(0).or_else(|| node.named_child("object"))?;
                Self::assignment_target_base_name(&object)
            }
            _ => None,
        }
    }

    fn mutates_free_var(
        node: &Node<JavaScript>,
        locals: &std::collections::HashSet<String>,
    ) -> bool {
        for child in node.iter() {
            match child.kind() {
                "assignment_expression" | "augmented_assignment_expression" => {
                    if let Some(left) = child.child(0)
                        && let Some(name) = Self::assignment_target_base_name(&left)
                        && !locals.contains(&name)
                    {
                        return true;
                    }
                }
                "update_expression" => {
                    if let Some(operand) = child.iter().find(|c| c.kind() == "identifier")
                        && let Ok(name) = operand.text()
                        && !locals.contains(name)
                    {
                        return true;
                    }
                }
                _ => {}
            }
            if Self::mutates_free_var(&child, locals) {
                return true;
            }
        }
        false
    }

    /// Resolves the callback argument node into either a user function to inline evaluate or a recognized native conversion (`Number`, `String`)
    fn resolve_callback(cb: &Node<JavaScript>) -> Option<Callback> {
        if cb.kind() == "identifier" {
            return match cb.text().ok()? {
                "Number" => Some(Callback::Native(NativeConversion::Number)),
                "String" => Some(Callback::Native(NativeConversion::String)),
                _ => None,
            };
        }

        if matches!(
            cb.kind(),
            "arrow_function" | "function_expression" | "function"
        ) {
            let params = Self::callback_params(cb);
            let body = cb.named_child("body")?;

            let mut locals: std::collections::HashSet<String> = params.iter().cloned().collect();
            Self::collect_locals(&body, &mut locals);
            if Self::mutates_free_var(&body, &locals) {
                return None;
            }

            let body_source = Self::callback_body_source(&body)?;
            let free_var_bindings = Self::collect_free_var_bindings(&body, &params);
            return Some(Callback::UserFunction {
                params,
                body_source,
                free_var_bindings,
            });
        }

        None
    }

    fn apply_callback(
        kind: MapFilterKind,
        input: &[JavaScript],
        cb: &Node<JavaScript>,
    ) -> Option<Vec<JavaScript>> {
        if input.is_empty() {
            return Some(vec![]);
        }

        let callback = Self::resolve_callback(cb)?;

        let mut out = Vec::with_capacity(input.len());
        for (index, element) in input.iter().enumerate() {
            let value = match &callback {
                Callback::Native(conversion) => conversion.apply(element),
                Callback::UserFunction {
                    params,
                    body_source,
                    free_var_bindings,
                } => Self::evaluate(
                    free_var_bindings,
                    params,
                    body_source,
                    element,
                    index,
                    input,
                )?,
            };
            match kind {
                MapFilterKind::Map => out.push(value),
                MapFilterKind::Filter => {
                    if value.as_bool() {
                        out.push(element.clone());
                    }
                }
            }
        }

        Some(out)
    }
}

impl<'a> RuleMut<'a> for ArrayMapFilter {
    type Language = JavaScript;

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
        if view.kind() != "call_expression" {
            return Ok(());
        }

        let Some(callee) = view.named_child("function").or_else(|| view.child(0)) else {
            return Ok(());
        };
        let Some(method) = method_name(&callee) else {
            return Ok(());
        };

        let kind = match method.as_str() {
            "map" => MapFilterKind::Map,
            "filter" => MapFilterKind::Filter,
            _ => return Ok(()),
        };

        let Some(object) = callee.child(0).or_else(|| callee.named_child("object")) else {
            return Ok(());
        };
        let Some(Array(input)) = object.data() else {
            return Ok(());
        };

        let args = view.named_child("arguments");
        let positional_args = get_positional_arguments(args);
        let Some(cb) = positional_args.into_iter().next() else {
            return Ok(());
        };
        // callbacks are sometimes redundantly parenthesized, e.g. `.map(((e) => e))`
        let cb = Self::unwrap_parens(cb);

        if !matches!(
            cb.kind(),
            "arrow_function" | "function_expression" | "function" | "identifier"
        ) {
            return Ok(());
        }

        if MAP_FILTER_DEPTH.get() >= MAX_MAP_FILTER_DEPTH {
            warn!("ArrayMapFilter: max recursion depth reached, leaving call unresolved");
            return Ok(());
        }

        MAP_FILTER_DEPTH.with(|d| d.set(d.get() + 1));
        let result = Self::apply_callback(kind, input, &cb);
        MAP_FILTER_DEPTH.with(|d| d.set(d.get() - 1));

        if let Some(values) = result {
            trace!(
                "ArrayMapFilter: reducing '{}'.{}(...) to {} elements",
                Array(input.clone()),
                method,
                values.len()
            );
            node.reduce(Array(values));
        }

        Ok(())
    }
}

const MAX_FOR_ITERATIONS: usize = 10_000;
pub const MAX_FOR_DEPTH: usize = 3;

thread_local! {
    static FOR_DEPTH: Cell<usize> = const { Cell::new(0) };
    static FOR_LOOP_ENABLED: Cell<bool> = const { Cell::new(false) };
    static FOR_LOOP_RESULTS: RefCell<HashMap<usize, Vec<(String, JavaScript)>>> =
        RefCell::new(HashMap::new());
}

pub fn is_for_loop_enabled() -> bool {
    FOR_LOOP_ENABLED.get()
}

pub fn for_depth_get() -> usize {
    FOR_DEPTH.get()
}

pub fn for_depth_inc() {
    FOR_DEPTH.with(|d| d.set(d.get() + 1));
}

pub fn for_depth_dec() {
    FOR_DEPTH.with(|d| d.set(d.get() - 1));
}

pub fn clear_for_loop_results() {
    FOR_LOOP_RESULTS.with(|m| m.borrow_mut().clear());
}

pub fn take_for_loop_result(node_id: usize) -> Option<Vec<(String, JavaScript)>> {
    FOR_LOOP_RESULTS.with(|m| m.borrow_mut().remove(&node_id))
}

pub fn store_for_loop_result(node_id: usize, vars: Vec<(String, JavaScript)>) {
    FOR_LOOP_RESULTS.with(|m| m.borrow_mut().insert(node_id, vars));
}

pub fn body_has_bail_node<T>(node: &Node<T>) -> bool {
    for child in node.iter() {
        match child.kind() {
            "break_statement" | "continue_statement" | "return_statement" | "throw_statement"
            | "for_statement" | "while_statement" | "do_statement" | "for_in_statement"
            | "for_of_statement" => return true,
            "function_declaration"
            | "function"
            | "arrow_function"
            | "method_definition"
            | "generator_function_declaration"
            | "generator_function" => {}
            _ => {
                if body_has_bail_node(&child) {
                    return true;
                }
            }
        }
    }
    false
}

pub fn extract_for_parts(node: &Node<JavaScript>) -> Option<(String, String, String, String)> {
    let init = node.named_child("initializer")?;
    let condition = node.named_child("condition")?;
    let update = node.named_child("increment")?;
    let body = node.named_child("body")?;
    Some((
        init.text().ok()?.to_string(),
        condition.text().ok()?.to_string(),
        update.text().ok()?.to_string(),
        body.text().ok()?.to_string(),
    ))
}

pub fn vars_to_source(vars: &[(String, JavaScript)]) -> String {
    vars.iter()
        .map(|(name, val)| format!("var {name} = {val};\n"))
        .collect()
}

fn collect_declarator_names_from_root<T>(node: &Node<T>, names: &mut HashSet<String>) {
    for child in node.iter() {
        if child.kind() == "variable_declarator"
            && let Some(name_node) = child.named_child("name")
            && name_node.kind() == "identifier"
            && let Ok(name) = name_node.text()
            && !name.starts_with("__v_")
        {
            names.insert(name.to_string());
        }
        collect_declarator_names_from_root(&child, names);
    }
}

fn collect_declared_names(src: &str) -> Vec<String> {
    let Ok(tree) = build_javascript_tree(src) else {
        return vec![];
    };
    let Ok(root) = tree.root() else {
        return vec![];
    };
    let mut names = HashSet::new();
    collect_declarator_names_from_root(&root, &mut names);
    names.into_iter().collect()
}

pub fn run_program_extract_state(
    src: &str,
    tracked: &[String],
) -> Option<(Vec<(String, JavaScript)>, JavaScript)> {
    let snapshot_suffix: String = tracked
        .iter()
        .map(|name| format!("var __v_{name} = {name};\n"))
        .collect();
    let full_src = format!("{src}\n{snapshot_suffix}");

    let mut tree = build_javascript_tree(&full_src).ok()?;
    tree.apply_mut_with_strategy(
        &mut JavaScriptRuleSet::new(RuleSetBuilderType::WithoutRules(vec![])),
        JavaScriptStrategy,
    )
    .ok()?;

    let root = tree.root().ok()?;
    let mut state: HashMap<String, JavaScript> = HashMap::new();
    let mut condition: Option<JavaScript> = None;

    for stmt in root.iter() {
        match stmt.kind() {
            "variable_declaration" | "lexical_declaration" => {
                for child in stmt.iter() {
                    if child.kind() == "variable_declarator"
                        && let Some(name_node) = child.named_child("name")
                        && name_node.kind() == "identifier"
                        && let Ok(name) = name_node.text()
                        && let Some(real_name) = name.strip_prefix("__v_")
                        && let Some(val_node) = child.named_child("value")
                        && let Some(data) = val_node.data()
                    {
                        state.insert(real_name.to_string(), data.clone());
                    }
                }
            }
            "expression_statement" => {
                for child in stmt.iter() {
                    if child.kind() != ";" {
                        if let Some(data) = child.data() {
                            condition = Some(data.clone());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let condition = condition?;
    Some((state.into_iter().collect(), condition))
}

pub fn simulate_for_loop(
    scope_snapshot: &str,
    init_src: &str,
    condition_src: &str,
    update_src: &str,
    body_src: &str,
) -> Option<Vec<(String, JavaScript)>> {
    let init_program = format!("{scope_snapshot}{init_src}\n({condition_src})");
    let init_var_names = collect_declared_names(init_src);

    // discover all variable names present after init by running the init program
    let mut tracked: Vec<String> = {
        let mut tree = build_javascript_tree(&init_program).ok()?;
        tree.apply_mut_with_strategy(
            &mut JavaScriptRuleSet::new(RuleSetBuilderType::WithoutRules(vec![])),
            JavaScriptStrategy,
        )
        .ok()?;
        let root = tree.root().ok()?;
        let mut names = HashSet::new();
        collect_declarator_names_from_root(&root, &mut names);
        names.into_iter().collect()
    };
    for name in &init_var_names {
        if !tracked.contains(name) {
            tracked.push(name.clone());
        }
    }

    let (mut state, first_condition) = run_program_extract_state(&init_program, &tracked)?;

    match first_condition {
        Raw(Bool(false)) => return Some(state),
        Raw(Bool(true)) => {}
        _ => return None,
    }

    for _ in 0..MAX_FOR_ITERATIONS {
        let state_src = vars_to_source(&state);
        let iter_program = format!("{state_src}{body_src}\n{update_src};\n({condition_src})");
        let (new_state, condition) = run_program_extract_state(&iter_program, &tracked)?;
        state = new_state;

        match condition {
            Raw(Bool(false)) => return Some(state),
            Raw(Bool(true)) => continue,
            _ => return None,
        }
    }

    None
}

/// Simulates deterministic `for` loops (no break/continue/return/throw, no nested loops)
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::r#loop::ForLoop;
/// use minusone::js::var::Var;
/// use minusone::js::linter::Linter;
/// use minusone::js::strategy::JavaScriptStrategy;
/// use minusone::js::JavaScriptRuleSet;
/// use minusone::rule::RuleSetBuilderType;
///
/// let mut tree = build_javascript_tree(
///     "var s = ''; for(var i = 0; i < 3; i++) { s = s + String.fromCharCode(65 + i); } var out = s;"
/// ).unwrap();
/// tree.apply_mut_with_strategy(
///     &mut JavaScriptRuleSet::new(RuleSetBuilderType::WithoutRules(vec![])),
///     JavaScriptStrategy,
/// ).unwrap();
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
/// assert!(linter.output.contains("var out = 'ABC';"));
/// ```
#[derive(Default)]
pub struct ForLoop;

impl<'a> RuleMut<'a> for ForLoop {
    type Language = JavaScript;

    fn enter(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        if node.view().kind() == "program" {
            FOR_LOOP_ENABLED.set(true);
        }
        Ok(())
    }

    fn leave(
        &mut self,
        _node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests_js_loop {
    use super::*;
    use crate::js::array::ParseArray;
    use crate::js::build_javascript_tree;
    use crate::js::forward::Forward;
    use crate::js::functions::function::ParseFunction;
    use crate::js::integer::ParseInt;
    use crate::js::linter::Linter;
    use crate::js::strategy::JavaScriptStrategy;
    use crate::js::string::ParseString;
    use crate::js::var::Var;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                ParseString::default(),
                ParseArray::default(),
                ParseFunction::default(),
                Forward::default(),
                Var::default(),
                ArrayMapFilter::default(),
            ),
            JavaScriptStrategy,
        )
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    fn deobfuscate_for_loop(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut_with_strategy(
            &mut JavaScriptRuleSet::new(RuleSetBuilderType::WithoutRules(vec![])),
            JavaScriptStrategy,
        )
        .unwrap();
        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_map_arrow_expression_body() {
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4].map((e) => e.toString());"),
            "var x = ['0', '1', '2', '3', '4'];"
        );
    }

    #[test]
    fn test_map_bare_arrow_param() {
        assert_eq!(
            deobfuscate("var x = [0, 1, 2].map(e => e + 1);"),
            "var x = [1, 2, 3];"
        );
    }

    #[test]
    fn test_map_block_body_single_return() {
        assert_eq!(
            deobfuscate("var x = [1, 2, 3].map(function (e) { return e * 2; });"),
            "var x = [2, 4, 6];"
        );
    }

    #[test]
    fn test_filter_keeps_original_elements() {
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4].filter((e) => e == 1);"),
            "var x = [1];"
        );
    }

    #[test]
    fn test_filter_empty_array() {
        assert_eq!(
            deobfuscate("var x = [].filter(e => e == 1);"),
            "var x = [];"
        );
    }

    #[test]
    fn test_map_does_not_mutate_original_array() {
        assert_eq!(
            deobfuscate("var x = [0, 1, 2]; var y = x.map(e => e.toString()); var z = x;"),
            "var x = [0, 1, 2]; var y = ['0', '1', '2']; var z = [0, 1, 2];"
        );
    }

    #[test]
    fn test_map_zero_arg_callback() {
        assert_eq!(
            deobfuscate("var x = [1, 2, 3].map(() => 9);"),
            "var x = [9, 9, 9];"
        );
    }

    #[test]
    fn test_map_unresolvable_callback_leaves_call_untouched() {
        assert_eq!(
            deobfuscate("var x = [1, 2, 3].map(e => foo(e));"),
            "var x = [1, 2, 3].map(e => foo(e));"
        );
    }

    #[test]
    fn test_map_multiple_returns_leaves_call_untouched() {
        assert_eq!(
            deobfuscate("var x = [1, 2, 3].map(function (e) { if (e) { return 1; } return 2; });"),
            "var x = [1, 2, 3].map(function (e) { if (e) { return 1; } return 2; });"
        );
    }

    #[test]
    fn test_map_chained_with_filter() {
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3].map(e => e + 1).filter(e => e == 2);"),
            "var x = [2];"
        );
    }

    #[test]
    fn test_map_number_callback() {
        assert_eq!(
            deobfuscate("var x = ['3', '1', '2'].map(Number);"),
            "var x = [3, 1, 2];"
        );
    }

    #[test]
    fn test_map_string_callback() {
        assert_eq!(
            deobfuscate("var x = [1, 2, 0].map(String);"),
            "var x = ['1', '2', '0'];"
        );
    }

    #[test]
    fn test_map_string_callback_flattens_arrays() {
        assert_eq!(
            deobfuscate("var x = [[1, 2], [3]].map(String);"),
            "var x = ['1,2', '3'];"
        );
    }

    #[test]
    fn test_filter_number_callback_drops_falsy() {
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, '', '3'].filter(Number);"),
            "var x = [1, 2, '3'];"
        );
    }

    #[test]
    fn test_map_unknown_identifier_callback_leaves_call_untouched() {
        assert_eq!(
            deobfuscate("var x = [1, 2, 3].map(foo);"),
            "var x = [1, 2, 3].map(foo);"
        );
    }

    #[test]
    fn test_map_redundant_parens_around_callback() {
        assert_eq!(
            deobfuscate("var x = [1, 2, 3].map(((e) => e + 1));"),
            "var x = [2, 3, 4];"
        );
    }

    #[test]
    fn test_map_closure_over_outer_const() {
        assert_eq!(
            deobfuscate("var offset = 10; var x = [1, 2, 3].map(e => e + offset);"),
            "var offset = 10; var x = [11, 12, 13];"
        );
    }

    #[test]
    fn test_map_mutating_outer_var_leaves_call_untouched() {
        assert_eq!(
            deobfuscate("var x = 0; [1, 2, 3].map(() => x = x + 1);"),
            "var x = 0; [1, 2, 3].map(() => x = x + 1);"
        );
    }

    #[test]
    fn test_counter_propagated_after_loop() {
        let out = deobfuscate_for_loop("for(var i = 0; i < 5; i++) {} var x = i;");
        assert!(out.ends_with("var x = 5;"));
    }

    #[test]
    fn test_string_accumulation() {
        let out = deobfuscate_for_loop(
            "var s = ''; for(var i = 0; i < 3; i++) { s = s + String.fromCharCode(65 + i); } var out = s;",
        );
        assert!(out.ends_with("var out = 'ABC';"));
    }

    #[test]
    fn test_loop_never_runs() {
        let out = deobfuscate_for_loop(
            "var s = 'x'; for(var i = 0; i < 0; i++) { s = s + 'y'; } var out = s;",
        );
        assert!(out.ends_with("var out = 'x';"));
    }

    #[test]
    fn test_bail_on_break() {
        let src = "var s = ''; for(var i = 0; i < 3; i++) { if(i == 1) break; s = s + 'x'; } var out = s;";
        assert!(deobfuscate_for_loop(src).ends_with("var out = s;"));
    }

    #[test]
    fn test_bail_on_return() {
        let src = "function f() { var s = ''; for(var i = 0; i < 3; i++) { if(i == 1) return s; s = s + 'x'; } }";
        assert!(deobfuscate_for_loop(src).contains("for"));
    }

    #[test]
    fn test_non_deterministic_condition_bails() {
        let src = "var s = ''; for(var i = 0; i < unknown; i++) { s = s + 'x'; } var out = s;";
        assert!(deobfuscate_for_loop(src).ends_with("var out = s;"));
    }

    #[test]
    fn test_free_var_from_outer_scope() {
        let out = deobfuscate_for_loop(
            "var key = 10; var s = ''; for(var i = 0; i < 3; i++) { s = s + String.fromCharCode(55 + i + key); } var out = s;",
        );
        assert!(out.ends_with("var out = 'ABC';"));
    }
}
