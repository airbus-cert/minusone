use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::Value::{Bool, Num, Str};
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
    Call {
        name: String,
        args: Vec<ReturnExpr>,
    },
    Ternary {
        condition: Box<ReturnExpr>,
        then_branch: Box<ReturnExpr>,
        else_branch: Box<ReturnExpr>,
    },
    ArrayLiteral(Vec<ReturnExpr>),
}

#[derive(Clone)]
enum ReturnForm {
    Plain(ReturnExpr),
    Conditional {
        condition: ReturnExpr,
        then_branch: Box<ReturnForm>,
        else_branch: Box<ReturnForm>,
    },
}

#[derive(Clone)]
struct FunctionShape {
    params: Vec<String>,
    steps: Vec<EvalStep>,
    return_form: Option<ReturnForm>,
}

#[derive(Clone)]
enum EvalStep {
    Assign { name: String, expr: ReturnExpr },
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

    fn parse_return_expr(node: &Node<JavaScript>) -> Option<ReturnExpr> {
        if let Some(data) = node.data() {
            return Some(ReturnExpr::Literal(data.clone()));
        }

        match node.kind() {
            "identifier" => {
                let name = node.text().ok()?.to_string();
                Some(ReturnExpr::Symbol(name))
            }

            // Raw literals at hoist time, before ParseInt/ParseString have run.
            "number" => {
                let raw = node.text().ok()?;
                let cleaned = if !raw.starts_with('_') && !raw.ends_with('_') {
                    raw.replace('_', "")
                } else {
                    raw.to_string()
                };
                let n = if cleaned.len() > 2
                    && (cleaned.starts_with("0x") || cleaned.starts_with("0X"))
                {
                    u64::from_str_radix(&cleaned[2..], 16).ok()? as f64
                } else if cleaned.len() > 2
                    && (cleaned.starts_with("0o") || cleaned.starts_with("0O"))
                {
                    u64::from_str_radix(&cleaned[2..], 8).ok()? as f64
                } else if cleaned.len() > 2
                    && (cleaned.starts_with("0b") || cleaned.starts_with("0B"))
                {
                    u64::from_str_radix(&cleaned[2..], 2).ok()? as f64
                } else {
                    cleaned.parse::<f64>().ok()?
                };
                Some(ReturnExpr::Literal(JavaScript::Raw(Num(n))))
            }
            "string" => {
                let text = node.text().ok()?;
                if text.len() >= 2
                    && (text.starts_with('"') || text.starts_with('\''))
                    && text.ends_with(text.chars().next().unwrap())
                {
                    Some(ReturnExpr::Literal(JavaScript::Raw(Str(
                        crate::js::string::unescaped_js_string(text),
                    ))))
                } else {
                    None
                }
            }
            "true" => Some(ReturnExpr::Literal(JavaScript::Raw(Bool(true)))),
            "false" => Some(ReturnExpr::Literal(JavaScript::Raw(Bool(false)))),

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
            "call_expression" => {
                let callee = node.named_child("function").or_else(|| node.child(0))?;
                if callee.kind() != "identifier" {
                    return None;
                }
                let name = callee.text().ok()?.to_string();
                let args_node = node.named_child("arguments")?;
                let mut args = Vec::new();
                for child in args_node.iter() {
                    if matches!(child.kind(), "(" | ")" | ",") {
                        continue;
                    }
                    args.push(Self::parse_return_expr(&child)?);
                }
                Some(ReturnExpr::Call { name, args })
            }
            "unary_expression" => {
                let op_node = node.child(0)?;
                let operand_node = node
                    .named_child("argument")
                    .or_else(|| node.child(1))?;
                let op_text = op_node.text().ok()?;
                let operand = Self::parse_return_expr(&operand_node)?;
                match op_text {
                    "-" => Some(ReturnExpr::BinOp {
                        op: "-".to_string(),
                        left: Box::new(ReturnExpr::Literal(JavaScript::Raw(Num(0.0)))),
                        right: Box::new(operand),
                    }),
                    "+" => Some(operand),
                    _ => None,
                }
            }
            "array" => {
                let mut items = Vec::new();
                for child in node.iter() {
                    if matches!(child.kind(), "[" | "]" | ",") {
                        continue;
                    }
                    items.push(Self::parse_return_expr(&child)?);
                }
                Some(ReturnExpr::ArrayLiteral(items))
            }
            "ternary_expression" => {
                let cond = node
                    .named_child("condition")
                    .or_else(|| node.child(0))?;
                let then = node
                    .named_child("consequence")
                    .or_else(|| node.child(2))?;
                let alt = node
                    .named_child("alternative")
                    .or_else(|| node.child(4))?;

                Some(ReturnExpr::Ternary {
                    condition: Box::new(Self::parse_return_expr(&cond)?),
                    then_branch: Box::new(Self::parse_return_expr(&then)?),
                    else_branch: Box::new(Self::parse_return_expr(&alt)?),
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

    fn extract_block_shape(body: &Node<JavaScript>) -> Option<(Vec<EvalStep>, ReturnForm)> {
        let statements: Vec<Node<JavaScript>> = body
            .iter()
            .filter(|c| !matches!(c.kind(), "{" | "}"))
            .collect();
        if statements.is_empty() {
            return None;
        }

        let mut steps: Vec<EvalStep> = Vec::new();
        let mut i = 0;
        while i < statements.len() {
            let stmt = &statements[i];
            match stmt.kind() {
                "return_statement" => {
                    if i + 1 != statements.len() {
                        return None;
                    }
                    let form = Self::extract_return_form(stmt)?;
                    return Some((steps, form));
                }
                "if_statement" => {
                    // Implicit-else: `if (cond) {...return...} return Y;`
                    if stmt.named_child("alternative").is_none()
                        && i + 2 == statements.len()
                        && statements[i + 1].kind() == "return_statement"
                    {
                        let cond = stmt.named_child("condition")?;
                        let cond_expr = Self::parse_return_expr(&cond)?;
                        let consequence = stmt.named_child("consequence")?;
                        let then_form = Self::extract_return_form(&consequence)?;
                        let else_form = Self::extract_return_form(&statements[i + 1])?;
                        return Some((
                            steps,
                            ReturnForm::Conditional {
                                condition: cond_expr,
                                then_branch: Box::new(then_form),
                                else_branch: Box::new(else_form),
                            },
                        ));
                    }

                    if i + 1 != statements.len() {
                        return None;
                    }
                    let form = Self::extract_return_form(stmt)?;
                    return Some((steps, form));
                }
                _ => {
                    let step = Self::parse_assignment_step(stmt)?;
                    steps.push(step);
                    i += 1;
                }
            }
        }

        None
    }

    fn extract_return_form(stmt: &Node<JavaScript>) -> Option<ReturnForm> {
        match stmt.kind() {
            "return_statement" => {
                Self::parse_return_statement_expr(stmt).map(ReturnForm::Plain)
            }
            "statement_block" => {
                let (steps, form) = Self::extract_block_shape(stmt)?;
                if !steps.is_empty() {
                    return None;
                }
                Some(form)
            }
            "if_statement" => Self::extract_return_form_from_if(stmt),
            _ => None,
        }
    }

    fn extract_return_form_from_if(if_stmt: &Node<JavaScript>) -> Option<ReturnForm> {
        let cond = if_stmt.named_child("condition")?;
        let cond_expr = Self::parse_return_expr(&cond)?;

        let consequence = if_stmt.named_child("consequence")?;
        let then_form = Self::extract_return_form(&consequence)?;

        let alt = if_stmt.named_child("alternative")?;
        let inner = alt.iter().find(|c| c.kind() != "else")?;
        let else_form = Self::extract_return_form(&inner)?;

        Some(ReturnForm::Conditional {
            condition: cond_expr,
            then_branch: Box::new(then_form),
            else_branch: Box::new(else_form),
        })
    }

    fn fallback_single_return_form(body: &Node<JavaScript>) -> Option<ReturnForm> {
        Self::find_single_return_expr(body).map(ReturnForm::Plain)
    }

    // Recognise the obfuscator self-redefining idiom:
    // `function NAME() { steps...; NAME = function () { ... }; return NAME(); }`.
    fn extract_self_redefining_shape(
        body: &Node<JavaScript>,
        fn_name: &str,
    ) -> Option<(Vec<EvalStep>, ReturnForm)> {
        let statements: Vec<Node<JavaScript>> = body
            .iter()
            .filter(|c| !matches!(c.kind(), "{" | "}"))
            .collect();
        if statements.len() < 2 {
            return None;
        }

        let last = statements.last().unwrap();
        if last.kind() != "return_statement" {
            return None;
        }
        let return_expr = last
            .iter()
            .find(|c| !matches!(c.kind(), "return" | ";"))?;
        if return_expr.kind() != "call_expression" {
            return None;
        }
        let return_callee = return_expr
            .named_child("function")
            .or_else(|| return_expr.child(0))?;
        if return_callee.kind() != "identifier" {
            return None;
        }
        if return_callee.text().ok()? != fn_name {
            return None;
        }

        let assign_stmt = &statements[statements.len() - 2];
        if assign_stmt.kind() != "expression_statement" {
            return None;
        }
        let assign_expr = assign_stmt
            .iter()
            .find(|c| c.kind() == "assignment_expression")?;
        let lhs = assign_expr.child(0)?;
        if lhs.kind() != "identifier" || lhs.text().ok()? != fn_name {
            return None;
        }
        let rhs = assign_expr.child(2)?;
        if !matches!(
            rhs.kind(),
            "function" | "function_expression" | "arrow_function"
        ) {
            return None;
        }

        let inner_params = Self::extract_params(&rhs);
        if !inner_params.is_empty() {
            return None;
        }
        let inner_body = rhs.named_child("body")?;
        let inner_form = if inner_body.kind() == "statement_block" {
            let (inner_steps, inner_form) = Self::extract_block_shape(&inner_body)?;
            if !inner_steps.is_empty() {
                return None;
            }
            inner_form
        } else {
            ReturnForm::Plain(Self::parse_return_expr(&inner_body)?)
        };

        let mut steps: Vec<EvalStep> = Vec::new();
        for stmt in &statements[..statements.len() - 2] {
            let step = Self::parse_assignment_step(stmt)?;
            steps.push(step);
        }

        Some((steps, inner_form))
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
        let fn_name: Option<String> = function_node
            .named_child("name")
            .and_then(|n| n.text().ok().map(|s| s.to_string()));

        let (steps, return_form) = if let Some(body) = function_node.named_child("body") {
            if body.kind() == "statement_block" {
                if let Some((steps, form)) = Self::extract_block_shape(&body) {
                    (steps, Some(form))
                } else if params.is_empty()
                    && let Some(name) = fn_name.as_deref()
                    && let Some((steps, form)) =
                        Self::extract_self_redefining_shape(&body, name)
                {
                    (steps, Some(form))
                } else {
                    (vec![], Self::fallback_single_return_form(&body))
                }
            } else {
                (
                    vec![],
                    Self::parse_return_expr(&body).map(ReturnForm::Plain),
                )
            }
        } else {
            (vec![], None)
        };

        Some(FunctionShape {
            params,
            steps,
            return_form,
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
        shapes: &HashMap<String, FunctionShape>,
        recursion: &mut RecursionTracker,
    ) -> Option<JavaScript> {
        match expr {
            ReturnExpr::Literal(value) => Some(value.clone()),
            ReturnExpr::Symbol(name) => env.get(name).cloned(),
            ReturnExpr::BinOp { op, left, right } => {
                if op == "&&" || op == "||" {
                    let lhs = Self::eval_return_expr(left, env, shapes, recursion)?;
                    let lhs_bool = match &lhs {
                        JavaScript::Raw(Bool(b)) => Some(*b),
                        _ => None,
                    }?;
                    return match (op.as_str(), lhs_bool) {
                        ("&&", false) => Some(JavaScript::Raw(Bool(false))),
                        ("&&", true) => Self::eval_return_expr(right, env, shapes, recursion),
                        ("||", true) => Some(JavaScript::Raw(Bool(true))),
                        ("||", false) => Self::eval_return_expr(right, env, shapes, recursion),
                        _ => None,
                    };
                }

                let lhs = Self::eval_return_expr(left, env, shapes, recursion)?;
                let rhs = Self::eval_return_expr(right, env, shapes, recursion)?;
                Self::eval_binop(op.as_str(), &lhs, &rhs)
            }
            ReturnExpr::Subscript { array, index } => {
                let array = Self::eval_return_expr(array, env, shapes, recursion)?;
                let index = Self::eval_return_expr(index, env, shapes, recursion)?;

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
            ReturnExpr::Call { name, args } => {
                let callee_shape = shapes.get(name)?;
                if callee_shape.params.len() != args.len() {
                    return None;
                }

                let mut sub_env: HashMap<String, JavaScript> = HashMap::new();
                for (param, arg_expr) in callee_shape.params.iter().zip(args.iter()) {
                    let value = Self::eval_return_expr(arg_expr, env, shapes, recursion)?;
                    sub_env.insert(param.clone(), value);
                }

                if !recursion.bump() {
                    return None;
                }
                let result =
                    Self::eval_shape_with_env(callee_shape, sub_env, shapes, recursion);
                recursion.unbump();
                result
            }
            ReturnExpr::Ternary {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond = Self::eval_return_expr(condition, env, shapes, recursion)?;
                match cond {
                    JavaScript::Raw(Bool(true)) => {
                        Self::eval_return_expr(then_branch, env, shapes, recursion)
                    }
                    JavaScript::Raw(Bool(false)) => {
                        Self::eval_return_expr(else_branch, env, shapes, recursion)
                    }
                    _ => None,
                }
            }
            ReturnExpr::ArrayLiteral(items) => {
                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    values.push(Self::eval_return_expr(item, env, shapes, recursion)?);
                }
                Some(JavaScript::Array(values))
            }
        }
    }

    fn eval_binop(op: &str, lhs: &JavaScript, rhs: &JavaScript) -> Option<JavaScript> {
        match (lhs, rhs) {
            (JavaScript::Raw(Num(a)), JavaScript::Raw(Num(b))) => match op {
                "+" => Some(JavaScript::Raw(Num(a + b))),
                "-" => Some(JavaScript::Raw(Num(a - b))),
                "*" => Some(JavaScript::Raw(Num(a * b))),
                "/" => Some(JavaScript::Raw(Num(a / b))),
                "%" => Some(JavaScript::Raw(Num(a % b))),
                "==" | "===" => Some(JavaScript::Raw(Bool(a == b))),
                "!=" | "!==" => Some(JavaScript::Raw(Bool(a != b))),
                "<" => Some(JavaScript::Raw(Bool(a < b))),
                ">" => Some(JavaScript::Raw(Bool(a > b))),
                "<=" => Some(JavaScript::Raw(Bool(a <= b))),
                ">=" => Some(JavaScript::Raw(Bool(a >= b))),
                _ => None,
            },
            (JavaScript::Raw(Str(a)), JavaScript::Raw(Str(b))) => match op {
                "+" => Some(JavaScript::Raw(Str(format!("{a}{b}")))),
                "==" | "===" => Some(JavaScript::Raw(Bool(a == b))),
                "!=" | "!==" => Some(JavaScript::Raw(Bool(a != b))),
                "<" => Some(JavaScript::Raw(Bool(a < b))),
                ">" => Some(JavaScript::Raw(Bool(a > b))),
                "<=" => Some(JavaScript::Raw(Bool(a <= b))),
                ">=" => Some(JavaScript::Raw(Bool(a >= b))),
                _ => None,
            },
            (JavaScript::Raw(Bool(a)), JavaScript::Raw(Bool(b))) => match op {
                "==" | "===" => Some(JavaScript::Raw(Bool(a == b))),
                "!=" | "!==" => Some(JavaScript::Raw(Bool(a != b))),
                _ => None,
            },
            _ => None,
        }
    }

    fn eval_return_form(
        form: &ReturnForm,
        env: &HashMap<String, JavaScript>,
        shapes: &HashMap<String, FunctionShape>,
        recursion: &mut RecursionTracker,
    ) -> Option<JavaScript> {
        match form {
            ReturnForm::Plain(expr) => Self::eval_return_expr(expr, env, shapes, recursion),
            ReturnForm::Conditional {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond = Self::eval_return_expr(condition, env, shapes, recursion)?;
                match cond {
                    JavaScript::Raw(Bool(true)) => {
                        Self::eval_return_form(then_branch, env, shapes, recursion)
                    }
                    JavaScript::Raw(Bool(false)) => {
                        Self::eval_return_form(else_branch, env, shapes, recursion)
                    }
                    _ => None,
                }
            }
        }
    }

    fn eval_shape_with_env(
        shape: &FunctionShape,
        mut env: HashMap<String, JavaScript>,
        shapes: &HashMap<String, FunctionShape>,
        recursion: &mut RecursionTracker,
    ) -> Option<JavaScript> {
        for step in &shape.steps {
            match step {
                EvalStep::Assign { name, expr } => {
                    let value = Self::eval_return_expr(expr, &env, shapes, recursion)?;
                    env.insert(name.clone(), value);
                }
            }
        }

        let form = shape.return_form.as_ref()?;
        Self::eval_return_form(form, &env, shapes, recursion)
    }

    fn eval_shape(
        shape: &FunctionShape,
        call_node: &Node<JavaScript>,
        shapes: &HashMap<String, FunctionShape>,
        recursion: &mut RecursionTracker,
    ) -> Option<JavaScript> {
        let args = Self::extract_call_args(call_node);
        let mut env: HashMap<String, JavaScript> = HashMap::new();
        for (idx, param) in shape.params.iter().enumerate() {
            if let Some(arg) = args.get(idx) {
                env.insert(param.clone(), arg.clone());
            }
        }

        Self::eval_shape_with_env(shape, env, shapes, recursion)
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

    fn resolve_member_call_semantic_fallback<'a>(
        call_node: &Node<'a, JavaScript>,
        base: &str,
        key: &str,
        recursion: &mut RecursionTracker,
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
        Self::eval_shape(shape, call_node, &var_shapes, recursion)
    }

    fn resolve_identifier_call_semantic_fallback<'a>(
        call_node: &Node<'a, JavaScript>,
        fn_name: &str,
        recursion: &mut RecursionTracker,
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
        Self::eval_shape(&shape, call_node, &var_shapes, recursion)
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
        if !self.recursion.bump() {
            return None;
        }
        let result = Self::eval_shape(shape, view, &self.var_shapes, &mut self.recursion);
        self.recursion.unbump();
        result
    }

    fn try_resolve_identifier_call(
        &mut self,
        view: &Node<JavaScript>,
        fn_name: &str,
    ) -> Option<JavaScript> {
        if !self.recursion.bump() {
            return None;
        }
        let result = Self::resolve_identifier_call_semantic_fallback(
            view,
            fn_name,
            &mut self.recursion,
        );
        self.recursion.unbump();
        result
    }

    fn try_resolve_member_call(
        &mut self,
        view: &Node<JavaScript>,
        base: &str,
        key: &str,
    ) -> Option<JavaScript> {
        if !self.recursion.bump() {
            return None;
        }
        let result = Self::resolve_member_call_semantic_fallback(
            view,
            base,
            key,
            &mut self.recursion,
        );
        self.recursion.unbump();
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

            // Hoist top-level function_declarations so forward calls resolve.
            for child in view.iter() {
                Self::hoist_function_declaration(&child, &mut self.var_shapes);
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
