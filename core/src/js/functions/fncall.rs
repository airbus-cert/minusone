use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScriptRuleSet;
use crate::js::Value::{Num, Str};
use crate::js::build_javascript_tree;
use crate::js::functions::function::function_value_from_node;
use crate::js::linter::Linter;
use crate::js::strategy::JavaScriptStrategy;
use crate::rule::{RuleExecutionContext, RuleMut, RuleReference, RuleSetBuilderType};
use crate::tree::{ControlFlow, Node, NodeMut};
use log::{trace, warn};
use std::any::Any;
use std::collections::HashMap;

#[derive(Clone)]
struct FnCallSnapshot {
    function_sources: HashMap<String, String>,
    vars: HashMap<String, JavaScript>,
    object_fields: HashMap<(String, String), JavaScript>,
    member_functions: HashMap<String, JavaScript>,
    member_aliases: HashMap<String, String>,
}

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
#[derive(Clone)]
pub struct FnCall {
    function_sources: HashMap<String, String>,
    vars: HashMap<String, JavaScript>,
    object_fields: HashMap<(String, String), JavaScript>,
    member_functions: HashMap<String, JavaScript>,
    member_aliases: HashMap<String, String>,
    reset_on_program_enter: bool,
    nested_peer_rule_names: Option<Vec<String>>,
    nested_eval_depth: usize,
    max_nested_eval_depth: usize,
}

impl Default for FnCall {
    fn default() -> Self {
        FnCall {
            function_sources: HashMap::new(),
            vars: HashMap::new(),
            object_fields: HashMap::new(),
            member_functions: HashMap::new(),
            member_aliases: HashMap::new(),
            reset_on_program_enter: true,
            nested_peer_rule_names: None,
            nested_eval_depth: 0,
            max_nested_eval_depth: 3,
        }
    }
}

impl FnCall {
    fn is_function_like_kind(kind: &str) -> bool {
        matches!(
            kind,
            "function"
                | "function_expression"
                | "function_declaration"
                | "arrow_function"
                | "generator_function"
                | "generator_function_declaration"
        )
    }

    fn render_node_source(node: &Node<JavaScript>) -> Option<String> {
        let mut linter = Linter::default();
        node.apply(&mut linter).ok()?;
        Some(linter.output)
    }

    fn with_rendered_function_source(
        value: JavaScript,
        source_node: &Node<JavaScript>,
    ) -> JavaScript {
        let JavaScript::Function { return_value, .. } = value else {
            return value;
        };

        let source = Self::render_node_source(source_node)
            .unwrap_or_else(|| source_node.text().unwrap_or_default().to_string());
        JavaScript::Function {
            source,
            return_value,
        }
    }

    fn clear_state(&mut self) {
        self.function_sources.clear();
        self.vars.clear();
        self.object_fields.clear();
        self.member_functions.clear();
        self.member_aliases.clear();
    }

    fn clone_for_nested_eval(&self) -> Self {
        let mut cloned = self.clone();
        cloned.reset_on_program_enter = false;
        cloned.nested_eval_depth += 1;
        cloned
    }

    fn track_function_binding(&mut self, name: String, value: JavaScript) {
        if let Some(source) = Self::function_source_from_value(&value) {
            self.function_sources
                .insert(name.clone(), source.to_string());
            self.vars.insert(name, value);
        }
    }

    fn resolve_function_value_from_identifier(&self, identifier: &str) -> Option<JavaScript> {
        self.vars.get(identifier).cloned().or_else(|| {
            self.function_sources
                .get(identifier)
                .map(|source| JavaScript::Function {
                    source: source.clone(),
                    return_value: None,
                })
        })
    }

    fn resolve_function_value_from_node(&self, node: &Node<JavaScript>) -> Option<JavaScript> {
        node.data()
            .cloned()
            .or_else(|| function_value_from_node(node))
            .or_else(|| {
                if node.kind() == "identifier" {
                    node.text().ok().and_then(|identifier| {
                        self.resolve_function_value_from_identifier(identifier)
                    })
                } else {
                    None
                }
            })
    }

