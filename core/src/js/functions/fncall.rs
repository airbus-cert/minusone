use crate::engine::{CleanEngine, DeobfuscationBackend};
use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScriptRuleSet;
use crate::js::Value::{Bool, Num, Str};
use crate::js::backend::JavaScriptBackend;
use crate::js::build_javascript_tree;
use crate::js::forward::Forward;
use crate::js::functions::function::function_value_from_node;
use crate::js::integer::{AddInt, ParseInt};
use crate::js::recursion::{RecursionExt, RecursionTracker, global_unbump, try_global_bump};
use crate::js::specials::ParseSpecials;
use crate::js::strategy::JavaScriptStrategy;
use crate::js::string::{Concat, ParseString};
use crate::js::utils::{get_positional_arguments, method_name};
use crate::rule::{RuleMut, RuleSetBuilderType};
use crate::tree::{ControlFlow, Node, NodeMut};
use log::trace;
use std::collections::HashMap;

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
    recursion: RecursionTracker,
    // All top-level function declarations as raw source, prepended to every
    // synthesised sub-program so sub-FnCall can hoist + resolve nested calls.
    fn_decl_prelude: String,
}

#[derive(Clone)]
struct FunctionShape {
    params: Vec<String>,
    // Body source text, with the outer `{` / `}` already stripped so we can
    // splice it directly into a synthesised sub-program (after binding params
    // as top-level vars).
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

        // For statement-block bodies (`function f() { ... }`), strip the
        // outer braces so the inner statements can be spliced directly into
        // a parent program. For arrow expression bodies (`x => x.foo()`),
        // wrap the expression in a `return` so it has the same semantics
        // when spliced.
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

    // -----------------------------------------------------------------
    //  Subtree-based evaluation: synthesise `var p = V; <body>` and run
    //  the full JavaScript ruleset on a fresh tree so method calls,
    //  array operations, regex, etc. — anything minusone already handles
    //  — work inside function bodies without re-implementing them.
    // -----------------------------------------------------------------

    fn js_value_to_source(value: &JavaScript) -> Option<String> {
        match value {
            JavaScript::Raw(Num(n)) => {
                if n.is_nan() {
                    Some("NaN".to_string())
                } else if n.is_infinite() {
                    Some(if *n > 0.0 { "Infinity" } else { "-Infinity" }.to_string())
                } else if *n == n.trunc() && n.abs() < 1e16 {
                    Some(format!("{}", *n as i64))
                } else {
                    Some(format!("{}", n))
                }
            }
            JavaScript::Raw(Str(s)) => Some(format!("'{}'", Self::escape_js_string(s))),
            JavaScript::Raw(Bool(b)) => Some(b.to_string()),
            JavaScript::Array(items) => {
                let parts: Option<Vec<String>> =
                    items.iter().map(Self::js_value_to_source).collect();
                Some(format!("[{}]", parts?.join(",")))
            }
            JavaScript::Function { source, .. } => Some(format!("({})", source)),
            _ => None,
        }
    }

