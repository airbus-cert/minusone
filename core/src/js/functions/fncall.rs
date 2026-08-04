use crate::engine::{CleanEngine, DeobfuscationBackend};
use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::Undefined;
use crate::js::Value::{Num, Str};
use crate::js::backend::JavaScriptBackend;
use crate::js::functions::function::function_value_from_node;
use crate::js::subprogram::{build_and_reduce, enter_fncall, with_seed};
use crate::js::utils::{get_positional_arguments, js_to_string_value, method_name};
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, Node, NodeMut};
use log::trace;
use std::collections::{HashMap, HashSet};

/// Tracks function declarations with predictable return values
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::forward::Forward;
/// use minusone::js::integer::ParseInt;
/// use minusone::js::string::ParseString;
/// use minusone::js::var::Var;
/// use minusone::js::functions::fncall::FnCall;
/// use minusone::js::linter::Linter;
/// use minusone::js::strategy::JavaScriptStrategy;
///
/// let mut tree = build_javascript_tree("function test() { return 'hello'; } console.log(test());").unwrap();
/// tree.apply_mut_with_strategy(
///     &mut (ParseString::default(), ParseInt::default(), Forward::default(), Var::default(), FnCall::default()),
///     JavaScriptStrategy::default(),
/// ).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "function test() { return 'hello'; } console.log('hello');");
/// ```
#[derive(Default)]
pub struct FnCall {
    functions: HashMap<String, JavaScript>,
    vars: HashMap<String, JavaScript>,
    object_fields: HashMap<(String, String), JavaScript>,
    var_shapes: HashMap<String, FunctionShape>,
    object_field_shapes: HashMap<(String, String), FunctionShape>,
    shapes_by_source: HashMap<String, FunctionShape>,
    // Top-level function declarations as (name, raw source), can hoist + resolve nested calls.
    // Only the ones a body can actually reach are re-emitted into its sub-program.
    fn_decls: Vec<(String, String)>,
}

#[derive(Clone)]
struct FunctionShape {
    params: Vec<String>,
    body_inner: String,
}

impl FnCall {
    fn reduce_array_subscript(node: &mut NodeMut<JavaScript>) {
        let view = node.view();
        if let (Some(array_node), Some(index_node)) = (view.child(0), view.child(2)) {
            if let (Some(JavaScript::Array(arr)), Some(JavaScript::Raw(Num(index)))) =
                (array_node.data(), index_node.data())
                && *index >= 0.0
            {
                let idx = *index as usize;
                if idx < arr.len() {
                    node.reduce(arr[idx].clone());
                    return;
                }
            }

            if let (Some(JavaScript::Array(arr)), Some(JavaScript::Raw(Str(index_str)))) =
                (array_node.data(), index_node.data())
                && let Ok(idx) = index_str.parse::<usize>()
                && idx < arr.len()
            {
                node.reduce(arr[idx].clone());
            }
        }
    }

    fn find_single_return_value(body: &Node<JavaScript>) -> Option<JavaScript> {
        let mut return_value: Option<JavaScript> = None;
        let mut found_count = 0;

        Self::walk_for_returns(body, &mut return_value, &mut found_count);

        if found_count == 1 { return_value } else { None }
    }