    fn resolve_inline_callable_value(&self, node: &Node<JavaScript>) -> Option<JavaScript> {
        if let Some(value @ JavaScript::Function { .. }) =
            self.resolve_function_value_from_node(node)
        {
            return Some(value);
        }

        match node.kind() {
            "parenthesized_expression" => node
                .child(1)
                .and_then(|inner| self.resolve_inline_callable_value(&inner)),
            "assignment_expression" => node
                .child(2)
                .and_then(|right| self.resolve_inline_callable_value(&right)),
            _ => None,
        }
    }

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

    fn function_source_from_value(value: &JavaScript) -> Option<&str> {
        match value {
            JavaScript::Function { source, .. } => Some(source.as_str()),
            _ => None,
        }
    }

    fn push_unique_source(sources: &mut Vec<String>, source: Option<&str>) {
        if let Some(source) = source {
            let source = source.to_string();
            if !sources.contains(&source) {
                sources.push(source);
            }
        }
    }

    fn collect_call_target_sources(&self, func_node: &Node<JavaScript>) -> Vec<String> {
        let mut sources = Vec::new();

        if func_node.kind() == "identifier" {
            if let Ok(identifier) = func_node.text() {
                if let Some(source) = self.function_sources.get(identifier) {
                    Self::push_unique_source(&mut sources, Some(source));
                }

                if let Some(value) = self.vars.get(identifier) {
                    Self::push_unique_source(&mut sources, Self::function_source_from_value(value));
                }
            }
        }

        if let Ok(member_key) = func_node.text() {
            if let Some(value) = self.member_functions.get(member_key) {
                Self::push_unique_source(&mut sources, Self::function_source_from_value(value));
            }

            if let Some(alias) = self.member_aliases.get(member_key) {
                if let Some(source) = self.function_sources.get(alias) {
                    Self::push_unique_source(&mut sources, Some(source));
                }

                if let Some(value) = self.vars.get(alias) {
                    Self::push_unique_source(&mut sources, Self::function_source_from_value(value));
                }
            }
        }

        if let Some((base, key)) = Self::extract_member_access(func_node)
            && let Some(value) = self.object_fields.get(&(base, key))
        {
            Self::push_unique_source(&mut sources, Self::function_source_from_value(value));
        }

        Self::push_unique_source(
            &mut sources,
            func_node.data().and_then(Self::function_source_from_value),
        );

        if let Some(inline_value) = self.resolve_inline_callable_value(func_node) {
            Self::push_unique_source(
                &mut sources,
                Self::function_source_from_value(&inline_value),
            );
        }

        sources
    }

    fn collect_call_target_return_values(&self, func_node: &Node<JavaScript>) -> Vec<JavaScript> {
        let mut returns = Vec::new();

        if func_node.kind() == "identifier"
            && let Ok(identifier) = func_node.text()
            && let Some(value) = self.vars.get(identifier)
            && let Some(return_value) = Self::function_return_from_value(value)
        {
            returns.push(return_value);
        }

        if let Ok(member_key) = func_node.text() {
            if let Some(value) = self.member_functions.get(member_key)
                && let Some(return_value) = Self::function_return_from_value(value)
            {
                returns.push(return_value);
            }

            if let Some(alias) = self.member_aliases.get(member_key)
                && let Some(value) = self.vars.get(alias)
                && let Some(return_value) = Self::function_return_from_value(value)
            {
                returns.push(return_value);
            }
        }

        if let Some((base, key)) = Self::extract_member_access(func_node)
            && let Some(value) = self.object_fields.get(&(base, key))
            && let Some(return_value) = Self::function_return_from_value(value)
        {
            returns.push(return_value);
        }

        if let Some(return_value) = func_node.data().and_then(Self::function_return_from_value) {
            returns.push(return_value);
        }

        if let Some(inline_value) = self.resolve_inline_callable_value(func_node)
            && let Some(return_value) = Self::function_return_from_value(&inline_value)
        {
            returns.push(return_value);
        }

        returns
    }

    fn extract_known_call_arg_sources(call_node: &Node<JavaScript>) -> Option<Vec<String>> {
        let mut rendered_args = Vec::new();
        let arguments = call_node.named_child("arguments")?;

        for idx in 0..arguments.child_count() {
            let child = arguments.child(idx)?;
            if matches!(child.kind(), "(" | ")" | ",") {
                continue;
            }

            let value = child.data()?;
            rendered_args.push(value.to_string());
        }

        Some(rendered_args)
    }