    fn escape_js_string(s: &str) -> String {
        let mut out = String::with_capacity(s.len() + 2);
        for c in s.chars() {
            match c {
                '\\' => out.push_str("\\\\"),
                '\'' => out.push_str("\\'"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                '\0' => out.push_str("\\0"),
                c if (c as u32) < 0x20 => {
                    out.push_str(&format!("\\x{:02x}", c as u32));
                }
                c => out.push(c),
            }
        }
        out
    }

    /// Strict positional extraction: returns None as soon as any arg has no
    /// inferred data, so we never silently shift later args into earlier
    /// parameter slots when running on a sub-tree.
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

    /// Walk the deobfuscated sub-program, find the FIRST top-level
    /// `return_statement` (the one that survived CleanEngine's
    /// constant-`if` collapse), and read the data attached to its
    /// expression. Returns are only honoured at program scope: anything
    /// still nested inside a surviving conditional means we couldn't
    /// statically pick a branch and we must bail.
    fn extract_top_level_return_value(root: &Node<JavaScript>) -> Option<JavaScript> {
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

    /// Synthesise `<prelude>; var p1 = V1; ...; <body>` and run the full
    /// JavaScript backend on it. The body is included verbatim — its
    /// `return` statements stay where they are. After the backend reduces
    /// the program, exactly one top-level `return` survives if the body is
    /// reducible, and its expression carries the value.
    fn evaluate_shape_via_subtree(
        shape: &FunctionShape,
        args: &[JavaScript],
        prelude: &str,
    ) -> Option<JavaScript> {
        if args.len() > shape.params.len() {
            return None;
        }

        let mut program = String::new();
        if !prelude.is_empty() {
            program.push_str(prelude);
            program.push('\n');
        }

        for (i, param) in shape.params.iter().enumerate() {
            let value_src = match args.get(i) {
                Some(value) => Self::js_value_to_source(value)?,
                None => "undefined".to_string(),
            };
            program.push_str(&format!("var {} = {};\n", param, value_src));
        }

        program.push_str(&shape.body_inner);
        program.push('\n');

        Self::run_subtree_pipeline(&program)
    }

    fn run_subtree_pipeline(program: &str) -> Option<JavaScript> {
        // 1. Pre-process: InlineIife, ExpandAugmentedAssignment, ReduceSequence,
        //    UnusedVar+RemoveUnused (which does the constant-condition `if` collapsing).
        let cleaned = JavaScriptBackend::remove_extra(program).ok()?;

        // 2. Apply ruleset, lint, post-clean. Iterate the (rules + clean)
        //    pair until the source stabilises - one cycle isn't enough when
        //    a constant `if` collapse on iteration N exposes new constant
        //    folds for iteration N+1 (e.g. recursive call resolution).
        let mut current = cleaned;
        for _ in 0..8 {
            let mut tree = build_javascript_tree(&current).ok()?;
            tree.apply_mut_with_strategy(
                &mut JavaScriptRuleSet::new(RuleSetBuilderType::WithoutRules(vec![])),
                JavaScriptStrategy,
            )
            .ok()?;

            let mut linter = crate::js::linter::Linter::default();
            tree.apply(&mut linter).ok()?;
            let linted = linter.output;

            let post_cleaned = match CleanEngine::<JavaScriptBackend>::from_source(&linted) {
                Ok(mut e) => e.clean().unwrap_or(linted),
                Err(_) => return None,
            };

            if post_cleaned == current {
                break;
            }
            current = post_cleaned;
        }

        // 3. Final pass to attach data on the stabilised tree.
        let mut tree = build_javascript_tree(&current).ok()?;
        tree.apply_mut_with_strategy(
            &mut JavaScriptRuleSet::new(RuleSetBuilderType::WithoutRules(vec![])),
            JavaScriptStrategy,
        )
        .ok()?;
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

        let mut current = name;
        for _ in 0..crate::js::recursion::DEFAULT_MAX_RECURSION_DEPTH {
            let next = aliases.get(current)?;
            if let Some(shape) = var_shapes.get(next) {
                return Some(shape.clone());
            }
            current = next;
        }

        None
    }

    fn build_program_prelude(program: &Node<JavaScript>) -> String {
        let mut prelude = String::new();
        for child in program.iter() {
            if matches!(
                child.kind(),
                "function_declaration" | "generator_function_declaration"
            ) && let Ok(src) = child.text()
            {
                prelude.push_str(src);
                prelude.push('\n');
            }
        }
        prelude
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
        let prelude = Self::build_program_prelude(&program);
        Self::evaluate_shape_via_subtree(shape, &args, &prelude)
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
        let prelude = Self::build_program_prelude(&program);
        Self::evaluate_shape_via_subtree(&shape, &args, &prelude)
    }

    fn parse_simple_return_literal(source: &str) -> Option<JavaScript> {
        let return_idx = source.find("return")?;
        let after_return = &source[return_idx + "return".len()..];
        let end_idx = after_return.find(';').or_else(|| after_return.find('}'))?;
        let literal = after_return[..end_idx].trim();

        if literal.starts_with('"') && literal.ends_with('"') && literal.len() >= 2 {
            return Some(JavaScript::Raw(Str(
                literal[1..literal.len() - 1].to_string()
            )));
        }

        if literal.starts_with('\'') && literal.ends_with('\'') && literal.len() >= 2 {
            return Some(JavaScript::Raw(Str(
                literal[1..literal.len() - 1].to_string()
            )));
        }

        literal.parse::<f64>().ok().map(|n| JavaScript::Raw(Num(n)))
    }

    fn extract_return_expr(source: &str) -> Option<String> {
        let return_idx = source.find("return")?;
        let after_return = &source[return_idx + "return".len()..];
        let end_idx = after_return.find(';').or_else(|| after_return.find('}'))?;
        Some(after_return[..end_idx].trim().to_string())
    }

    fn extract_first_param_name(source: &str) -> Option<String> {
        let open = source.find('(')?;
        let close = source[open + 1..].find(')')? + open + 1;
        let first = source[open + 1..close].split(',').next()?.trim();
        if first.is_empty() {
            None
        } else {
            Some(first.to_string())
        }
    }

    fn extract_first_numeric_arg(call: &Node<JavaScript>) -> Option<f64> {
        if let Some(args) = call.named_child("arguments") {
            for child in args.iter() {
                if let Some(JavaScript::Raw(Num(n))) = child.data() {
                    return Some(*n);
                }
            }
        }

        let text = call.text().ok()?;
        let open = text.find('(')?;
        let close = text[open + 1..].find(')')? + open + 1;
        let first = text[open + 1..close].split(',').next()?.trim();
        first.parse::<f64>().ok()
    }

    fn eval_simple_numeric_expr(expr: &str, param: &str, arg: f64) -> Option<f64> {
        let expr = expr.replace(' ', "");
        if expr == param {
            return Some(arg);
        }

        for op in ['+', '-', '*', '/'] {
            if let Some(idx) = expr.find(op) {
                let left = &expr[..idx];
                let right = &expr[idx + 1..];

                if left == param {
                    let rhs = right.parse::<f64>().ok()?;
                    return Some(match op {
                        '+' => arg + rhs,
                        '-' => arg - rhs,
                        '*' => arg * rhs,
                        '/' => arg / rhs,
                        _ => return None,
                    });
                }

                if right == param {
                    let lhs = left.parse::<f64>().ok()?;
                    return Some(match op {
                        '+' => lhs + arg,
                        '-' => lhs - arg,
                        '*' => lhs * arg,
                        '/' => lhs / arg,
                        _ => return None,
                    });
                }
            }
        }

        None
    }

    fn parse_simple_return_with_arg(source: &str, call: &Node<JavaScript>) -> Option<JavaScript> {
        let param = Self::extract_first_param_name(source)?;
        let arg = Self::extract_first_numeric_arg(call)?;
        let expr = Self::extract_return_expr(source)?;
        let value = Self::eval_simple_numeric_expr(&expr, &param, arg)?;
        Some(JavaScript::Raw(Num(value)))
    }

    fn find_initializer_source(prefix: &str, name: &str) -> Option<String> {
        fn extract_function_source(rhs: &str) -> Option<String> {
            let trimmed = rhs.trim_start();
            if !(trimmed.starts_with("function")
                || trimmed.starts_with("async function")
                || trimmed.contains("=>"))
            {
                return None;
            }

            if let Some(open_idx) = trimmed.find('{') {
                let mut depth = 0usize;
                for (i, ch) in trimmed.char_indices().skip(open_idx) {
                    match ch {
                        '{' => depth += 1,
                        '}' => {
                            depth = depth.saturating_sub(1);
                            if depth == 0 {
                                return Some(trimmed[..=i].trim().to_string());
                            }
                        }
                        _ => {}
                    }
                }
            }

            let end = trimmed.find(';').unwrap_or(trimmed.len());
            Some(trimmed[..end].trim().to_string())
        }

        for kw in ["let", "var", "const"] {
            let pattern = format!("{kw} {name} =");
            if let Some(idx) = prefix.rfind(&pattern) {
                let rhs = &prefix[idx + pattern.len()..];
                return extract_function_source(rhs);
            }
        }
        None
    }

    fn resolve_member_call_from_source(call: &Node<JavaScript>) -> Option<JavaScript> {
        let callee = call.named_child("function").or_else(|| call.child(0))?;
        if callee.kind() != "member_expression" {
            return None;
        }

        let object = callee.named_child("object")?;
        let property = callee.named_child("property")?;
        if object.kind() != "identifier" {
            return None;
        }

        let base = object.text().ok()?.to_string();
        let key = property.text().ok()?.to_string();
        let program = Self::find_program_node(call)?;
        let source = program.text().ok()?;
        let prefix_end = call.start_abs().saturating_sub(program.start_abs());
        let prefix = &source[..prefix_end];

        let assign_pattern = format!("{base}.{key} =");
        let assign_idx = prefix.rfind(&assign_pattern)?;
        let rhs_text = {
            let rhs = &prefix[assign_idx + assign_pattern.len()..];
            let end = rhs.find(';')?;
            rhs[..end].trim().to_string()
        };

        let function_source = if rhs_text.starts_with("function") || rhs_text.contains("=>") {
            rhs_text
        } else {
            Self::find_initializer_source(prefix, &rhs_text)?
        };

        Self::parse_simple_return_with_arg(&function_source, call)
            .or_else(|| Self::parse_simple_return_literal(&function_source))
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
        let mut tree = build_javascript_tree(source).ok()?;

        // Literal-only pipeline; no FnCall/Var so eval can't trigger nested fn inlining.
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseString::default(),
            ParseSpecials::default(),
            crate::js::bool::ParseBool::default(),
            AddInt::default(),
            Concat::default(),
            Forward::default(),
        ))
        .ok()?;

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

        call_node
            .within_recursion(&mut self.recursion, |_| {
                Self::evaluate_eval_source(&source)
            })
            .flatten()
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
        if !try_global_bump() {
            return None;
        }
        let result = (|| -> Option<JavaScript> {
            let args = Self::extract_positional_call_args(view)?;
            Self::evaluate_shape_via_subtree(shape, &args, &self.fn_decl_prelude)
        })();
        global_unbump();
        result
    }

