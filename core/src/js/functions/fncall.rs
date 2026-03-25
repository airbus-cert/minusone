use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::Value::{Num, Str};
use crate::js::functions::function::function_value_from_node;
use crate::rule::RuleMut;
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
pub struct FnCall {
    functions: HashMap<String, JavaScript>,
    vars: HashMap<String, JavaScript>,
    object_fields: HashMap<(String, String), JavaScript>,
    var_shapes: HashMap<String, FunctionShape>,
    object_field_shapes: HashMap<(String, String), FunctionShape>,
    shapes_by_source: HashMap<String, FunctionShape>,
}

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

impl Default for FnCall {
    fn default() -> Self {
        FnCall {
            functions: HashMap::new(),
            vars: HashMap::new(),
            object_fields: HashMap::new(),
            var_shapes: HashMap::new(),
            object_field_shapes: HashMap::new(),
            shapes_by_source: HashMap::new(),
        }
    }
}

impl FnCall {
    fn reduce_array_subscript(node: &mut NodeMut<JavaScript>) {
        let view = node.view();
        if let (Some(array_node), Some(index_node)) = (view.child(0), view.child(2)) {
            if let (Some(JavaScript::Array(arr)), Some(JavaScript::Raw(Num(index)))) =
                (array_node.data(), index_node.data())
            {
                if *index >= 0.0 {
                    let idx = *index as usize;
                    if idx < arr.len() {
                        node.reduce(arr[idx].clone());
                        return;
                    }
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
                            if let Some(c) = child.child(i) {
                                if c.kind() != "return" && c.kind() != ";" {
                                    if let Some(data) = c.data() {
                                        *return_value = Some(data.clone());
                                    }
                                    break;
                                }
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
                    if child.kind() != "(" && child.kind() != ")" {
                        if let Some(expr) = Self::parse_return_expr(&child) {
                            return Some(expr);
                        }
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
        for _ in 0..16 {
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
                if let Some(name_node) = view.named_child("name") {
                    if name_node.kind() == "identifier" {
                        let fn_name = name_node.text()?.to_string();

                        if let Some(body) = view.named_child("body") {
                            if let Some(return_data) = Self::find_single_return_value(&body) {
                                trace!(
                                    "FnCall (L): Recorded function '{}' with return value: {:?}",
                                    fn_name, return_data
                                );
                                self.functions.insert(fn_name, return_data);
                            }
                        }
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
                    if func_node.kind() == "identifier" {
                        let fn_name = func_node.text()?.to_string();

                        if let Some(return_data) = self.functions.get(&fn_name) {
                            trace!(
                                "FnCall (L): Resolving call to '{}' with: {:?}",
                                fn_name, return_data
                            );
                            node.reduce(return_data.clone());
                        } else if let Some(shape) = self.var_shapes.get(&fn_name)
                            && let Some(value) = Self::eval_shape(shape, &view)
                        {
                            trace!(
                                "FnCall (L): Resolving call to semantic variable function with: {:?}",
                                value
                            );
                            node.reduce(value);
                        } else if let Some(value) = self.vars.get(&fn_name)
                            && let Some(return_value) = Self::function_return_from_value(value)
                        {
                            trace!(
                                "FnCall (L): Resolving call to variable function value with: {:?}",
                                return_value
                            );
                            node.reduce(return_value);
                        } else if let Some(value) =
                            Self::resolve_identifier_call_semantic_fallback(&view, &fn_name)
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
                        && let Some(shape) =
                            self.object_field_shapes.get(&(base.clone(), key.clone()))
                        && let Some(value) = Self::eval_shape(shape, &view)
                    {
                        trace!(
                            "FnCall (L): Resolving call to semantic object field function with: {:?}",
                            value
                        );
                        node.reduce(value);
                    } else if let Some((base, key)) = Self::extract_member_access(&func_node)
                        && let Some(value) = self.object_fields.get(&(base, key))
                        && let Some(return_value) = Self::function_return_from_value(value)
                    {
                        trace!(
                            "FnCall (L): Resolving call to object field function with: {:?}",
                            return_value
                        );
                        node.reduce(return_value);
                    } else if let Some((base, key)) = Self::extract_member_access(&func_node)
                        && let Some(value) =
                            Self::resolve_member_call_semantic_fallback(&view, &base, &key)
                    {
                        trace!(
                            "FnCall (L): Resolving call to semantic fallback object field function with: {:?}",
                            value
                        );
                        node.reduce(value);
                    } else if let Some(return_value) = Self::resolve_member_call_from_source(&view)
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
    use crate::js::integer::{ParseInt, SubAddInt};
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
                SubAddInt::default(),
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
    fn test_fncall_does_not_resolve_param_dependent_return() {
        assert_eq!(
            deobfuscate("function test(x) { return x; } console.log(test('hello'));"),
            "function test(x) { return x; } console.log(test('hello'));"
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
}