    fn is_simple_identifier(name: &str) -> bool {
        let mut chars = name.chars();
        let Some(first) = chars.next() else {
            return false;
        };

        if !(first.is_ascii_alphabetic() || first == '_' || first == '$') {
            return false;
        }

        chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
    }

    fn extract_function_parts(function_source: &str) -> Option<(Vec<String>, String)> {
        let open_paren = function_source.find('(')?;
        let mut paren_depth = 0usize;
        let mut close_paren = None;
        for (idx, ch) in function_source.char_indices().skip(open_paren) {
            match ch {
                '(' => paren_depth += 1,
                ')' => {
                    paren_depth = paren_depth.saturating_sub(1);
                    if paren_depth == 0 {
                        close_paren = Some(idx);
                        break;
                    }
                }
                _ => {}
            }
        }

        let close_paren = close_paren?;
        let params_src = &function_source[open_paren + 1..close_paren];
        let params = params_src
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .filter(|param| Self::is_simple_identifier(param))
            .map(ToString::to_string)
            .collect::<Vec<_>>();

        let open_brace = function_source[close_paren..]
            .find('{')
            .map(|idx| idx + close_paren)?;
        let mut brace_depth = 0usize;
        let mut close_brace = None;
        for (idx, ch) in function_source.char_indices().skip(open_brace) {
            match ch {
                '{' => brace_depth += 1,
                '}' => {
                    brace_depth = brace_depth.saturating_sub(1);
                    if brace_depth == 0 {
                        close_brace = Some(idx);
                        break;
                    }
                }
                _ => {}
            }
        }

        let close_brace = close_brace?;
        let body = function_source[open_brace + 1..close_brace].to_string();
        Some((params, body))
    }

    fn rewrite_body_for_nested_eval(body: &str) -> Option<String> {
        fn is_ident_byte(b: u8) -> bool {
            b.is_ascii_alphanumeric() || b == b'_'
        }

        fn has_top_level_keyword(body: &[u8], keyword: &[u8]) -> bool {
            let mut depth = 0usize;
            let mut i = 0usize;
            while i < body.len() {
                match body[i] {
                    b'{' => depth += 1,
                    b'}' => depth = depth.saturating_sub(1),
                    _ => {}
                }

                if depth == 0
                    && i + keyword.len() <= body.len()
                    && &body[i..i + keyword.len()] == keyword
                    && (i == 0 || !is_ident_byte(body[i - 1]))
                    && (i + keyword.len() == body.len() || !is_ident_byte(body[i + keyword.len()]))
                {
                    return true;
                }

                i += 1;
            }
            false
        }

        let bytes = body.as_bytes();
        if has_top_level_keyword(bytes, b"if")
            || has_top_level_keyword(bytes, b"for")
            || has_top_level_keyword(bytes, b"while")
            || has_top_level_keyword(bytes, b"switch")
            || has_top_level_keyword(bytes, b"try")
        {
            return None;
        }

        let mut depth = 0usize;
        let mut return_start: Option<usize> = None;
        let mut return_expr_end: Option<usize> = None;
        let mut return_stmt_end: Option<usize> = None;

        let mut i = 0usize;
        while i < bytes.len() {
            match bytes[i] {
                b'{' => depth += 1,
                b'}' => depth = depth.saturating_sub(1),
                _ => {}
            }

            if depth == 0
                && i + 6 <= bytes.len()
                && &bytes[i..i + 6] == b"return"
                && (i == 0 || !is_ident_byte(bytes[i - 1]))
                && (i + 6 == bytes.len() || !is_ident_byte(bytes[i + 6]))
            {
                if return_start.is_some() {
                    return None;
                }

                let mut j = i + 6;
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }

                let mut expr_end = j;
                while expr_end < bytes.len() && bytes[expr_end] != b';' {
                    expr_end += 1;
                }

                let stmt_end = if expr_end < bytes.len() {
                    expr_end + 1
                } else {
                    expr_end
                };

                return_start = Some(i);
                return_expr_end = Some(expr_end);
                return_stmt_end = Some(stmt_end);
                i = expr_end;
            }

            i += 1;
        }

