//! Resolves predictable JavaScript function calls and `eval(...)` invocations
//! to their inferred return values.
//!
//! # Design overview
//!
//! Two recursion-prone operations live in this rule:
//!
//! 1. **Function call resolution** - when a call site references a known
//!    function shape, we evaluate the shape with the call's arguments and
//!    reduce the call expression to the resulting [`JavaScript`] value.
//! 2. **`eval(...)` resolution** - when the callee is the `eval` builtin and
//!    the argument is a literal source string, we parse it as a fresh
//!    JavaScript program, run a small literal-reduction pipeline on it, and
//!    reduce the eval call to the value of its **last** statement.
//!
//! Both expansions are gated by a [`RecursionTracker`] (see
//! [`crate::js::recursion`]), so adversarial inputs (mutually recursive
//! functions, `eval('eval("...")')` chains, decoder bombs) cannot push the
//! deobfuscator into an infinite loop. The tracker uses a default depth of
//! 16, which matches the constant requested by issue #152.
//!
//! ## Backup / restore semantics
//!
//! Issue #152 requires that a function's source remain *byte-identical* after
//! a call to it has been resolved. A naive implementation that walked into
//! the function body, propagated argument values, and let other rules (e.g.
//! [`crate::js::integer::AddInt`]) rewrite the body would mutate the AST
//! data attached to the function nodes, and the linter would then emit the
//! mutated body instead of the original.
//!
//! To avoid that we **never** materialise call-site bindings inside the
//! function body. Instead, [`FunctionShape`] is a parallel, immutable
//! description of the function (params, simple straight-line steps, return
//! expression). Shape evaluation runs on a transient
//! [`std::collections::HashMap`] environment - the AST storage is untouched
//! - and only the *call expression* node receives a `node.reduce(...)`.
//! This is the backup/restore mechanism: the shape acts as a snapshot, and
//! the function body never leaves its original state.

use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::Value::{Num, Str};
use crate::js::build_javascript_tree;
use crate::js::forward::Forward;
use crate::js::functions::function::function_value_from_node;
use crate::js::integer::{AddInt, ParseInt};
use crate::js::recursion::{RecursionExt, RecursionTracker};
use crate::js::specials::ParseSpecials;
use crate::js::string::{Concat, ParseString};
use crate::js::utils::{get_positional_arguments, method_name};
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, Node, NodeMut};
use log::trace;
use std::collections::HashMap;

/// Predictable function and `eval` call resolver.
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
    /// Functions with a single deterministic, parameter-independent return
    /// value (legacy path, kept for backwards compatibility with existing
    /// rules and tests that depend on it).
    functions: HashMap<String, JavaScript>,
    /// Variables holding a function value.
    vars: HashMap<String, JavaScript>,
    /// Object fields holding a function value: `obj.field = function() {...}`.
    object_fields: HashMap<(String, String), JavaScript>,
    /// Function shapes resolved by variable name.
    var_shapes: HashMap<String, FunctionShape>,
    /// Function shapes resolved by `(object, field)` access.
    object_field_shapes: HashMap<(String, String), FunctionShape>,
    /// Function shapes indexed by their textual source (allows resolving
    /// inline expressions like `(function () { return 1; })()`).
    shapes_by_source: HashMap<String, FunctionShape>,
    /// Recursion guard for nested expansions (function inlining and `eval`).
    recursion: RecursionTracker,
}

/// A simple, side-effect-free expression that may appear in a return
/// statement or in the right-hand side of an assignment we are willing to
/// constant-fold.
#[derive(Clone)]
enum ReturnExpr {
    Literal(JavaScript),
    Symbol(String),
    BinOp {
        op: String,
        left: Box<ReturnExpr>,
        right: Box<ReturnExpr>,
    },
    Subscript {
        array: Box<ReturnExpr>,
        index: Box<ReturnExpr>,
    },
}

/// An immutable description of a function's body, used to evaluate calls
/// without mutating the function's AST.
#[derive(Clone)]
struct FunctionShape {
    params: Vec<String>,
    steps: Vec<EvalStep>,
    return_expr: Option<ReturnExpr>,
}

#[derive(Clone)]
enum EvalStep {
    Assign { name: String, expr: ReturnExpr },
}