    fn try_resolve_identifier_call(
        &mut self,
        view: &Node<JavaScript>,
        fn_name: &str,
    ) -> Option<JavaScript> {
        if !try_global_bump() {
            return None;
        }
        let result = Self::resolve_identifier_call_semantic_fallback(view, fn_name);
        global_unbump();
        result
    }

    fn try_resolve_member_call(
        &mut self,
        view: &Node<JavaScript>,
        base: &str,
        key: &str,
    ) -> Option<JavaScript> {
        if !try_global_bump() {
            return None;
        }
        let result = Self::resolve_member_call_semantic_fallback(view, base, key);
        global_unbump();
        result
    }

    fn try_resolve_member_call_from_source(
        &mut self,
        view: &Node<JavaScript>,
    ) -> Option<JavaScript> {
        view.within_recursion(&mut self.recursion, |node| {
            Self::resolve_member_call_from_source(node)
        })
        .flatten()
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
            self.recursion.reset();
            self.fn_decl_prelude.clear();

            for child in view.iter() {
                // Hoist top-level function_declarations so forward calls resolve.
                Self::hoist_function_declaration(&child, &mut self.var_shapes);
                // Collect function declaration source for sub-program prelude.
                if matches!(
                    child.kind(),
                    "function_declaration" | "generator_function_declaration"
                ) && let Ok(src) = child.text()
                {
                    self.fn_decl_prelude.push_str(src);
                    self.fn_decl_prelude.push('\n');
                }
            }
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
                    } else if let Some(return_value) =
                        self.try_resolve_member_call_from_source(&view)
                    {
                        trace!(
                            "FnCall (L): Resolving call to object field function from source fallback with: {:?}",
                            return_value
                        );
                        node.reduce(return_value);
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
    use crate::js::functions::fncall::FnCall;
    use crate::js::functions::function::ParseFunction;
    use crate::js::integer::{AddInt, ParseInt};
    use crate::js::linter::Linter;
    use crate::js::objects::object::{ObjectField, ParseObject};
    use crate::js::strategy::JavaScriptStrategy;
    use crate::js::string::ParseString;
    use crate::js::var::Var;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                AddInt::default(),
                ParseString::default(),
                ParseFunction::default(),
                ParseObject::default(),
                Forward::default(),
                ObjectField::default(),
                Var::default(),
                FnCall::default(),
            ),
            JavaScriptStrategy::default(),
        )
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_fncall_simple_string_return() {
        assert_eq!(
            deobfuscate("function test() { return 'hello'; } console.log(test());"),
            "function test() { return 'hello'; } console.log('hello');"
        );
    }

    #[test]
    fn test_fncall_simple_int_return() {
        assert_eq!(
            deobfuscate("function getValue() { return 42; } var x = getValue();"),
            "function getValue() { return 42; } var x = 42;"
        );
    }

    #[test]
    fn test_fncall_with_var_inside() {
        assert_eq!(
            deobfuscate("function test() { var a = 'hello'; return a; } console.log(test());"),
            "function test() { var a = 'hello'; return 'hello'; } console.log('hello');"
        );
    }

    #[test]
    fn test_fncall_resolves_param_dependent_return() {
        assert_eq!(
            deobfuscate("function test(x) { return x; } console.log(test('hello'));"),
            "function test(x) { return x; } console.log('hello');"
        );
    }

    #[test]
    fn test_fncall_resolve_param_independent_return() {
        assert_eq!(
            deobfuscate("function test(x) { console.log(x); return 1; } var a = test(7);"),
            "function test(x) { console.log(x); return 1; } var a = 1;"
        );
    }

    #[test]
    fn test_fncall_resolve_with_args_when_return_is_constant() {
        assert_eq!(
            deobfuscate("function test() { return 'hello'; } console.log(test('unused'));"),
            "function test() { return 'hello'; } console.log('hello');"
        );
    }

    #[test]
    fn test_fncall_constant_conditional_resolves() {
        assert_eq!(
            deobfuscate(
                "function test() { if (true) { return 'a'; } return 'b'; } console.log(test());"
            ),
            "function test() { if (true) { return 'a'; } return 'b'; } console.log('a');"
        );
    }

    #[test]
    fn test_fncall_no_return_not_resolved() {
        assert_eq!(
            deobfuscate("function test() { var a = 1; } console.log(test());"),
            "function test() { var a = 1; } console.log(test());"
        );
    }

    #[test]
    fn test_fncall_nested_function_scope() {
        assert_eq!(
            deobfuscate(
                "function outer() { function inner() { return 'inner'; } return 'outer'; } console.log(outer());"
            ),
            "function outer() { function inner() { return 'inner'; } return 'outer'; } console.log('outer');"
        );
    }

    #[test]
    fn test_fncall_expression_return() {
        assert_eq!(
            deobfuscate("function test() { return 1 + 2; } console.log(test());"),
            "function test() { return 3; } console.log(3);"
        );
    }

    #[test]
    fn test_fncall_unknown_return_not_resolved() {
        assert_eq!(
            deobfuscate("function test() { return foo(); } console.log(test());"),
            "function test() { return foo(); } console.log(test());"
        );
    }

    #[test]
    fn test_fncall_object_stored_function_constant_return() {
        assert_eq!(
            deobfuscate(
                "let a = {}; let x = function (params) { return 0; } a.t = x; console.log(a.t());"
            ),
            "let a = {}; let x = function (params) { return 0; } a.t = x; console.log(0);"
        );
    }

    #[test]
    fn test_fncall_object_stored_function_param_dependent_return() {
        assert_eq!(
            deobfuscate(
                "let a = {}; let x = function (n) { return n+1; } a.t = x; console.log(a.t(1)); console.log(a.t(2));"
            ),
            "let a = {}; let x = function (n) { return n+1; } a.t = x; console.log(2); console.log(3);"
        );
    }

    #[test]
    fn test_fncall_self_redefining_function() {
        let output = deobfuscate(
            "function _0x45a5(){return(_0x45a5=function(){return'minusone'})()}console.log(_0x45a5());",
        );

        assert!(output.ends_with("console.log('minusone');"));
    }
}