    fn walk_for_returns<'a>(
        node: &Node<'a, JavaScript>,
        return_value: &mut Option<JavaScript>,
        found_count: &mut usize,
    ) {
        for child in node.iter() {
            match child.kind() {
                "return_statement" => {
                    *found_count += 1;
                    if *found_count == 1 {
                        // first named child after "return"
                        for i in 0..child.child_count() {
                            if let Some(c) = child.child(i)
                                && c.kind() != "return"
                                && c.kind() != ";"
                            {
                                if let Some(data) = c.data() {
                                    *return_value = Some(data.clone());
                                }
                                break;
                            }
                        }
                    }
                }
                "function_declaration"
                | "function"
                | "arrow_function"
                | "generator_function_declaration"
                | "generator_function" => {
                    // skip nested fn having their own returns
                }
                // skip loops and conditionals
                "if_statement" | "while_statement" | "do_statement" | "for_statement"
                | "for_in_statement" | "switch_statement" | "try_statement" => {
                    let mut inner_count = 0;
                    Self::count_returns_in_subtree(&child, &mut inner_count);
                    if inner_count > 0 {
                        *found_count += inner_count;
                    }
                }
                _ => {
                    Self::walk_for_returns(&child, return_value, found_count);
                }
            }
        }
    }

    fn count_returns_in_subtree<'a>(node: &Node<'a, JavaScript>, count: &mut usize) {
        for child in node.iter() {
            match child.kind() {
                "return_statement" => {
                    *count += 1;
                }
                "function_declaration"
                | "function"
                | "arrow_function"
                | "generator_function_declaration"
                | "generator_function" => {
                    // skip nested fn
                }
                _ => {
                    Self::count_returns_in_subtree(&child, count);
                }
            }
        }
    }

    fn extract_member_access(node: &Node<JavaScript>) -> Option<(String, String)> {
        if node.kind() != "member_expression" {
            return None;
        }

        let object = node.named_child("object")?;
        let property = node.named_child("property")?;
        if object.kind() != "identifier" {
            return None;
        }

        let base = object.text().ok()?.to_string();
        let key = property.text().ok()?.to_string();
        Some((base, key))
    }

    fn function_return_from_value(value: &JavaScript) -> Option<JavaScript> {
        match value {
            JavaScript::Function {
                return_value: Some(return_value),
                ..
            } => Some(return_value.as_ref().clone()),
            _ => None,
        }
    }

    fn collect_identifiers(node: &Node<JavaScript>, out: &mut Vec<String>) {
        for child in node.iter() {
            if child.kind() == "identifier"
                && let Ok(name) = child.text()
            {
                out.push(name.to_string());
            }
            Self::collect_identifiers(&child, out);
        }
    }

    fn extract_params(function_node: &Node<JavaScript>) -> Vec<String> {
        if let Some(params_node) = function_node.named_child("parameters") {
            let mut params = Vec::new();
            Self::collect_identifiers(&params_node, &mut params);
            if !params.is_empty() {
                return params;
            }
        }

        vec![]
    }

    fn function_shape_from_node(function_node: &Node<JavaScript>) -> Option<FunctionShape> {
        if !matches!(
            function_node.kind(),
            "function"
                | "function_expression"
                | "function_declaration"
                | "arrow_function"
                | "generator_function"
                | "generator_function_declaration"
        ) {
            return None;
        }

        let params = Self::extract_params(function_node);
        let body = function_node.named_child("body")?;
        let body_text = body.text().ok()?;

        let body_inner = if body.kind() == "statement_block" {
            let trimmed = body_text.trim();
            let stripped = trimmed
                .strip_prefix('{')
                .unwrap_or(trimmed)
                .strip_suffix('}')
                .unwrap_or(trimmed);
            stripped.to_string()
        } else {
            format!("return ({});", body_text)
        };

        Some(FunctionShape { params, body_inner })
    }

    fn extract_positional_call_args(call_node: &Node<JavaScript>) -> Option<Vec<JavaScript>> {
        let arguments_node = call_node.named_child("arguments")?;
        let mut args = Vec::new();
        for child in arguments_node.iter() {
            if matches!(child.kind(), "(" | ")" | ",") {
                continue;
            }
            args.push(child.data()?.clone());
        }
        Some(args)
    }

    fn extract_top_level_return_value(root: &Node<JavaScript>) -> Option<JavaScript> {
        if Self::has_nested_return_in_control_flow(root) {
            return None;
        }

        for child in root.iter() {
            if child.kind() == "return_statement" {
                for c in child.iter() {
                    if matches!(c.kind(), "return" | ";") {
                        continue;
                    }
                    return c.data().cloned();
                }
                return None;
            }
        }
        None
    }

    fn has_nested_return_in_control_flow(node: &Node<JavaScript>) -> bool {
        for child in node.iter() {
            match child.kind() {
                "if_statement" | "while_statement" | "do_statement" | "for_statement"
                | "for_in_statement" | "switch_statement" | "try_statement" => {
                    let mut count = 0;
                    Self::count_returns_in_subtree(&child, &mut count);
                    if count > 0 {
                        return true;
                    }
                }
                "function_declaration"
                | "function"
                | "function_expression"
                | "arrow_function"
                | "generator_function"
                | "generator_function_declaration" => {
                    // nested fn has its own returns; ignore
                }
                _ => {
                    if Self::has_nested_return_in_control_flow(&child) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Maximal identifier-like tokens of a JS source.
    ///
    /// This over-approximates: a name that only appears inside a string literal
    /// is reported as referenced. It never under-approximates, because a real
    /// reference is always a maximal identifier token in the source. Using it to
    /// decide which declarations a body needs can therefore only keep too many,
    /// never drop one that was needed.
    fn identifier_tokens(src: &str) -> HashSet<String> {
        let mut out = HashSet::new();
        let mut current = String::new();
        for c in src.chars() {
            if c.is_alphanumeric() || c == '_' || c == '$' {
                current.push(c);
            } else if !current.is_empty() {
                out.insert(std::mem::take(&mut current));
            }
        }
        if !current.is_empty() {
            out.insert(current);
        }
        out
    }

    /// Prelude restricted to the declarations `body` can reach, transitively.
    ///
    /// Re-emitting every top-level declaration into every sub-program made the
    /// synthesised source grow with the whole file instead of with the function
    /// being resolved, which is quadratic over a file's call sites. A body
    /// typically reaches none or a couple of other functions.
    // pub(crate) so `js::tests::fncall_tests` can assert on the filtering,
    // which is invisible from the deobfuscated output.
    pub(crate) fn prelude_for(decls: &[(String, String)], body: &str) -> String {
        if decls.is_empty() {
            return String::new();
        }

        let mut needed: HashSet<&str> = HashSet::new();
        let mut frontier = Self::identifier_tokens(body);

        while !frontier.is_empty() {
            let mut next = HashSet::new();
            for (name, src) in decls {
                if needed.contains(name.as_str()) || !frontier.contains(name.as_str()) {
                    continue;
                }
                needed.insert(name.as_str());
                next.extend(Self::identifier_tokens(src));
            }
            frontier = next;
        }

        // Source order, so hoisting stays identical to the original program.
        let mut prelude = String::new();
        for (name, src) in decls {
            if needed.contains(name.as_str()) {
                prelude.push_str(src);
                prelude.push('\n');
            }
        }
        prelude
    }

    fn evaluate_shape_via_subtree(
        shape: &FunctionShape,
        args: &[JavaScript],
        decls: &[(String, String)],
    ) -> Option<JavaScript> {
        let mut program = Self::prelude_for(decls, &shape.body_inner);
        if !program.is_empty() {
            program.push('\n');
        }
        program.push_str(&shape.body_inner);
        program.push('\n');

        // Arguments are handed over through the seed channel rather than
        // rendered back to source. A value that has no literal syntax (a Buffer,
        // an iterator, an object with a custom toString) used to abort the whole
        // resolution; now it just travels as-is.
        //
        // Extra arguments are dropped instead of aborting: JS only binds the
        // first `params.len()` of them (issue #193). `arguments` is not modelled,
        // so nothing else can observe the ones we drop.
        let seed = shape
            .params
            .iter()
            .enumerate()
            .map(|(i, param)| {
                (
                    param.clone(),
                    args.get(i).cloned().unwrap_or(JavaScript::Undefined),
                )
            })
            .collect();

        with_seed(seed, || Self::run_subtree_pipeline(&program))
    }

    fn stabilise_via_minusone(program: &str) -> Option<String> {
        let cleaned = JavaScriptBackend::remove_extra(program, false).ok()?;
        const SUBTREE_FIXPOINT_ITER_CAP: usize = 8;
        let mut current = cleaned;
        for _ in 0..SUBTREE_FIXPOINT_ITER_CAP {
            let tree = build_and_reduce(&current)?;

            let mut linter = crate::js::linter::Linter::default();
            tree.apply(&mut linter).ok()?;
            let linted = linter.output;

            let post_cleaned = match CleanEngine::<JavaScriptBackend>::from_source(&linted) {
                Ok(mut e) => e.clean(false).unwrap_or(linted),
                Err(_) => return None,
            };

            if post_cleaned == current {
                break;
            }
            current = post_cleaned;
        }

        Some(current)
    }

    fn run_subtree_pipeline(program: &str) -> Option<JavaScript> {
        let stable = Self::stabilise_via_minusone(program)?;
        // Final pass to attach data on the stabilised tree.
        let tree = build_and_reduce(&stable)?;
        let root = tree.root().ok()?;
        Self::extract_top_level_return_value(&root)
    }

    fn shape_from_value(&self, value: &JavaScript) -> Option<FunctionShape> {
        match value {
            JavaScript::Function { source, .. } => self.shapes_by_source.get(source).cloned(),
            _ => None,
        }
    }

    fn find_program_node<'a>(node: &Node<'a, JavaScript>) -> Option<Node<'a, JavaScript>> {
        let mut current = node.parent();
        while let Some(parent) = current {
            if parent.kind() == "program" {
                return Some(parent);
            }
            current = parent.parent();
        }
        None
    }

    fn build_shapes_until<'a>(
        node: &Node<'a, JavaScript>,
        stop_abs: usize,
        var_shapes: &mut HashMap<String, FunctionShape>,
        object_field_shapes: &mut HashMap<(String, String), FunctionShape>,
        aliases: &mut HashMap<String, String>,
    ) {
        if node.start_abs() >= stop_abs {
            return;
        }

        match node.kind() {
            "variable_declarator" => {
                if let Some(name_node) = node.named_child("name")
                    && name_node.kind() == "identifier"
                    && let Ok(name) = name_node.text()
                    && let Some(value_node) = node.named_child("value").or_else(|| node.child(2))
                {
                    if let Some(shape) = Self::function_shape_from_node(&value_node) {
                        var_shapes.insert(name.to_string(), shape);
                    } else if value_node.kind() == "identifier"
                        && let Ok(rhs_name) = value_node.text()
                    {
                        aliases.insert(name.to_string(), rhs_name.to_string());
                        if let Some(shape) = var_shapes.get(rhs_name).cloned() {
                            var_shapes.insert(name.to_string(), shape);
                        }
                    }
                }
            }
            "function_declaration" | "generator_function_declaration" => {
                if let Some(name_node) = node.named_child("name")
                    && name_node.kind() == "identifier"
                    && let Ok(name) = name_node.text()
                    && let Some(shape) = Self::function_shape_from_node(node)
                {
                    var_shapes.insert(name.to_string(), shape);
                }
            }
            "assignment_expression" => {
                if let (Some(left), Some(right)) = (node.child(0), node.child(2)) {
                    if left.kind() == "identifier"
                        && let Ok(var_name) = left.text()
                    {
                        if let Some(shape) = Self::function_shape_from_node(&right) {
                            var_shapes.insert(var_name.to_string(), shape);
                        } else if right.kind() == "identifier"
                            && let Ok(rhs_name) = right.text()
                        {
                            aliases.insert(var_name.to_string(), rhs_name.to_string());
                            if let Some(shape) = var_shapes.get(rhs_name).cloned() {
                                var_shapes.insert(var_name.to_string(), shape);
                            }
                        }
                    } else if let Some((base, key)) = Self::extract_member_access(&left) {
                        if let Some(shape) = Self::function_shape_from_node(&right) {
                            object_field_shapes.insert((base, key), shape);
                        } else if right.kind() == "identifier"
                            && let Ok(rhs_name) = right.text()
                            && let Some(shape) = var_shapes.get(rhs_name).cloned()
                        {
                            object_field_shapes.insert((base, key), shape);
                        }
                    }
                }
            }
            _ => {}
        }

        for child in node.iter() {
            Self::build_shapes_until(&child, stop_abs, var_shapes, object_field_shapes, aliases);
        }
    }

    fn resolve_shape_with_aliases(
        name: &str,
        var_shapes: &HashMap<String, FunctionShape>,
        aliases: &HashMap<String, String>,
    ) -> Option<FunctionShape> {
        if let Some(shape) = var_shapes.get(name) {
            return Some(shape.clone());
        }

        // `var f = g; var g = h;` chains: bound so a cyclic alias table cannot
        // spin here. Unrelated to the sub-pipeline depth cap.
        const MAX_ALIAS_HOPS: usize = 2;

        let mut current = name;
        for _ in 0..MAX_ALIAS_HOPS {
            let next = aliases.get(current)?;
            if let Some(shape) = var_shapes.get(next) {
                return Some(shape.clone());
            }
            current = next;
        }

        None
    }

    fn collect_fn_decls(program: &Node<JavaScript>) -> Vec<(String, String)> {
        let mut decls = Vec::new();
        for child in program.iter() {
            if matches!(
                child.kind(),
                "function_declaration" | "generator_function_declaration"
            ) && let Some(name_node) = child.named_child("name")
                && name_node.kind() == "identifier"
                && let Ok(name) = name_node.text()
                && let Ok(src) = child.text()
            {
                decls.push((name.to_string(), src.to_string()));
            }
        }
        decls
    }

    fn resolve_member_call_semantic_fallback<'a>(
        call_node: &Node<'a, JavaScript>,
        base: &str,
        key: &str,
    ) -> Option<JavaScript> {
        let program = Self::find_program_node(call_node)?;
        let mut var_shapes = HashMap::new();
        let mut object_field_shapes = HashMap::new();
        let mut aliases = HashMap::new();

        Self::build_shapes_until(
            &program,
            call_node.start_abs(),
            &mut var_shapes,
            &mut object_field_shapes,
            &mut aliases,
        );

        let shape = object_field_shapes.get(&(base.to_string(), key.to_string()))?;
        let args = Self::extract_positional_call_args(call_node)?;
        let decls = Self::collect_fn_decls(&program);
        Self::evaluate_shape_via_subtree(shape, &args, &decls)
    }

    fn resolve_identifier_call_semantic_fallback<'a>(
        call_node: &Node<'a, JavaScript>,
        fn_name: &str,
    ) -> Option<JavaScript> {
        let program = Self::find_program_node(call_node)?;
        let mut var_shapes = HashMap::new();
        let mut object_field_shapes = HashMap::new();
        let mut aliases = HashMap::new();

        Self::build_shapes_until(
            &program,
            call_node.start_abs(),
            &mut var_shapes,
            &mut object_field_shapes,
            &mut aliases,
        );

        if !aliases.contains_key(fn_name) {
            return None;
        }

        let shape = Self::resolve_shape_with_aliases(fn_name, &var_shapes, &aliases)?;
        let args = Self::extract_positional_call_args(call_node)?;
        let decls = Self::collect_fn_decls(&program);
        Self::evaluate_shape_via_subtree(&shape, &args, &decls)
    }

    fn is_eval_callee(callee: &Node<JavaScript>) -> bool {
        callee.kind() == "identifier" && callee.text().map(|t| t == "eval").unwrap_or(false)
    }

    fn eval_source_from_argument(arg: &Node<JavaScript>) -> Option<String> {
        if let Some(JavaScript::Raw(Str(s))) = arg.data() {
            return Some(s.clone());
        }

        let text = arg.text().ok()?.trim();
        if text.len() < 2 {
            return None;
        }

        let bytes = text.as_bytes();
        let first = bytes[0];
        let last = bytes[text.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return Some(text[1..text.len() - 1].to_string());
        }

        None
    }

    fn last_statement_value(program: &Node<JavaScript>) -> Option<JavaScript> {
        let mut last_stmt: Option<Node<JavaScript>> = None;
        for child in program.iter() {
            match child.kind() {
                "expression_statement" | "variable_declaration" | "lexical_declaration"
                | "return_statement" => {
                    last_stmt = Some(child);
                }
                _ => {}
            }
        }
        let stmt = last_stmt?;

        match stmt.kind() {
            "expression_statement" => {
                for child in stmt.iter() {
                    if child.kind() != ";"
                        && let Some(data) = child.data()
                    {
                        return Some(data.clone());
                    }
                }
                None
            }
            "variable_declaration" | "lexical_declaration" => None,
            "return_statement" => {
                for i in 0..stmt.child_count() {
                    if let Some(c) = stmt.child(i)
                        && c.kind() != "return"
                        && c.kind() != ";"
                    {
                        return c.data().cloned();
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn evaluate_eval_source(source: &str) -> Option<JavaScript> {
        let tree = build_and_reduce(source)?;
        let root = tree.root().ok()?;
        Self::last_statement_value(&root)
    }

    fn try_resolve_eval(&mut self, call_node: &Node<JavaScript>) -> Option<JavaScript> {
        let callee = call_node
            .named_child("function")
            .or_else(|| call_node.child(0))?;
        if !Self::is_eval_callee(&callee) {
            return None;
        }

        let positional = get_positional_arguments(call_node.named_child("arguments"));
        if positional.is_empty() {
            return None;
        }

        let source = Self::eval_source_from_argument(&positional[0])?;

        let _guard = enter_fncall()?;
        Self::evaluate_eval_source(&source)
    }

    fn hoist_function_declaration(
        node: &Node<JavaScript>,
        var_shapes: &mut HashMap<String, FunctionShape>,
    ) {
        if !matches!(
            node.kind(),
            "function_declaration" | "generator_function_declaration"
        ) {
            return;
        }
        let Some(name_node) = node.named_child("name") else {
            return;
        };
        if name_node.kind() != "identifier" {
            return;
        }
        let Ok(name) = name_node.text() else {
            return;
        };
        if let Some(shape) = Self::function_shape_from_node(node) {
            var_shapes.insert(name.to_string(), shape);
        }
    }

    fn try_eval_shape(
        &mut self,
        shape: &FunctionShape,
        view: &Node<JavaScript>,
    ) -> Option<JavaScript> {
        let _guard = enter_fncall()?;
        let args = Self::extract_positional_call_args(view)?;
        Self::evaluate_shape_via_subtree(shape, &args, &self.fn_decls)
    }

    fn try_resolve_identifier_call(
        &mut self,
        view: &Node<JavaScript>,
        fn_name: &str,
    ) -> Option<JavaScript> {
        let _guard = enter_fncall()?;
        Self::resolve_identifier_call_semantic_fallback(view, fn_name)
    }

    fn try_resolve_member_call(
        &mut self,
        view: &Node<JavaScript>,
        base: &str,
        key: &str,
    ) -> Option<JavaScript> {
        let _guard = enter_fncall()?;
        Self::resolve_member_call_semantic_fallback(view, base, key)
    }
}

impl<'a> RuleMut<'a> for FnCall {
    type Language = JavaScript;

    fn enter(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        let view = node.view();
        if view.kind() == "program" {
            self.functions.clear();
            self.vars.clear();
            self.object_fields.clear();
            self.var_shapes.clear();
            self.object_field_shapes.clear();
            self.shapes_by_source.clear();
            self.fn_decls.clear();

            for child in view.iter() {
                // Hoist top-level function_declarations so forward calls resolve.
                Self::hoist_function_declaration(&child, &mut self.var_shapes);
            }
            // Keep the declaration sources so a sub-program can re-emit the
            // subset its body actually reaches.
            self.fn_decls = Self::collect_fn_decls(&view);
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
            "subscript_expression" => {
                Self::reduce_array_subscript(node);
            }
            "function" | "function_expression" | "arrow_function" | "generator_function" => {
                if let Some(shape) = Self::function_shape_from_node(&view)
                    && let Ok(source) = view.text()
                {
                    self.shapes_by_source.insert(source.to_string(), shape);
                }
            }
            "variable_declarator" => {
                if let Some(name_node) = view.named_child("name")
                    && name_node.kind() == "identifier"
                {
                    let name = name_node.text()?.to_string();
                    if let Some(value_node) = view.named_child("value").or_else(|| view.child(2)) {
                        let value = value_node
                            .data()
                            .cloned()
                            .or_else(|| function_value_from_node(&value_node));

                        if let Some(shape) = Self::function_shape_from_node(&value_node) {
                            self.var_shapes.insert(name.clone(), shape);
                        } else if value_node.kind() == "identifier"
                            && let Ok(rhs_name) = value_node.text()
                            && let Some(shape) = self.var_shapes.get(rhs_name).cloned()
                        {
                            self.var_shapes.insert(name.clone(), shape);
                        } else if let Some(value) = value.as_ref()
                            && let Some(shape) = self.shape_from_value(value)
                        {
                            self.var_shapes.insert(name.clone(), shape);
                        }

                        if let Some(value @ JavaScript::Function { .. }) = value {
                            self.vars.insert(name, value);
                        }
                    }
                }
            }
            "function_declaration" => {
                if let Some(name_node) = view.named_child("name")
                    && name_node.kind() == "identifier"
                {
                    let fn_name = name_node.text()?.to_string();

                    if let Some(shape) = Self::function_shape_from_node(&view) {
                        self.var_shapes.insert(fn_name.clone(), shape);
                    }

                    if let Some(body) = view.named_child("body")
                        && let Some(return_data) = Self::find_single_return_value(&body)
                    {
                        trace!(
                            "FnCall (L): Recorded function '{}' with return value: {:?}",
                            fn_name, return_data
                        );
                        self.functions.insert(fn_name, return_data);
                    }
                }
            }
            "assignment_expression" => {
                if let (Some(left), Some(right)) = (view.child(0), view.child(2)) {
                    if left.kind() == "identifier" {
                        let var_name = left.text()?.to_string();
                        let value = right
                            .data()
                            .cloned()
                            .or_else(|| function_value_from_node(&right))
                            .or_else(|| {
                                if right.kind() == "identifier" {
                                    right
                                        .text()
                                        .ok()
                                        .and_then(|name| self.vars.get(name).cloned())
                                } else {
                                    None
                                }
                            });

                        if let Some(shape) = Self::function_shape_from_node(&right) {
                            self.var_shapes.insert(var_name.clone(), shape);
                        } else if right.kind() == "identifier"
                            && let Some(name) = right.text().ok()
                            && let Some(shape) = self.var_shapes.get(name).cloned()
                        {
                            self.var_shapes.insert(var_name.clone(), shape);
                        } else if let Some(value) = value.as_ref()
                            && let Some(shape) = self.shape_from_value(value)
                        {
                            self.var_shapes.insert(var_name.clone(), shape);
                        }

                        if let Some(value @ JavaScript::Function { .. }) = value {
                            self.vars.insert(var_name, value);
                        }
                    } else if let Some((base, key)) = Self::extract_member_access(&left) {
                        let value = right
                            .data()
                            .cloned()
                            .or_else(|| function_value_from_node(&right))
                            .or_else(|| {
                                if right.kind() == "identifier" {
                                    right
                                        .text()
                                        .ok()
                                        .and_then(|name| self.vars.get(name).cloned())
                                } else {
                                    None
                                }
                            });

                        if let Some(shape) = Self::function_shape_from_node(&right) {
                            self.object_field_shapes
                                .insert((base.clone(), key.clone()), shape);
                        } else if right.kind() == "identifier"
                            && let Some(name) = right.text().ok()
                            && let Some(shape) = self.var_shapes.get(name).cloned()
                        {
                            self.object_field_shapes
                                .insert((base.clone(), key.clone()), shape);
                        } else if let Some(value) = value.as_ref()
                            && let Some(shape) = self.shape_from_value(value)
                        {
                            self.object_field_shapes
                                .insert((base.clone(), key.clone()), shape);
                        }

                        if let Some(value @ JavaScript::Function { .. }) = value {
                            self.object_fields.insert((base, key), value);
                        }
                    }
                }
            }
            "call_expression" => {
                // check known fn
                if let Some(func_node) = view.named_child("function").or_else(|| view.child(0)) {
                    if Self::is_eval_callee(&func_node)
                        && let Some(value) = self.try_resolve_eval(&view)
                    {
                        trace!("FnCall (L): Resolving eval call to: {:?}", value);
                        node.reduce(value);
                        return Ok(());
                    }

                    let is_tostring_method = method_name(&func_node).as_deref() == Some("toString");
                    let has_args =
                        !get_positional_arguments(view.named_child("arguments")).is_empty();
                    let tostring_on_buffer = is_tostring_method
                        && func_node
                            .child(0)
                            .or_else(|| func_node.named_child("object"))
                            .map(|obj| matches!(obj.data(), Some(JavaScript::Buffer(_))))
                            .unwrap_or(false);
                    // keep Buffer.toString and argument-aware toString in dedicated rules
                    if is_tostring_method && (tostring_on_buffer || has_args) {
                        return Ok(());
                    }

                    if func_node.kind() == "identifier" {
                        let fn_name = func_node.text()?.to_string();

                        if let Some(return_data) = self.functions.get(&fn_name).cloned() {
                            trace!(
                                "FnCall (L): Resolving call to '{}' with: {:?}",
                                fn_name, return_data
                            );
                            node.reduce(return_data);
                        } else if let Some(shape) = self.var_shapes.get(&fn_name).cloned()
                            && let Some(value) = self.try_eval_shape(&shape, &view)
                        {
                            trace!(
                                "FnCall (L): Resolving call to semantic variable function with: {:?}",
                                value
                            );
                            node.reduce(value);
                        } else if let Some(value) = self.vars.get(&fn_name).cloned()
                            && let Some(return_value) = Self::function_return_from_value(&value)
                        {
                            trace!(
                                "FnCall (L): Resolving call to variable function value with: {:?}",
                                return_value
                            );
                            node.reduce(return_value);
                        } else if let Some(value) =
                            self.try_resolve_identifier_call(&view, &fn_name)
                        {
                            trace!(
                                "FnCall (L): Resolving call to semantic identifier fallback with: {:?}",
                                value
                            );
                            node.reduce(value);
                        } else if let Some(JavaScript::Function {
                            return_value: Some(return_value),
                            ..
                        }) = func_node.data()
                        {
                            trace!(
                                "FnCall (L): Resolving call to identifier function value with: {:?}",
                                return_value
                            );
                            node.reduce(return_value.as_ref().clone());
                        }
                    } else if method_name(&func_node).as_deref() == Some("fontcolor")
                        && let Some(object_node) = func_node.named_child("object")
                        && let Some(JavaScript::Raw(Str(base))) = object_node.data()
                    {
                        let color = js_to_string_value(
                            get_positional_arguments(view.named_child("arguments"))
                                .first()
                                .and_then(|arg| arg.data())
                                .unwrap_or(&Undefined),
                        )
                        .replace('"', "&quot;");

                        trace!(
                            "FnCall (L): Resolving fontcolor call on {:?} with color {:?}",
                            base, color
                        );
                        node.reduce(JavaScript::Raw(Str(format!(
                            "<font color=\"{color}\">{base}</font>"
                        ))));
                    } else if let Some(return_value) =
                        func_node.data().and_then(Self::function_return_from_value)
                    {
                        trace!(
                            "FnCall (L): Resolving call to function value with: {:?}",
                            return_value
                        );
                        node.reduce(return_value);
                    } else if let Some((base, key)) = Self::extract_member_access(&func_node)
                        && let Some(shape) = self
                            .object_field_shapes
                            .get(&(base.clone(), key.clone()))
                            .cloned()
                        && let Some(value) = self.try_eval_shape(&shape, &view)
                    {
                        trace!(
                            "FnCall (L): Resolving call to semantic object field function with: {:?}",
                            value
                        );
                        node.reduce(value);
                    } else if let Some((base, key)) = Self::extract_member_access(&func_node)
                        && let Some(value) =
                            self.object_fields.get(&(base.clone(), key.clone())).cloned()
                        && let Some(return_value) = Self::function_return_from_value(&value)
                    {
                        trace!(
                            "FnCall (L): Resolving call to object field function with: {:?}",
                            return_value
                        );
                        node.reduce(return_value);
                    } else if let Some((base, key)) = Self::extract_member_access(&func_node)
                        && let Some(value) = self.try_resolve_member_call(&view, &base, &key)
                    {
                        trace!(
                            "FnCall (L): Resolving call to semantic fallback object field function with: {:?}",
                            value
                        );
                        node.reduce(value);
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}