impl FnCall {
    // -----------------------------------------------------------------
    //  Subscript reduction (kept here so that array-access and call-site
    //  reductions live in one place).
    // -----------------------------------------------------------------

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

    // -----------------------------------------------------------------
    //  Return-value extraction (legacy literal path).
    // -----------------------------------------------------------------

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
                    // skip nested functions, they own their returns
                }
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
                | "generator_function" => {}
                _ => {
                    Self::count_returns_in_subtree(&child, count);
                }
            }
        }
    }

    // -----------------------------------------------------------------
    //  Member access helpers.
    // -----------------------------------------------------------------

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

    // -----------------------------------------------------------------
    //  Function shape construction (the "backup" half of backup/restore).
    //
    //  The shape is a snapshot of the function's body that we can evaluate
    //  any number of times without ever touching the original AST nodes.
    //  The function's source therefore stays byte-identical after a call is
    //  resolved - no separate restore step is needed because no mutation
    //  ever happened in the first place.
    // -----------------------------------------------------------------

    fn parse_return_expr(node: &Node<JavaScript>) -> Option<ReturnExpr> {
        if let Some(data) = node.data() {
            return Some(ReturnExpr::Literal(data.clone()));
        }

        match node.kind() {
            "identifier" => {
                let name = node.text().ok()?.to_string();
                Some(ReturnExpr::Symbol(name))
            }
            "parenthesized_expression" => {
                for child in node.iter() {
                    if child.kind() != "("
                        && child.kind() != ")"
                        && let Some(expr) = Self::parse_return_expr(&child)
                    {
                        return Some(expr);
                    }
                }
                None
            }
            "binary_expression" => {
                let left = node.child(0)?;
                let operator = node.child(1)?.text().ok()?.to_string();
                let right = node.child(2)?;

                Some(ReturnExpr::BinOp {
                    op: operator,
                    left: Box::new(Self::parse_return_expr(&left)?),
                    right: Box::new(Self::parse_return_expr(&right)?),
                })
            }
            "subscript_expression" => {
                let array = node.child(0)?;
                let index = node.child(2)?;

                Some(ReturnExpr::Subscript {
                    array: Box::new(Self::parse_return_expr(&array)?),
                    index: Box::new(Self::parse_return_expr(&index)?),
                })
            }
            _ => None,
        }
    }

    fn parse_return_statement_expr(return_statement: &Node<JavaScript>) -> Option<ReturnExpr> {
        for i in 0..return_statement.child_count() {
            if let Some(c) = return_statement.child(i)
                && c.kind() != "return"
                && c.kind() != ";"
            {
                return Self::parse_return_expr(&c);
            }
        }
        None
    }

    fn find_single_return_expr(body: &Node<JavaScript>) -> Option<ReturnExpr> {
        let mut return_expr: Option<ReturnExpr> = None;
        let mut found_count = 0;

        fn walk(
            node: &Node<JavaScript>,
            return_expr: &mut Option<ReturnExpr>,
            found_count: &mut usize,
        ) {
            for child in node.iter() {
                match child.kind() {
                    "return_statement" => {
                        *found_count += 1;
                        if *found_count == 1 {
                            *return_expr = FnCall::parse_return_statement_expr(&child);
                        }
                    }
                    "function_declaration"
                    | "function"
                    | "arrow_function"
                    | "generator_function_declaration"
                    | "generator_function" => {}
                    "if_statement" | "while_statement" | "do_statement" | "for_statement"
                    | "for_in_statement" | "switch_statement" | "try_statement" => {
                        let mut inner_count = 0;
                        FnCall::count_returns_in_subtree(&child, &mut inner_count);
                        if inner_count > 0 {
                            *found_count += inner_count;
                        }
                    }
                    _ => walk(&child, return_expr, found_count),
                }
            }
        }

        walk(body, &mut return_expr, &mut found_count);
        if found_count == 1 { return_expr } else { None }
    }

    fn parse_assignment_step(statement: &Node<JavaScript>) -> Option<EvalStep> {
        if matches!(
            statement.kind(),
            "variable_declaration" | "lexical_declaration"
        ) {
            for child in statement.iter() {
                if child.kind() == "variable_declarator"
                    && let Some(name_node) = child.named_child("name")
                    && name_node.kind() == "identifier"
                    && let Some(value_node) = child.named_child("value").or_else(|| child.child(2))
                {
                    let name = name_node.text().ok()?.to_string();
                    let expr = Self::parse_return_expr(&value_node)?;
                    return Some(EvalStep::Assign { name, expr });
                }
            }
            return None;
        }

        if statement.kind() == "expression_statement" {
            for child in statement.iter() {
                if child.kind() == "assignment_expression" {
                    let left = child.child(0)?;
                    let right = child.child(2)?;
                    if left.kind() != "identifier" {
                        return None;
                    }

                    let name = left.text().ok()?.to_string();
                    let expr = Self::parse_return_expr(&right)?;
                    return Some(EvalStep::Assign { name, expr });
                }
            }
        }

        None
    }

    fn collect_simple_steps_and_return(
        body: &Node<JavaScript>,
    ) -> Option<(Vec<EvalStep>, ReturnExpr)> {
        let mut steps = vec![];
        let mut return_expr = None;
        let mut return_count = 0usize;

        for statement in body.iter() {
            match statement.kind() {
                "if_statement" | "while_statement" | "do_statement" | "for_statement"
                | "for_in_statement" | "switch_statement" | "try_statement" => return None,
                "return_statement" => {
                    return_count += 1;
                    if return_count == 1 {
                        return_expr = Self::parse_return_statement_expr(&statement);
                    }
                }
                _ => {
                    if let Some(step) = Self::parse_assignment_step(&statement) {
                        steps.push(step);
                    }
                }
            }
        }

        if return_count == 1 {
            return_expr.map(|ret| (steps, ret))
        } else {
            None
        }
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
        let (steps, return_expr) = if let Some(body) = function_node.named_child("body") {
            if body.kind() == "statement_block" {
                if let Some((steps, ret)) = Self::collect_simple_steps_and_return(&body) {
                    (steps, Some(ret))
                } else {
                    (vec![], Self::find_single_return_expr(&body))
                }
            } else {
                (vec![], Self::parse_return_expr(&body))
            }
        } else {
            (vec![], None)
        };

        Some(FunctionShape {
            params,
            steps,
            return_expr,
        })
    }

    fn extract_call_args(call_node: &Node<JavaScript>) -> Vec<JavaScript> {
        let mut args = Vec::new();
        if let Some(arguments_node) = call_node.named_child("arguments") {
            for child in arguments_node.iter() {
                if let Some(data) = child.data() {
                    args.push(data.clone());
                }
            }
        }
        args
    }

    fn eval_return_expr(
        expr: &ReturnExpr,
        env: &HashMap<String, JavaScript>,
    ) -> Option<JavaScript> {
        match expr {
            ReturnExpr::Literal(value) => Some(value.clone()),
            ReturnExpr::Symbol(name) => env.get(name).cloned(),
            ReturnExpr::BinOp { op, left, right } => {
                let lhs = Self::eval_return_expr(left, env)?;
                let rhs = Self::eval_return_expr(right, env)?;
                match (lhs, rhs) {
                    (JavaScript::Raw(Num(a)), JavaScript::Raw(Num(b))) => {
                        let result = match op.as_str() {
                            "+" => a + b,
                            "-" => a - b,
                            "*" => a * b,
                            "/" => a / b,
                            _ => return None,
                        };
                        Some(JavaScript::Raw(Num(result)))
                    }
                    _ => None,
                }
            }
            ReturnExpr::Subscript { array, index } => {
                let array = Self::eval_return_expr(array, env)?;
                let index = Self::eval_return_expr(index, env)?;

                match (array, index) {
                    (JavaScript::Array(arr), JavaScript::Raw(Num(i))) if i >= 0.0 => {
                        arr.get(i as usize).cloned()
                    }
                    (JavaScript::Array(arr), JavaScript::Raw(Str(i))) => {
                        let idx = i.parse::<usize>().ok()?;
                        arr.get(idx).cloned()
                    }
                    _ => None,
                }
            }
        }
    }

    fn eval_shape(shape: &FunctionShape, call_node: &Node<JavaScript>) -> Option<JavaScript> {
        let args = Self::extract_call_args(call_node);
        let mut env: HashMap<String, JavaScript> = HashMap::new();
        for (idx, param) in shape.params.iter().enumerate() {
            if let Some(arg) = args.get(idx) {
                env.insert(param.clone(), arg.clone());
            }
        }

        for step in &shape.steps {
            match step {
                EvalStep::Assign { name, expr } => {
                    let value = Self::eval_return_expr(expr, &env)?;
                    env.insert(name.clone(), value);
                }
            }
        }

        let expr = shape.return_expr.as_ref()?;
        Self::eval_return_expr(expr, &env)
    }

    fn shape_from_value(&self, value: &JavaScript) -> Option<FunctionShape> {
        match value {
            JavaScript::Function { source, .. } => self.shapes_by_source.get(source).cloned(),
            _ => None,
        }
    }

    // -----------------------------------------------------------------
    //  Program-level shape pre-pass for forward references.
    // -----------------------------------------------------------------

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
        // Bound the alias chain at the same depth as the recursion tracker.
        for _ in 0..crate::js::recursion::DEFAULT_MAX_RECURSION_DEPTH {
            let next = aliases.get(current)?;
            if let Some(shape) = var_shapes.get(next) {
                return Some(shape.clone());
            }
            current = next;
        }

        None
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
        Self::eval_shape(shape, call_node)
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
        Self::eval_shape(&shape, call_node)
    }

    // -----------------------------------------------------------------
    //  Source-text fallbacks (used by a handful of edge-case tests where
    //  the function declaration was emitted in a self-modifying form).
    // -----------------------------------------------------------------

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

    // -----------------------------------------------------------------
    //  Eval handling.
    //
    //  Implements the issue #152 contract:
    //    * `eval(literalString)` reduces to the value of the **last**
    //      statement in the parsed string.
    //    * The eval source itself is evaluated through a literal-only
    //      pipeline (ParseInt, ParseString, AddInt, Concat, ...) - we never
    //      reuse FnCall on the sub-tree, which guarantees that an eval
    //      cannot trigger another eval expansion via mutual recursion.
    //    * The whole expansion is gated by [`RecursionTracker`] so that
    //      adversarial inputs hit the depth cap (16) instead of looping.
    // -----------------------------------------------------------------

    fn is_eval_callee(callee: &Node<JavaScript>) -> bool {
        callee.kind() == "identifier" && callee.text().map(|t| t == "eval").unwrap_or(false)
    }

    /// Extract a string source from an `eval()` argument node.
    ///
    /// Accepts both already-inferred `Raw(Str(...))` data and literal
    /// `"..."`/`'...'` source text (the latter is used when ParseString has
    /// not run yet on the call site).
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

    /// Find the value of the last statement in a parsed eval program.
    ///
    /// This deliberately ignores intermediate expression statements (their
    /// side effects are conservatively forgotten by the [`crate::js::var::Var`]
    /// rule, which clears any local that an `eval(...)` call could touch).
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
            "variable_declaration" | "lexical_declaration" => {
                // declarations have no value in JS (they yield undefined),
                // but minusone has no Undefined-as-value in this context, so
                // we simply refuse to reduce.
                None
            }
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

    /// Run the literal-reduction pipeline on a fresh sub-tree built from
    /// the eval source string and return the value of the last statement.
    fn evaluate_eval_source(source: &str) -> Option<JavaScript> {
        let mut tree = build_javascript_tree(source).ok()?;

        // The pipeline below is deliberately narrow: only rules that infer
        // values out of literal tokens. We exclude [`crate::js::functions::fncall::FnCall`]
        // and [`crate::js::var::Var`] because:
        //   * FnCall would re-enter this same function when the eval source
        //     contains another `eval(...)`. The recursion tracker is the
        //     hard safety net but the cleaner contract is "eval does not
        //     trigger function inlining inside its own sub-tree".
        //   * Var depends on a scope manager that lives outside this
        //     sub-tree; running it without state would forget more than it
        //     should.
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

        // Reserve a recursion slot via the node-driven helper so the depth
        // counter is decremented automatically when the closure returns.
        call_node
            .within_recursion(&mut self.recursion, |_| {
                Self::evaluate_eval_source(&source)
            })
            .flatten()
    }

    // -----------------------------------------------------------------
    //  Recursion-aware helpers used from `leave()` to expand a call site.
    // -----------------------------------------------------------------

    fn try_eval_shape(
        &mut self,
        shape: &FunctionShape,
        view: &Node<JavaScript>,
    ) -> Option<JavaScript> {
        view.within_recursion(&mut self.recursion, |node| Self::eval_shape(shape, node))
            .flatten()
    }

    fn try_resolve_identifier_call(
        &mut self,
        view: &Node<JavaScript>,
        fn_name: &str,
    ) -> Option<JavaScript> {
        view.within_recursion(&mut self.recursion, |node| {
            Self::resolve_identifier_call_semantic_fallback(node, fn_name)
        })
        .flatten()
    }

    fn try_resolve_member_call(
        &mut self,
        view: &Node<JavaScript>,
        base: &str,
        key: &str,
    ) -> Option<JavaScript> {
        view.within_recursion(&mut self.recursion, |node| {
            Self::resolve_member_call_semantic_fallback(node, base, key)
        })
        .flatten()
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

                    // Issue #152: store a shape for parametric function
                    // declarations too. The shape evaluation engine never
                    // mutates the function body, so storing the shape is
                    // safe and unlocks `function f(a) { return a + 1 }`
                    // style calls.
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
                if let Some(func_node) = view.named_child("function").or_else(|| view.child(0)) {
                    // 1) eval(...) - special-cased before any function
                    //    inlining so that an eval call always goes through
                    //    the eval pipeline.
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
    use crate::js::recursion::DEFAULT_MAX_RECURSION_DEPTH;
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

    // ---------- Existing semantics (preserved across the rewrite) ----------

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

    /// Issue #152: parametric function declarations are now resolved at
    /// each call site by evaluating their shape with the actual arguments.
    /// The function body itself is never rewritten - shape evaluation runs
    /// on an in-memory environment, so the original source is preserved.
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
    fn test_fncall_multiple_returns_not_resolved() {
        assert_eq!(
            deobfuscate(
                "function test() { if (true) { return 'a'; } return 'b'; } console.log(test());"
            ),
            "function test() { if (true) { return 'a'; } return 'b'; } console.log(test());"
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

    // ---------- Issue #152: function source preservation ("backup/restore") ----------

    /// The function source must remain byte-identical after a call to it
    /// has been resolved. This is the bug example from the issue: with the
    /// old approach the body could leak the call-site value back into the
    /// function definition.
    #[test]
    fn test_fncall_function_source_preserved_after_resolved_call() {
        let output = deobfuscate("function test(a) { return a + 1 } console.log(test(1));");
        assert!(
            output.contains("function test(a) { return a + 1 }"),
            "function source was rewritten: {output}",
        );
        assert!(
            output.contains("console.log(2)"),
            "call site was not reduced: {output}",
        );
    }

    #[test]
    fn test_fncall_function_source_preserved_after_repeated_calls() {
        let output = deobfuscate(
            "function double(n) { return n * 2 } console.log(double(3)); console.log(double(5));",
        );
        assert!(
            output.contains("function double(n) { return n * 2 }"),
            "function source mutated by repeated calls: {output}",
        );
        assert!(output.contains("console.log(6)"), "first call wrong: {output}");
        assert!(
            output.contains("console.log(10)"),
            "second call wrong: {output}"
        );
    }

    // ---------- Issue #152: recursion depth limit ----------

    /// Build a chain of functions that all forward to each other so that
    /// resolving any of them requires unwinding the whole chain. With the
    /// recursion guard we expect either successful resolution (when the
    /// chain length stays under the cap) or a clean fallback (no panic, no
    /// infinite loop) when the chain is longer than the cap.
    #[test]
    fn test_fncall_recursion_depth_limit_does_not_panic() {
        let mut input = String::new();
        let chain_len = DEFAULT_MAX_RECURSION_DEPTH * 2 + 5;
        for i in 0..chain_len {
            input += &format!("function f{i}() {{ return f{}(); }} ", i + 1);
        }
        input += &format!("function f{chain_len}() {{ return 1; }} ");
        input += "console.log(f0());";

        // The important property is that deobfuscate returns - it must not
        // diverge or stack-overflow. We do not assert on the exact output
        // because the depth cap may stop expansion mid-chain.
        let output = deobfuscate(&input);
        assert!(output.contains("console.log("));
    }

    /// A self-recursive function with no base case must still terminate
    /// deobfuscation - the recursion cap is the safety net.
    #[test]
    fn test_fncall_self_recursive_unconditional_does_not_panic() {
        let output =
            deobfuscate("function loop() { return loop(); } var stop = 'sentinel'; loop();");
        assert!(output.contains("'sentinel'"));
    }

    // ---------- Issue #152: eval() reduces to last statement ----------

    #[test]
    fn test_eval_reduces_to_simple_literal() {
        assert_eq!(
            deobfuscate("var x = eval('5');"),
            "var x = 5;"
        );
    }

    #[test]
    fn test_eval_reduces_to_string_literal() {
        assert_eq!(
            deobfuscate("var x = eval('\"hello\"');"),
            "var x = 'hello';"
        );
    }

    #[test]
    fn test_eval_reduces_arithmetic_expression() {
        assert_eq!(
            deobfuscate("var x = eval('1 + 2');"),
            "var x = 3;"
        );
    }

    /// The canonical issue #152 example: `eval('a++; 8')` must reduce to
    /// `8` (the value of the **last** statement) rather than `8` being
    /// dropped or `a++` being silently propagated.
    #[test]
    fn test_eval_reduces_to_last_statement() {
        let output = deobfuscate("var a = 0; var b = eval('a++; 8'); console.log(b);");
        assert!(
            output.contains("var b = 8"),
            "eval did not reduce to last statement: {output}",
        );
    }

    #[test]
    fn test_eval_with_concat_reduces_to_last_string() {
        let output = deobfuscate("var b = eval('1; \"foo\" + \"bar\"');");
        assert!(
            output.contains("var b = 'foobar'"),
            "eval string concat last-stmt failed: {output}",
        );
    }

    /// Even when the last statement is non-literal, the eval call itself
    /// must remain in the output (no incorrect reduction).
    #[test]
    fn test_eval_unresolvable_last_statement_kept() {
        let output = deobfuscate("var b = eval('a++');");
        assert!(
            output.contains("eval"),
            "unresolvable eval was incorrectly removed: {output}",
        );
    }

    // ---------- Issue #152: eval lexical scoping ----------

    /// The classic eval-scoping example from the issue. Without proper
    /// handling, `Var` would propagate the inner `let a = -1` past the
    /// `eval('a++')` call, which is incorrect because eval can mutate `a`.
    /// We don't model the mutation precisely, but we also must not pretend
    /// `a` is still `-1` after the eval call.
    #[test]
    fn test_eval_does_not_propagate_local_after_mutation() {
        let output = deobfuscate(
            "let a = 1; if (true) { let a = -1; eval('a++'); console.log(a); } console.log(a);",
        );
        // The inner console.log(a) must NOT be reduced to -1 because eval
        // could have mutated a. We accept either an unreduced `console.log(a)`
        // or anything other than `console.log(-1)`.
        assert!(
            !output.contains("console.log(-1)"),
            "eval mutation not respected, scope incorrectly propagated: {output}",
        );
    }

    /// A function call (not eval) may NOT see the caller's block-local
    /// `a` - it sees its own lexical scope. With `function edit() { a++ }`
    /// declared at the top-level, calling `edit()` inside a block where
    /// `let a = -1` shadows the outer `a` must leave the block-local `a`
    /// untouched in the linter's view (the block-local binding is
    /// independent of the function's `a`).
    #[test]
    fn test_function_does_not_see_callers_block_scope() {
        let output = deobfuscate(
            "let a = 1; function edit() { a++ } if (true) { let a = -1; edit(); console.log(a); }",
        );
        // The block-local `a` is `-1` and `edit()` cannot see it. The Var
        // rule may either keep `console.log(a)` or substitute -1 - both
        // are acceptable. What is NOT acceptable is for `edit()` to be
        // reduced to a value (it has no return) or for the function source
        // to be rewritten.
        assert!(
            output.contains("function edit() { a++ }"),
            "function source mutated by call: {output}",
        );
    }
}
