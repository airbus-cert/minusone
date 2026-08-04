use crate::error::MinusOneResult;
use crate::js::JavaScript::*;
use crate::js::Value::{Bool, Str};
use crate::js::array::flatten_array;
use crate::js::build_javascript_tree;
use crate::js::strategy::JavaScriptStrategy;
use crate::js::subprogram::{build_and_reduce, enter_map_filter, take_seed_result, with_seed};
use crate::js::utils::{get_positional_arguments, is_write_target, method_name};
use crate::js::{JavaScript, JavaScriptRuleSet};
use crate::rule::{RuleMut, RuleSetBuilderType};
use crate::tree::{ControlFlow, Node, NodeMut};
use log::{trace, warn};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

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

        let Some(_depth) = enter_map_filter() else {
            warn!("ArrayMapFilter: max recursion depth reached, leaving call unresolved");
            return Ok(());
        };

        let result = Self::apply_callback(kind, input, &cb);

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

const MAX_FOR_ITERATIONS: usize = 20_000;

thread_local! {
    static FOR_LOOP_ENABLED: Cell<bool> = const { Cell::new(false) };
    static INSIDE_SIMULATED_FOR: Cell<bool> = const { Cell::new(false) };
    static FOR_LOOP_RESULTS: RefCell<HashMap<usize, Vec<(String, JavaScript)>>> =
        RefCell::new(HashMap::new());
    // arrays hoisted out of the current loop simulation, read in O(1) by name/index, I had to use
    // this trick because most of the time, loops iterate over arrays
    static LOOP_INVARIANTS: RefCell<HashMap<String, JavaScript>> = RefCell::new(HashMap::new());
}

fn swap_loop_invariants(next: HashMap<String, JavaScript>) -> HashMap<String, JavaScript> {
    LOOP_INVARIANTS.with(|c| c.replace(next))
}

pub fn loop_invariant_array_len(name: &str) -> Option<usize> {
    LOOP_INVARIANTS.with(|c| match c.borrow().get(name) {
        Some(Array(a)) => Some(a.len()),
        _ => None,
    })
}

pub fn loop_invariant_array_index(name: &str, index: usize) -> Option<JavaScript> {
    LOOP_INVARIANTS.with(|c| match c.borrow().get(name) {
        Some(Array(a)) => Some(a.get(index).cloned().unwrap_or(Undefined)),
        _ => None,
    })
}

pub fn loop_invariant_get(name: &str) -> Option<JavaScript> {
    LOOP_INVARIANTS.with(|c| c.borrow().get(name).cloned())
}

pub fn is_for_loop_enabled() -> bool {
    FOR_LOOP_ENABLED.get()
}

pub fn is_inside_simulated_for() -> bool {
    INSIDE_SIMULATED_FOR.get()
}