        let return_start = return_start?;
        let return_expr_end = return_expr_end?;
        let return_stmt_end = return_stmt_end?;
        let expr_start = return_start + 6;
        let expr = body[expr_start..return_expr_end].trim();
        if expr.is_empty() {
            return None;
        }

        let prefix = &body[..return_start];
        let suffix = &body[return_stmt_end..];

        let mut rewritten = String::new();
        rewritten.push_str(prefix);
        rewritten.push_str("__minusone_result = ");
        rewritten.push_str(expr);
        rewritten.push(';');
        rewritten.push_str(suffix);
        Some(rewritten)
    }

    fn resolve_call_from_injected_body(
        &self,
        call_node: &Node<JavaScript>,
        function_source: &str,
        other_rules: &[RuleReference<JavaScript>],
    ) -> Option<JavaScript> {
        if self.nested_eval_depth >= self.max_nested_eval_depth {
            return None;
        }

        let args = Self::extract_known_call_arg_sources(call_node)?;
        let (params, body) = Self::extract_function_parts(function_source)?;
        let rewritten_body = Self::rewrite_body_for_nested_eval(&body)?;

        let mut nested_source = String::from("let __minusone_result = undefined;\n");
        for (idx, arg) in args.iter().enumerate() {
            nested_source.push_str(format!("let __minusone_arg_{idx} = {arg};\n").as_str());
        }
        for (idx, param) in params.iter().enumerate() {
            if idx < args.len() {
                nested_source.push_str(format!("let {param} = __minusone_arg_{idx};\n").as_str());
            } else {
                nested_source.push_str(format!("let {param} = undefined;\n").as_str());
            }
        }
        nested_source.push_str(&rewritten_body);
        nested_source.push_str("\n__minusone_result;");

        let mut tree = build_javascript_tree(&nested_source).ok()?;
        let inherited_rule_names = self.nested_peer_rule_names.clone().unwrap_or_default();
        let selected_rule_names_owned: Vec<String> = if other_rules.is_empty() {
            inherited_rule_names
        } else {
            other_rules
                .iter()
                .map(|rule| rule.name.to_string())
                .collect()
        };

        let peer_snapshots: HashMap<String, Box<dyn Any>> = if other_rules.is_empty() {
            HashMap::new()
        } else {
            other_rules
                .iter()
                .filter_map(|rule| {
                    rule.rule
                        .snapshot_state()
                        .map(|snapshot| (rule.name.to_string(), snapshot))
                })
                .collect()
        };

        if selected_rule_names_owned.is_empty() {
            warn!(
                "No other rules were passed, falling back to a few rules. This may lead to limited resolution of the injected function body."
            );
            let mut nested_rules = (
                crate::js::integer::ParseInt::default(),
                crate::js::integer::NegInt::default(),
                crate::js::integer::SubAddInt::default(),
                crate::js::integer::MultInt::default(),
                crate::js::string::ParseString::default(),
                crate::js::functions::function::ParseFunction::default(),
                crate::js::array::ParseArray::default(),
                crate::js::objects::object::ParseObject::default(),
                crate::js::forward::Forward::default(),
                crate::js::array::GetArrayElement::default(),
                crate::js::objects::object::ObjectField::default(),
                crate::js::var::Var::default(),
                self.clone_for_nested_eval(),
            );

            tree.apply_mut_with_strategy(&mut nested_rules, JavaScriptStrategy::default())
                .ok()?;
            tree.apply_mut_with_strategy(&mut nested_rules, JavaScriptStrategy::default())
                .ok()?;
            tree.apply_mut_with_strategy(&mut nested_rules, JavaScriptStrategy::default())
                .ok()?;
        } else {
            let mut nested_fncall = self.clone_for_nested_eval();
            nested_fncall.nested_peer_rule_names = Some(selected_rule_names_owned.clone());
            let selected_rule_names: Vec<&str> = selected_rule_names_owned
                .iter()
                .map(String::as_str)
                .collect();

            for _ in 0..2 {
                {
                    let mut nested_rules = JavaScriptRuleSet::new(RuleSetBuilderType::WithRules(
                        selected_rule_names.clone(),
                    ));
                    nested_rules.restore_rule_snapshots(&peer_snapshots);
                    tree.apply_mut_with_strategy(&mut nested_rules, JavaScriptStrategy::default())
                        .ok()?;
                }

                tree.apply_mut_with_strategy(&mut nested_fncall, JavaScriptStrategy::default())
                    .ok()?;
            }
        }

        let root = tree.root().ok()?;
        for idx in (0..root.child_count()).rev() {
            let statement = root.child(idx)?;
            if statement.kind() == "expression_statement" {
                let expr = statement.child(0)?;
                if let Some(value) = expr
                    .data()
                    .cloned()
                    .or_else(|| expr.smallest_child().data().cloned())
                {
                    return Some(value);
                }
            }
        }

        None
    }

    fn resolve_call_expression(
        &self,
        call_node: &Node<JavaScript>,
        other_rules: &[RuleReference<JavaScript>],
    ) -> Option<JavaScript> {
        let func_node = call_node
            .named_child("function")
            .or_else(|| call_node.child(0))?;

        for function_source in self.collect_call_target_sources(&func_node) {
            if let Some(return_value) =
                self.resolve_call_from_injected_body(call_node, &function_source, other_rules)
            {
                return Some(return_value);
            }
        }

        self.collect_call_target_return_values(&func_node)
            .into_iter()
            .next()
    }

    fn track_variable_declarator(&mut self, view: &Node<JavaScript>) -> MinusOneResult<()> {
        let Some(name_node) = view.named_child("name") else {
            return Ok(());
        };

        if name_node.kind() != "identifier" {
            return Ok(());
        }

        let Some(value_node) = view.named_child("value").or_else(|| view.child(2)) else {
            return Ok(());
        };

        let name = name_node.text()?.to_string();
        if let Some(value @ JavaScript::Function { .. }) =
            self.resolve_function_value_from_node(&value_node)
        {
            let value = if Self::is_function_like_kind(value_node.kind()) {
                Self::with_rendered_function_source(value, &value_node)
            } else {
                value
            };
            self.track_function_binding(name, value);
        }

        Ok(())
    }

    fn track_function_declaration(&mut self, view: &Node<JavaScript>) -> MinusOneResult<()> {
        let Some(name_node) = view.named_child("name") else {
            return Ok(());
        };

        if name_node.kind() != "identifier" {
            return Ok(());
        }

        let name = name_node.text()?.to_string();
        if let Some(value @ JavaScript::Function { .. }) =
            self.resolve_function_value_from_node(view)
        {
            let value = Self::with_rendered_function_source(value, view);
            self.track_function_binding(name, value);
        } else if let Ok(source) = view.text() {
            self.function_sources.insert(name, source.to_string());
        }

        Ok(())
    }

    fn track_assignment_expression(&mut self, view: &Node<JavaScript>) -> MinusOneResult<()> {
        let (Some(left), Some(right)) = (view.child(0), view.child(2)) else {
            return Ok(());
        };

        if left.kind() == "identifier" {
            let var_name = left.text()?.to_string();
            if let Some(value @ JavaScript::Function { .. }) =
                self.resolve_function_value_from_node(&right)
            {
                let value = if Self::is_function_like_kind(right.kind()) {
                    Self::with_rendered_function_source(value, &right)
                } else {
                    value
                };
                self.track_function_binding(var_name, value);
            }
            return Ok(());
        }

        if left.kind() != "member_expression" {
            return Ok(());
        }

        let member_key = left.text().ok().map(str::to_string);
        let right_identifier = if right.kind() == "identifier" {
            right.text().ok().map(str::to_string)
        } else {
            None
        };

        if let (Some(member_key), Some(alias)) = (member_key.clone(), right_identifier) {
            self.member_aliases.insert(member_key, alias);
        }

        if let Some(value @ JavaScript::Function { .. }) =
            self.resolve_function_value_from_node(&right)
        {
            let value = if Self::is_function_like_kind(right.kind()) {
                Self::with_rendered_function_source(value, &right)
            } else {
                value
            };

            if let Some(member_key) = member_key {
                self.member_functions.insert(member_key, value.clone());
            }

            if let Some((base, key)) = Self::extract_member_access(&left) {
                self.object_fields.insert((base, key), value);
            }
        }

        Ok(())
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
        if self.reset_on_program_enter && view.kind() == "program" {
            self.clear_state();
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

    fn leave_with_context(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
        context: &RuleExecutionContext<'_, 'a, Self::Language>,
    ) -> MinusOneResult<()> {
        let other_rules = context.other_rules;
        let view = node.view();
        match view.kind() {
            "subscript_expression" => Self::reduce_array_subscript(node),
            "variable_declarator" => self.track_variable_declarator(&view)?,
            "function_declaration" | "generator_function_declaration" => {
                self.track_function_declaration(&view)?;
            }
            "assignment_expression" => self.track_assignment_expression(&view)?,
            "call_expression" => {
                if let Some(return_value) = self.resolve_call_expression(&view, other_rules) {
                    trace!("FnCall (L): Reduced call expression to {:?}", return_value);
                    node.reduce(return_value);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn snapshot_state(&self) -> Option<Box<dyn Any>> {
        Some(Box::new(FnCallSnapshot {
            function_sources: self.function_sources.clone(),
            vars: self.vars.clone(),
            object_fields: self.object_fields.clone(),
            member_functions: self.member_functions.clone(),
            member_aliases: self.member_aliases.clone(),
        }))
    }

    fn restore_state(&mut self, snapshot: &dyn Any) {
        if let Some(snapshot) = snapshot.downcast_ref::<FnCallSnapshot>() {
            self.function_sources = snapshot.function_sources.clone();
            self.vars = snapshot.vars.clone();
            self.object_fields = snapshot.object_fields.clone();
            self.member_functions = snapshot.member_functions.clone();
            self.member_aliases = snapshot.member_aliases.clone();
        }
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
    fn test_fncall_param_injected_eval() {
        assert_eq!(
            deobfuscate("function test(num) { return num + 1; } test(1);"),
            "function test(num) { return num + 1; } 2;"
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
            "let a = {}; let x = function (params) { return 0; } a.t = x; console.log(a.t());"
        );
    }

    #[test]
    fn test_fncall_object_stored_function_param_dependent_return() {
        assert_eq!(
            deobfuscate(
                "let a = {};
let x = function (n) {
    return n + 1;
}
a.t = x;
console.log(a.t(1));
console.log(a.t(2));"
            ),
            "let a = {};
let x = function (n) {
    return n + 1;
}
a.t = x;
console.log(2);
console.log(3);"
        );
    }

    #[test]
    fn test_fncall_alias_array_subscript() {
        let output = deobfuscate(
            "function _0x51e9(_0x2aa0e4, _0x111bec) { _0x2aa0e4 = _0x2aa0e4 - 0; var _0x361bb4 = ['log', '0 1 2 3 4 5 6 7 8 9'][_0x2aa0e4]; return _0x361bb4; } var _0x5aa3f9 = _0x51e9; console[_0x5aa3f9(0)](_0x5aa3f9(1));",
        );

        assert!(
            output.ends_with("console['log']('0 1 2 3 4 5 6 7 8 9');")
                || output.ends_with("console.log('0 1 2 3 4 5 6 7 8 9');"),
            "unexpected output: {output}"
        );
    }

    #[test]
    fn test_fncall_single_pass_reduction() {
        let output = deobfuscate(
            "function _0x4c7c() { var _0x4c602b = ['log', '0\\x201\\x202\\x203\\x204\\x205\\x206\\x207\\x208\\x209']; _0x4c7c = function () { return _0x4c602b; }; return _0x4c7c(); } function _0x51e9(_0x2aa0e4, _0x111bec) { _0x2aa0e4 = _0x2aa0e4 - (0xbcc * 0x2 + 0x3 * 0x959 + -0x33a3); var _0x243f78 = _0x4c7c(); var _0x361bb4 = _0x243f78[_0x2aa0e4]; return _0x361bb4; } var _0x5aa3f9 = _0x51e9; console[_0x5aa3f9(0x0)](_0x5aa3f9(0x1));",
        );

        assert!(
            output.ends_with("console['log']('0 1 2 3 4 5 6 7 8 9');")
                || output.ends_with("console.log('0 1 2 3 4 5 6 7 8 9');")
        );
    }

    #[test]
    fn test_fncall_self_redefining_function() {
        let output = deobfuscate(
            "function _0x45a5(){return(_0x45a5=function(){return'minusone'})()}console.log(_0x45a5());",
        );

        assert!(
            output.ends_with("console.log('minusone');"),
            "unexpected output: {output}"
        );
    }
}