pub fn set_inside_simulated_for(v: bool) {
    INSIDE_SIMULATED_FOR.set(v);
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
        match child.kind() {
            "variable_declarator" => {
                if let Some(name_node) = child.named_child("name")
                    && name_node.kind() == "identifier"
                    && let Ok(name) = name_node.text()
                    && !name.starts_with("__v_")
                {
                    names.insert(name.to_string());
                }
            }
            "assignment_expression" | "augmented_assignment_expression" => {
                if let Some(target) = child.child(0)
                    && target.kind() == "identifier"
                    && let Ok(name) = target.text()
                    && !name.starts_with("__v_")
                {
                    names.insert(name.to_string());
                }
            }
            "update_expression" => {
                if let Some(target) = child.iter().find(|c| c.kind() == "identifier")
                    && let Ok(name) = target.text()
                    && !name.starts_with("__v_")
                {
                    names.insert(name.to_string());
                }
            }
            _ => {}
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

    let tree = build_and_reduce(&full_src)?;
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

fn array_is_hoistable(name: &str, src: &str) -> bool {
    let Ok(tree) = build_javascript_tree(src) else {
        return false;
    };
    let Ok(root) = tree.root() else {
        return false;
    };
    all_uses_readonly_index(&root, name)
}

fn all_uses_readonly_index(node: &Node<JavaScript>, name: &str) -> bool {
    for child in node.iter() {
        if child.kind() == "identifier"
            && child.text().map(|t| t == name).unwrap_or(false)
            && !is_readonly_index_use(&child)
        {
            return false;
        }
        if !all_uses_readonly_index(&child, name) {
            return false;
        }
    }
    true
}

fn is_readonly_index_use(occ: &Node<JavaScript>) -> bool {
    if is_write_target(occ) {
        return false;
    }
    let Some(parent) = occ.parent() else {
        return false;
    };
    let is_object = parent
        .named_child("object")
        .or_else(|| parent.child(0))
        .map(|o| o.id() == occ.id())
        .unwrap_or(false);
    match parent.kind() {
        "subscript_expression" => is_object,
        "member_expression" => {
            is_object
                && parent
                    .named_child("property")
                    .and_then(|p| p.text().ok().map(|s| s == "length"))
                    .unwrap_or(false)
        }
        _ => false,
    }
}

fn last_expression_data(root: &Node<JavaScript>) -> Option<JavaScript> {
    let mut data = None;
    for stmt in root.iter() {
        if stmt.kind() == "expression_statement" {
            data = stmt
                .iter()
                .find(|c| c.kind() != ";")
                .and_then(|c| c.data().cloned());
        }
    }
    data
}

fn run_seeded_program(
    program_src: &str,
    seed: HashMap<String, JavaScript>,
) -> Option<(HashMap<String, JavaScript>, JavaScript)> {
    with_seed(seed, || {
        let tree = build_and_reduce(program_src)?;
        let condition = last_expression_data(&tree.root().ok()?)?;
        Some((take_seed_result()?, condition))
    })
}

fn run_for_iterations(
    state: HashMap<String, JavaScript>,
    body_src: &str,
    update_src: &str,
    condition_src: &str,
) -> Option<Vec<(String, JavaScript)>> {
    let program = format!("{body_src}\n{update_src};\n({condition_src})");
    let mut state = state;
    for _ in 0..MAX_FOR_ITERATIONS {
        let (new_state, condition) = run_seeded_program(&program, state)?;
        state = new_state;

        match condition {
            Raw(Bool(false)) => return Some(state.into_iter().collect()),
            Raw(Bool(true)) => continue,
            _ => return None,
        }
    }

    None
}

pub fn simulate_for_loop(
    scope_snapshot: &str,
    init_src: &str,
    condition_src: &str,
    update_src: &str,
    body_src: &str,
) -> Option<Vec<(String, JavaScript)>> {
    let init_program = format!("{scope_snapshot}{init_src};\n({condition_src})");
    let init_var_names = collect_declared_names(init_src);

    // discover all variable names present after init by running the init program
    let mut tracked: Vec<String> = {
        let tree = build_and_reduce(&init_program)?;
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

    let (initial_state, first_condition) = run_program_extract_state(&init_program, &tracked)?;

    match first_condition {
        Raw(Bool(false)) => return Some(initial_state),
        Raw(Bool(true)) => {}
        _ => return None,
    }

    let all_uses = format!("{body_src}\n{update_src}\n({condition_src})");
    let mut invariants: HashMap<String, JavaScript> = HashMap::new();
    let mut state: HashMap<String, JavaScript> = HashMap::new();
    for (name, value) in initial_state {
        if matches!(value, Array(_)) && array_is_hoistable(&name, &all_uses) {
            invariants.insert(name, value);
        } else {
            state.insert(name, value);
        }
    }

    let previous = swap_loop_invariants(invariants);
    let outcome = run_for_iterations(state, body_src, update_src, condition_src);
    let invariants = swap_loop_invariants(previous);

    outcome.map(|mut final_state| {
        final_state.extend(invariants);
        final_state
    })
}

fn run_seeded_body(
    program_src: &str,
    seed: HashMap<String, JavaScript>,
) -> Option<HashMap<String, JavaScript>> {
    with_seed(seed, || {
        build_and_reduce(program_src)?;
        take_seed_result()
    })
}

pub fn simulate_for_in_loop(
    loop_var: &str,
    iter_values: &[JavaScript],
    body_src: &str,
    initial_state: HashMap<String, JavaScript>,
) -> Option<Vec<(String, JavaScript)>> {
    let mut invariants: HashMap<String, JavaScript> = HashMap::new();
    let mut state: HashMap<String, JavaScript> = HashMap::new();
    for (name, value) in &initial_state {
        if name == loop_var {
            continue;
        }
        if matches!(value, Array(_) | Object { .. }) && array_is_hoistable(name, body_src) {
            invariants.insert(name.clone(), value.clone());
        } else {
            state.insert(name.clone(), value.clone());
        }
    }

    let previous = swap_loop_invariants(invariants);
    let outcome = (|| {
        for value in iter_values {
            let mut seed = state.clone();
            seed.insert(loop_var.to_string(), value.clone());
            state = run_seeded_body(body_src, seed)?;
        }
        Some(())
    })();
    swap_loop_invariants(previous);
    outcome?;

    Some(
        state
            .into_iter()
            .filter(|(name, value)| initial_state.get(name) != Some(value))
            .collect(),
    )
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
