use crate::error::MinusOneResult;
use crate::js::r#switch::simplify_switch_statement_text;
use crate::rule::Rule;
use crate::tree::Node;
use log::trace;
use std::collections::HashMap;

/// Tracks which JavaScript identifiers are actually read (used) vs only written to (declared/assigned).
#[derive(Default)]
pub struct UnusedVar {
    reads: HashMap<String, bool>,
}

impl UnusedVar {
    pub fn is_unused(&self, var_name: &str) -> bool {
        !self.reads.get(var_name).copied().unwrap_or(false)
    }
}

impl<'a> Rule<'a> for UnusedVar {
    type Language = ();

    fn enter(&mut self, _node: &Node<'a, Self::Language>) -> MinusOneResult<bool> {
        Ok(true)
    }

    fn leave(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<()> {
        if node.kind() == "identifier" {
            let name = node.text()?.to_string();

            if is_write_target(node) {
                self.reads.entry(name).or_insert(false);
            } else {
                trace!("UnusedVar: '{}' is read", name);
                self.reads.insert(name, true);
            }
        }
        Ok(())
    }
}

fn is_write_target<T>(node: &Node<T>) -> bool {
    if let Some(parent) = node.parent() {
        match parent.kind() {
            "variable_declarator" => {
                // first child of variable_declarator is the name
                if let Some(name_child) = parent.child(0) {
                    return name_child.id() == node.id();
                }
            }
            "assignment_expression" | "augmented_assignment_expression" => {
                if let Some(left) = parent.child(0) {
                    return left.id() == node.id();
                }
            }
            "update_expression" => {
                return true;
            }
            // function name in a function_declaration is a write (definition)
            "function_declaration" => {
                if let Some(name_child) = parent.named_child("name") {
                    return name_child.id() == node.id();
                }
            }
            "formal_parameters" => {
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Rewrites canonical `for` loops with empty init/increment into equivalent `while` loops.
///
/// # Example
/// ```
/// use minusone::js::post_process::ForToWhile;
/// use minusone::js::build_javascript_tree_for_storage;
/// use minusone::tree::EmptyStorage;
///
/// let source = "for (; a != b;) { console.log(a); }";
/// let tree = build_javascript_tree_for_storage::<EmptyStorage>(source).unwrap();
///
/// let mut for_to_while = ForToWhile::default();
/// tree.apply(&mut for_to_while).unwrap();
///
/// assert_eq!(for_to_while.clear().unwrap(), "while (a != b) { console.log(a); }");
/// ```
#[derive(Default)]
pub struct ForToWhile {
    source: String,
    output: String,
    last_index: usize,
}

impl ForToWhile {
    pub fn clear(mut self) -> MinusOneResult<String> {
        if self.last_index < self.source.len() {
            self.output += &self.source[self.last_index..];
        }
        Ok(self.output)
    }

    fn copy_until(&mut self, end: usize) {
        let safe_end = end.min(self.source.len());
        if safe_end > self.last_index {
            self.output += &self.source[self.last_index..safe_end];
            self.last_index = safe_end;
        }
    }

    fn replace_node_with_text(&mut self, node: &Node<()>, replacement: &str) {
        let start = node.start_abs().min(self.source.len());
        let end = node.end_abs().min(self.source.len());

        if start < self.last_index || end <= self.last_index || end <= start {
            return;
        }

        while self.last_index < start && self.source.as_bytes().get(self.last_index) == Some(&b'\n')
        {
            self.last_index += 1;
        }

        self.copy_until(start);
        self.output += replacement;
        if self.source.as_bytes().get(end) == Some(&b'\n') {
            self.output += "\n";
        }
        self.last_index = end;
    }

    fn for_statement_to_while_text(node: &Node<()>) -> Option<String> {
        if node.kind() != "for_statement" {
            return None;
        }

        // `for (;;) {}` -> [empty_statement, empty_statement]
        // `for (; a != b;) {}` -> [empty_statement, binary_expression, empty_statement]
        let mut in_header = false;
        let mut header_parts: Vec<Node<()>> = Vec::new();
        let mut body: Option<Node<()>> = None;

        for child in node.iter() {
            match child.kind() {
                "(" => in_header = true,
                ")" => in_header = false,
                _ => {
                    if in_header {
                        header_parts.push(child);
                    } else if body.is_none()
                        && (child.kind().ends_with("statement")
                            || child.kind() == "statement_block")
                    {
                        body = Some(child);
                    }
                }
            }
        }

        let condition_text = match header_parts.as_slice() {
            [first, second]
                if first.kind() == "empty_statement" && second.kind() == "empty_statement" =>
            {
                "true".to_string()
            }
            [first, condition] if first.kind() == "empty_statement" => {
                condition.text().ok()?.trim().to_string()
            }
            [first, condition, third]
                if first.kind() == "empty_statement"
                    && (third.kind() == "empty_statement" || third.kind() == ";") =>
            {
                condition.text().ok()?.trim().to_string()
            }
            _ => return None,
        };

        let body = body?;
        let body_text = body.text().ok()?.trim();
        let condition_text = if condition_text.is_empty() {
            "true".to_string()
        } else {
            condition_text
        };

        Some(format!("while ({}) {}", condition_text, body_text))
    }
}

impl<'a> Rule<'a> for ForToWhile {
    type Language = ();

    fn enter(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<bool> {
        match node.kind() {
            "program" => {
                self.source = node.text()?.to_string();
                self.last_index = 0;
            }
            "for_statement" => {
                if let Some(replacement) = Self::for_statement_to_while_text(node) {
                    trace!("ForToWhile: rewriting for loop to while loop");
                    self.replace_node_with_text(node, &replacement);
                    return Ok(false);
                }
            }
            _ => {}
        }
        Ok(true)
    }

    fn leave(&mut self, _node: &Node<'a, Self::Language>) -> MinusOneResult<()> {
        Ok(())
    }
}

/// Rewrites bracket member calls with static string keys into dot member calls.
///
/// # Example
/// ```
/// use minusone::js::post_process::BracketCallToMember;
/// use minusone::js::build_javascript_tree_for_storage;
/// use minusone::tree::EmptyStorage;
///
/// let source = "console['log']('minusone');";
/// let tree = build_javascript_tree_for_storage::<EmptyStorage>(source).unwrap();
///
/// let mut bracket_to_member = BracketCallToMember::default();
/// tree.apply(&mut bracket_to_member).unwrap();
///
/// assert_eq!(bracket_to_member.clear().unwrap(), "console.log('minusone');");
/// ```
#[derive(Default)]
pub struct BracketCallToMember {
    source: String,
    output: String,
    last_index: usize,
}

impl BracketCallToMember {
    pub fn clear(mut self) -> MinusOneResult<String> {
        if self.last_index < self.source.len() {
            self.output += &self.source[self.last_index..];
        }
        Ok(self.output)
    }

    fn copy_until(&mut self, end: usize) {
        let safe_end = end.min(self.source.len());
        if safe_end > self.last_index {
            self.output += &self.source[self.last_index..safe_end];
            self.last_index = safe_end;
        }
    }

    fn replace_node_with_text(&mut self, node: &Node<()>, replacement: &str) {
        let start = node.start_abs().min(self.source.len());
        let end = node.end_abs().min(self.source.len());

        if start < self.last_index || end <= self.last_index || end <= start {
            return;
        }

        while self.last_index < start && self.source.as_bytes().get(self.last_index) == Some(&b'\n')
        {
            self.last_index += 1;
        }

        self.copy_until(start);
        self.output += replacement;
        if self.source.as_bytes().get(end) == Some(&b'\n') {
            self.output += "\n";
        }
        self.last_index = end;
    }

    fn parse_simple_string_literal(text: &str) -> Option<String> {
        let bytes = text.as_bytes();
        if bytes.len() < 2 {
            return None;
        }

        let quote = *bytes.first()?;
        if (quote != b'\'' && quote != b'"') || *bytes.last()? != quote {
            return None;
        }

        let inner = &text[1..text.len() - 1];
        if inner.contains('\\') {
            return None;
        }

        Some(inner.to_string())
    }

    fn is_ascii_identifier_name(name: &str) -> bool {
        let mut chars = name.chars();
        let Some(first) = chars.next() else {
            return false;
        };

        if !(first == '_' || first == '$' || first.is_ascii_alphabetic()) {
            return false;
        }

        chars.all(|c| c == '_' || c == '$' || c.is_ascii_alphanumeric())
    }

    fn bracket_call_to_member_text(node: &Node<()>) -> Option<String> {
        if node.kind() != "call_expression" {
            return None;
        }

        let callee = node.child(0)?;
        if callee.kind() != "subscript_expression" {
            return None;
        }

        let object = callee.child(0)?;
        let index = callee.named_child("index").or_else(|| callee.child(2))?;
        if index.kind() != "string" {
            return None;
        }

        let key = Self::parse_simple_string_literal(index.text().ok()?.trim())?;
        if !Self::is_ascii_identifier_name(&key) {
            return None;
        }

        let args = node.child(1)?;
        if args.kind() != "arguments" {
            return None;
        }

        Some(format!(
            "{}.{}{}",
            object.text().ok()?.trim(),
            key,
            args.text().ok()?.trim()
        ))
    }
}

impl<'a> Rule<'a> for BracketCallToMember {
    type Language = ();

    fn enter(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<bool> {
        match node.kind() {
            "program" => {
                self.source = node.text()?.to_string();
                self.last_index = 0;
            }
            "call_expression" => {
                if let Some(replacement) = Self::bracket_call_to_member_text(node) {
                    trace!("BracketCallToMember: rewriting bracket call to member call");
                    self.replace_node_with_text(node, &replacement);
                    return Ok(false);
                }
            }
            _ => {}
        }
        Ok(true)
    }

    fn leave(&mut self, _node: &Node<'a, Self::Language>) -> MinusOneResult<()> {
        Ok(())
    }
}

/// Removes variable declarations, assignments, and function declarations where the declared name is never read.
///
/// # Example
/// ```
/// use minusone::js::post_process::{UnusedVar, RemoveUnused};
/// use minusone::js::build_javascript_tree_for_storage;
/// use minusone::tree::EmptyStorage;
///
/// let source = "var a = 'hello'; console.log('world');";
/// let tree = build_javascript_tree_for_storage::<EmptyStorage>(source).unwrap();
///
/// let mut unused = UnusedVar::default();
/// tree.apply(&mut unused).unwrap();
///
/// let mut remover = RemoveUnused::new(unused);
/// tree.apply(&mut remover).unwrap();
///
/// assert_eq!(remover.clear().unwrap(), "console.log('world');");
/// ```
pub struct RemoveUnused {
    rule: UnusedVar,
    source: String,
    output: String,
    last_index: usize,
}

impl RemoveUnused {
    pub fn new(rule: UnusedVar) -> Self {
        Self {
            rule,
            source: String::new(),
            output: String::new(),
            last_index: 0,
        }
    }

    pub fn clear(mut self) -> MinusOneResult<String> {
        if self.last_index < self.source.len() {
            self.output += &self.source[self.last_index..];
        }
        Ok(self.output)
    }

    fn copy_until(&mut self, end: usize) {
        let safe_end = end.min(self.source.len());
        if safe_end > self.last_index {
            self.output += &self.source[self.last_index..safe_end];
            self.last_index = safe_end;
        }
    }

    fn remove_node<T>(&mut self, node: &Node<T>) -> MinusOneResult<()> {
        let start = node.start_abs().min(self.source.len());
        let end = node.end_abs().min(self.source.len());

        if start < self.last_index || end <= self.last_index || end <= start {
            return Ok(());
        }

        // skip leading newlines before the removed node, but never past node start
        while self.last_index < start && self.source.as_bytes().get(self.last_index) == Some(&b'\n')
        {
            self.last_index += 1;
        }

        self.copy_until(start);
        self.last_index = end;

        // skip trailing whitespace
        while matches!(
            self.source.as_bytes().get(self.last_index),
            Some(b' ') | Some(b'\t')
        ) {
            self.last_index += 1;
        }

        if self.source.as_bytes().get(self.last_index) == Some(&b'\n') {
            self.last_index += 1;
        }
        Ok(())
    }

    fn single_declarator_name(node: &Node<()>) -> Option<String> {
        let mut name = None;
        let mut count = 0;
        for child in node.iter() {
            if child.kind() == "variable_declarator" {
                count += 1;
                if count > 1 {
                    return None;
                }
                if let Some(name_node) = child.named_child("name")
                    && name_node.kind() == "identifier"
                {
                    name = Some(name_node.text().ok()?.to_string());
                }
            }
        }
        name
    }

    fn declaration_keyword(node: &Node<()>) -> Option<&'static str> {
        match node.kind() {
            "variable_declaration" => Some("var"),
            "lexical_declaration" => {
                for child in node.iter() {
                    match child.kind() {
                        "let" => return Some("let"),
                        "const" => return Some("const"),
                        _ => {}
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn in_for_header(node: &Node<()>) -> bool {
        node.parent().is_some_and(|parent| {
            matches!(
                parent.kind(),
                "for_statement" | "for_in_statement" | "for_of_statement"
            )
        })
    }

    fn split_declaration_text(node: &Node<()>) -> Option<String> {
        let keyword = Self::declaration_keyword(node)?;
        let mut declarators = Vec::new();

        for child in node.iter() {
            if child.kind() == "variable_declarator" {
                declarators.push(child.text().ok()?.trim().to_string());
            }
        }

        if declarators.len() <= 1 {
            return None;
        }

        Some(
            declarators
                .into_iter()
                .map(|decl| format!("{} {};", keyword, decl))
                .collect::<Vec<_>>()
                .join(" "),
        )
    }

    fn split_sequence_statement_text(node: &Node<()>) -> Option<String> {
        if node.kind() != "expression_statement" {
            return None;
        }

        let expr = node.child(0)?;
        if expr.kind() != "sequence_expression" {
            return None;
        }

        let mut parts = Vec::new();
        for child in expr.iter() {
            if child.kind() == "," {
                continue;
            }
            parts.push(format!("{};", child.text().ok()?.trim()));
        }

        if parts.len() <= 1 {
            return None;
        }

        Some(parts.join(" "))
    }

    fn is_literal_bool(node: &Node<()>, expected: bool) -> bool {
        let kind = if expected { "true" } else { "false" };

        match node.kind() {
            k if k == kind => true,
            "parenthesized_expression" => node.iter().any(|child| child.kind() == kind),
            _ => false,
        }
    }

    fn has_else_clause(node: &Node<()>) -> bool {
        for child in node.iter() {
            if child.kind() == "else_clause" {
                return true;
            }
        }
        false
    }

    fn block_inner_text(block: &Node<()>) -> Option<String> {
        let text = block.text().ok()?;
        let trimmed = text.trim();
        // strip surrounding { }
        if trimmed.starts_with('{') && trimmed.ends_with('}') {
            Some(trimmed[1..trimmed.len() - 1].trim().to_string())
        } else {
            None
        }
    }

    fn statement_block_is_empty(block: &Node<()>) -> bool {
        Self::block_inner_text(block)
            .map(|inner| inner.trim().is_empty())
            .unwrap_or(false)
    }

    fn else_clause_body_text(else_clause: &Node<()>) -> Option<String> {
        for child in else_clause.iter() {
            if child.kind() == "else" {
                continue;
            }
            return Some(child.text().ok()?.trim().to_string());
        }
        None
    }

    fn replace_node_with_text(&mut self, node: &Node<()>, replacement: &str) -> MinusOneResult<()> {
        let start = node.start_abs().min(self.source.len());
        let end = node.end_abs().min(self.source.len());

        // parent nodes can be replaced before children are visited.
        if start < self.last_index || end <= self.last_index || end <= start {
            return Ok(());
        }

        // skip leading newlines, but never past node start
        while self.last_index < start && self.source.as_bytes().get(self.last_index) == Some(&b'\n')
        {
            self.last_index += 1;
        }

        self.copy_until(start);
        self.output += replacement;
        if self.source.as_bytes().get(end) == Some(&b'\n') {
            self.output += "\n";
        }
        self.last_index = end;
        Ok(())
    }

    fn trim_output_trailing_space(&mut self) {
        if self.output.ends_with(' ') {
            self.output.pop();
        }
    }

    fn negate_condition_text(text: &str) -> String {
        let trimmed = text.trim();
        if trimmed.starts_with('(') && trimmed.ends_with(')') {
            format!("!{}", trimmed)
        } else {
            format!("!({})", trimmed)
        }
    }
}

impl<'a> Rule<'a> for RemoveUnused {
    type Language = ();

    fn enter(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<bool> {
        match node.kind() {
            "program" => {
                self.source = node.text()?.to_string();
                self.last_index = 0;
            }
            // var x = ...; / let x = ...; / const x = ...;
            "variable_declaration" | "lexical_declaration" => {
                // split chained declarations so clean output emits one var/let/const per statement.
                if !Self::in_for_header(node)
                    && let Some(replacement) = Self::split_declaration_text(node)
                {
                    trace!("RemoveUnusedVar: splitting chained declaration");
                    self.replace_node_with_text(node, &replacement)?;
                    return Ok(false);
                }

                if let Some(var_name) = Self::single_declarator_name(node)
                    && self.rule.is_unused(&var_name)
                {
                    trace!("RemoveUnusedVar: removing declaration of '{}'", var_name);
                    self.remove_node(node)?;
                    return Ok(false);
                }
            }
            // x = ...;  (expression_statement wrapping an assignment_expression)
            // also removes bare literal expression statements (e.g. 1;, 'hello';, true;)
            "expression_statement" => {
                if let Some(replacement) = Self::split_sequence_statement_text(node) {
                    trace!("RemoveUnusedVar: splitting comma sequence expression statement");
                    self.replace_node_with_text(node, &replacement)?;
                    return Ok(false);
                }

                if let Some(child) = node.child(0) {
                    match child.kind() {
                        "assignment_expression" => {
                            if let Some(left) = child.child(0)
                                && left.kind() == "identifier"
                            {
                                let var_name = left.text()?.to_string();
                                if self.rule.is_unused(&var_name) {
                                    trace!(
                                        "RemoveUnusedVar: removing assignment to '{}'",
                                        var_name
                                    );
                                    self.remove_node(node)?;
                                    return Ok(false);
                                }
                            }
                        }
                        // bare literals are dead code
                        "number" | "string" | "true" | "false" | "null" | "undefined" => {
                            trace!(
                                "RemoveUnusedVar: removing bare literal statement '{}'",
                                child.text().unwrap_or("?")
                            );
                            self.remove_node(node)?;
                            return Ok(false);
                        }
                        _ => {}
                    }
                }
            }
            // function foo() { ... }  where foo is never called
            "function_declaration" => {
                if let Some(name_node) = node.named_child("name")
                    && name_node.kind() == "identifier"
                {
                    let fn_name = name_node.text()?.to_string();
                    if self.rule.is_unused(&fn_name) {
                        trace!(
                            "RemoveUnusedVar: removing function declaration '{}'",
                            fn_name
                        );
                        self.remove_node(node)?;
                        return Ok(false);
                    }
                }
            }
            // if statements with known boolean conditions
            "if_statement" => {
                if let Some(condition) = node.named_child("condition") {
                    let consequence = node.named_child("consequence");
                    let consequence_empty = consequence
                        .as_ref()
                        .is_some_and(|block| block.kind() == "statement_block")
                        && consequence
                            .as_ref()
                            .is_some_and(Self::statement_block_is_empty);

                    if consequence_empty {
                        let mut else_body = None;
                        for child in node.iter() {
                            if child.kind() == "else_clause" {
                                else_body = Self::else_clause_body_text(&child);
                                if let Some(body) = &else_body {
                                    if body.trim().is_empty() {
                                        trace!("RemoveUnusedVar: removing empty else clause");
                                        self.remove_node(&child)?;
                                        return Ok(false);
                                    }
                                }
                                break;
                            }
                        }

                        if let Some(body) = else_body {
                            if Self::is_literal_bool(&condition, true) {
                                trace!("RemoveUnusedVar: removing if (true) with empty body");
                                self.remove_node(node)?;
                                return Ok(false);
                            } else if Self::is_literal_bool(&condition, false) {
                                trace!(
                                    "RemoveUnusedVar: replacing if (false) ... else with else body"
                                );
                                self.replace_node_with_text(node, &body)?;
                                return Ok(false);
                            } else {
                                let cond_text = condition.text()?;
                                let negated = Self::negate_condition_text(&cond_text);
                                let replacement = format!("if ({}) {}", negated, body);
                                trace!("RemoveUnusedVar: flipping empty if into negated else");
                                self.replace_node_with_text(node, &replacement)?;
                                return Ok(false);
                            }
                        } else {
                            trace!("RemoveUnusedVar: removing empty if statement");
                            self.remove_node(node)?;
                            return Ok(false);
                        }
                    }

                    if Self::is_literal_bool(&condition, false) {
                        if Self::has_else_clause(node) {
                            // if (false) { ... } else { BODY } -> keep BODY
                            for child in node.iter() {
                                if child.kind() == "else_clause" {
                                    for else_child in child.iter() {
                                        if else_child.kind() == "statement_block"
                                            && let Some(inner) = Self::block_inner_text(&else_child)
                                        {
                                            trace!(
                                                "RemoveUnusedVar: replacing if (false) ... else with else body"
                                            );
                                            self.replace_node_with_text(node, &inner)?;
                                            return Ok(false);
                                        }
                                    }
                                }
                            }
                        } else {
                            // if (false) { ... } with no else -> remove entirely
                            trace!("RemoveUnusedVar: removing dead if (false) statement");
                            self.remove_node(node)?;
                            return Ok(false);
                        }
                    } else if Self::is_literal_bool(&condition, true) {
                        // if (true) { BODY } ... -> keep BODY, discard else
                        if let Some(consequence) = node.named_child("consequence")
                            && let Some(inner) = Self::block_inner_text(&consequence)
                        {
                            trace!("RemoveUnusedVar: replacing if (true) with if body");
                            self.replace_node_with_text(node, &inner)?;
                            return Ok(false);
                        }
                    }
                }
            }
            "switch_statement" => {
                if let Some(replacement) = simplify_switch_statement_text(node) {
                    if replacement.is_empty() {
                        trace!("RemoveUnusedVar: removing empty deterministic switch");
                        self.remove_node(node)?;
                    } else {
                        trace!("RemoveUnusedVar: simplifying deterministic switch");
                        self.replace_node_with_text(node, &replacement)?;
                    }
                    return Ok(false);
                }
            }
            "while_statement" => {
                if let Some(condition) = node.named_child("condition") {
                    if Self::is_literal_bool(&condition, false) {
                        trace!("RemoveUnusedVar: removing dead while (false) loop");
                        self.remove_node(node)?;
                        return Ok(false);
                    }
                }
            }
            "else_clause" => {
                for child in node.iter() {
                    if child.kind() == "statement_block" && Self::statement_block_is_empty(&child) {
                        trace!("RemoveUnusedVar: removing empty else clause");
                        self.remove_node(node)?;
                        self.trim_output_trailing_space();
                        return Ok(false);
                    }
                }
            }
            "try_statement" => {
                let try_block = node.named_child("body");
                let try_empty = try_block
                    .as_ref()
                    .is_some_and(|block| block.kind() == "statement_block")
                    && try_block
                        .as_ref()
                        .is_some_and(Self::statement_block_is_empty);

                let mut catch_empty = true;
                let mut saw_catch = false;
                let mut finally_text = None;

                for child in node.iter() {
                    if child.kind() == "catch_clause" {
                        saw_catch = true;
                        for catch_child in child.iter() {
                            if catch_child.kind() == "statement_block" {
                                catch_empty = Self::statement_block_is_empty(&catch_child);
                                break;
                            }
                        }
                    } else if child.kind() == "finally_clause" {
                        for finally_child in child.iter() {
                            if finally_child.kind() == "statement_block" {
                                finally_text = Self::block_inner_text(&finally_child);
                                break;
                            }
                        }
                    }
                }

                if saw_catch && !catch_empty {
                    catch_empty = false;
                }

                if try_empty && catch_empty {
                    if let Some(inner) = finally_text {
                        trace!("RemoveUnusedVar: unwrapping finally-only try statement");
                        self.replace_node_with_text(node, &inner)?;
                        return Ok(false);
                    }

                    trace!("RemoveUnusedVar: removing empty try statement");
                    self.remove_node(node)?;
                    return Ok(false);
                }
            }
            _ => {}
        }
        Ok(true)
    }

    fn leave(&mut self, _node: &Node<'a, Self::Language>) -> MinusOneResult<()> {
        Ok(())
    }
}

/// Inlines immediately invoked function expressions (IIFEs) with no parameters and no return value
/// by replacing the call with the function body statements.
///
/// # Example
/// ```
/// use minusone::js::post_process::InlineIife;
/// use minusone::js::build_javascript_tree_for_storage;
/// use minusone::tree::EmptyStorage;
///
/// let source = "(function () {console.log('minusone')})()";
/// let tree = build_javascript_tree_for_storage::<EmptyStorage>(source).unwrap();
///
/// let mut inliner = InlineIife::default();
/// tree.apply(&mut inliner).unwrap();
///
/// assert_eq!(inliner.clear().unwrap(), "console.log('minusone')");
/// ```
#[derive(Default)]
pub struct InlineIife {
    source: String,
    output: String,
    last_index: usize,
}

impl InlineIife {
    pub fn clear(mut self) -> MinusOneResult<String> {
        if self.last_index < self.source.len() {
            self.output += &self.source[self.last_index..];
        }
        Ok(self.output)
    }

    fn copy_until(&mut self, end: usize) {
        let safe_end = end.min(self.source.len());
        if safe_end > self.last_index {
            self.output += &self.source[self.last_index..safe_end];
            self.last_index = safe_end;
        }
    }

    fn replace_node_with_text(&mut self, node: &Node<()>, replacement: &str) {
        let start = node.start_abs().min(self.source.len());
        let end = node.end_abs().min(self.source.len());

        if start < self.last_index || end <= self.last_index || end <= start {
            return;
        }

        while self.last_index < start && self.source.as_bytes().get(self.last_index) == Some(&b'\n')
        {
            self.last_index += 1;
        }

        self.copy_until(start);
        self.output += replacement;
        if self.source.as_bytes().get(end) == Some(&b'\n') {
            self.output += "\n";
        }
        self.last_index = end;
    }

    fn compact(text: &str) -> String {
        text.chars().filter(|c| !c.is_ascii_whitespace()).collect()
    }

    fn statement_block_inner_text(block: &Node<()>) -> Option<String> {
        let text = block.text().ok()?;
        let trimmed = text.trim();
        if trimmed.starts_with('{') && trimmed.ends_with('}') {
            Some(trimmed[1..trimmed.len() - 1].trim().to_string())
        } else {
            None
        }
    }

    fn has_named_args(args: &Node<()>) -> bool {
        args.iter()
            .any(|child| !matches!(child.kind(), "(" | ")" | ","))
    }

    fn has_params(params: &Node<()>) -> bool {
        Self::compact(params.text().unwrap_or("")) != "()"
    }

    fn unwrap_parenthesized_function(mut node: Node<()>) -> Option<Node<()>> {
        loop {
            match node.kind() {
                "function_expression" => return Some(node),
                "parenthesized_expression" => {
                    node = node.iter().find(|child| {
                        child.kind() != "(" && child.kind() != ")" && child.kind() != ";"
                    })?;
                }
                _ => return None,
            }
        }
    }

    fn iife_to_body_text(node: &Node<()>) -> Option<String> {
        if node.kind() != "expression_statement" {
            return None;
        }

        let call = node.child(0)?;
        if call.kind() != "call_expression" {
            return None;
        }

        let args = call.child(1)?;
        if args.kind() != "arguments" || Self::has_named_args(&args) {
            return None;
        }

        let callee = call.child(0)?;
        let function_expr = Self::unwrap_parenthesized_function(callee)?;

        // keep named function expressions unchanged because they might be self-recursive
        if function_expr.named_child("name").is_some() {
            return None;
        }

        let parameters = function_expr.named_child("parameters")?;
        if parameters.kind() != "formal_parameters" || Self::has_params(&parameters) {
            return None;
        }

        let body = function_expr.named_child("body")?;
        if body.kind() != "statement_block" {
            return None;
        }

        // inlining a `return` will break the parent function
        if body.iter().any(|child| child.kind() == "return_statement") {
            return None;
        }

        Self::statement_block_inner_text(&body)
    }
}

impl<'a> Rule<'a> for InlineIife {
    type Language = ();

    fn enter(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<bool> {
        match node.kind() {
            "program" => {
                self.source = node.text()?.to_string();
                self.last_index = 0;
            }
            "expression_statement" => {
                if let Some(replacement) = Self::iife_to_body_text(node) {
                    trace!("InlineIife: rewriting anonymous IIFE to statement body");
                    self.replace_node_with_text(node, &replacement);
                    return Ok(false);
                }
            }
            _ => {}
        }
        Ok(true)
    }

    fn leave(&mut self, _node: &Node<'a, Self::Language>) -> MinusOneResult<()> {
        Ok(())
    }
}

/// Rewrites simple augmented assignments into explicit binary assignments.
///
/// # Example
/// ```
/// use minusone::js::post_process::ExpandAugmentedAssignment;
/// use minusone::js::build_javascript_tree_for_storage;
/// use minusone::tree::EmptyStorage;
///
/// let source = "a += 2;";
/// let tree = build_javascript_tree_for_storage::<EmptyStorage>(source).unwrap();
///
/// let mut rewrite = ExpandAugmentedAssignment::default();
/// tree.apply(&mut rewrite).unwrap();
///
/// assert_eq!(rewrite.clear().unwrap(), "a = a + 2;");
/// ```
#[derive(Default)]
pub struct ExpandAugmentedAssignment {
    source: String,
    output: String,
    last_index: usize,
}

impl ExpandAugmentedAssignment {
    pub fn clear(mut self) -> MinusOneResult<String> {
        if self.last_index < self.source.len() {
            self.output += &self.source[self.last_index..];
        }
        Ok(self.output)
    }

    fn copy_until(&mut self, end: usize) {
        let safe_end = end.min(self.source.len());
        if safe_end > self.last_index {
            self.output += &self.source[self.last_index..safe_end];
            self.last_index = safe_end;
        }
    }

    fn replace_node_with_text(&mut self, node: &Node<()>, replacement: &str) {
        let start = node.start_abs().min(self.source.len());
        let end = node.end_abs().min(self.source.len());

        if start < self.last_index || end <= self.last_index || end <= start {
            return;
        }

        while self.last_index < start && self.source.as_bytes().get(self.last_index) == Some(&b'\n')
        {
            self.last_index += 1;
        }

        self.copy_until(start);
        self.output += replacement;
        if self.source.as_bytes().get(end) == Some(&b'\n') {
            self.output += "\n";
        }
        self.last_index = end;
    }

    fn augmented_to_assignment_text(node: &Node<()>) -> Option<String> {
        if node.kind() != "augmented_assignment_expression" {
            return None;
        }

        let left = node.child(0)?;
        if left.kind() != "identifier" {
            return None;
        }

        let op_node = node.child(1)?;
        let op_text = op_node.text().ok()?;
        let binary_op = match op_text.trim() {
            "+=" => "+",
            "-=" => "-",
            "*=" => "*",
            "/=" => "/",
            "%=" => "%",
            _ => return None,
        };

        let right = node.child(2)?;
        let left_text = left.text().ok()?.trim();
        let right_text = right.text().ok()?.trim();

        Some(format!(
            "{} = {} {} {}",
            left_text, left_text, binary_op, right_text
        ))
    }
}

impl<'a> Rule<'a> for ExpandAugmentedAssignment {
    type Language = ();

    fn enter(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<bool> {
        match node.kind() {
            "program" => {
                self.source = node.text()?.to_string();
                self.last_index = 0;
            }
            "augmented_assignment_expression" => {
                if let Some(replacement) = Self::augmented_to_assignment_text(node) {
                    trace!("ExpandAugmentedAssignment: rewriting augmented assignment");
                    self.replace_node_with_text(node, &replacement);
                    return Ok(false);
                }
            }
            _ => {}
        }
        Ok(true)
    }

    fn leave(&mut self, _node: &Node<'a, Self::Language>) -> MinusOneResult<()> {
        Ok(())
    }
}

/// Reduces safe comma sequence expressions to their last expression.
///
/// # Example
/// ```
/// use minusone::js::post_process::ReduceSequenceExpression;
/// use minusone::js::build_javascript_tree_for_storage;
/// use minusone::js::build_javascript_tree;
/// use minusone::js::forward::Forward;
/// use minusone::js::integer::ParseInt;
/// use minusone::js::linter::Linter;
/// use minusone::tree::EmptyStorage;
///
/// let source = "var a = (1, 2, 3);";
/// let tree = build_javascript_tree_for_storage::<EmptyStorage>(source).unwrap();
///
/// let mut reduce_sequence = ReduceSequenceExpression::default();
/// tree.apply(&mut reduce_sequence).unwrap();
///
/// let reduced = reduce_sequence.clear().unwrap();
/// let mut tree = build_javascript_tree(&reduced).unwrap();
/// // Forward still needs to be applied to remove the useless parentheses
/// tree.apply_mut(&mut (ParseInt::default(), Forward::default())).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "var a = 3;");
/// ```
#[derive(Default)]
pub struct ReduceSequenceExpression {
    source: String,
    output: String,
    last_index: usize,
}

impl ReduceSequenceExpression {
    pub fn clear(mut self) -> MinusOneResult<String> {
        if self.last_index < self.source.len() {
            self.output += &self.source[self.last_index..];
        }
        Ok(self.output)
    }

    fn copy_until(&mut self, end: usize) {
        let safe_end = end.min(self.source.len());
        if safe_end > self.last_index {
            self.output += &self.source[self.last_index..safe_end];
            self.last_index = safe_end;
        }
    }

    fn replace_node_with_text(&mut self, node: &Node<()>, replacement: &str) {
        let start = node.start_abs().min(self.source.len());
        let end = node.end_abs().min(self.source.len());

        if start < self.last_index || end <= self.last_index || end <= start {
            return;
        }

        while self.last_index < start && self.source.as_bytes().get(self.last_index) == Some(&b'\n')
        {
            self.last_index += 1;
        }

        self.copy_until(start);
        self.output += replacement;
        if self.source.as_bytes().get(end) == Some(&b'\n') {
            self.output += "\n";
        }
        self.last_index = end;
    }

    fn expression_parts<'a>(node: &Node<'a, ()>) -> Vec<Node<'a, ()>> {
        node.iter().filter(|child| child.kind() != ",").collect()
    }

    fn literal_like_kind(kind: &str) -> bool {
        matches!(
            kind,
            "number"
                | "string"
                | "true"
                | "false"
                | "null"
                | "undefined"
                | "regex"
                | "template_string"
        )
    }

    fn safe_to_drop(node: &Node<'_, ()>) -> bool {
        if Self::literal_like_kind(node.kind()) {
            return true;
        }

        if node.kind() == "parenthesized_expression" {
            let parts = Self::expression_parts(node);
            return parts.len() == 1 && Self::safe_to_drop(&parts[0]);
        }

        if node.kind() == "sequence_expression" {
            let parts = Self::expression_parts(node);
            return !parts.is_empty() && parts.iter().all(Self::safe_to_drop);
        }

        false
    }

    fn reduce_sequence_text(node: &Node<'_, ()>) -> Option<String> {
        if node.kind() != "sequence_expression" {
            return None;
        }

        let parts = Self::expression_parts(node);
        if parts.len() < 2 {
            return None;
        }

        if !parts[..parts.len() - 1].iter().all(Self::safe_to_drop) {
            return None;
        }

        Some(parts.last()?.text().ok()?.trim().to_string())
    }
}

impl<'a> Rule<'a> for ReduceSequenceExpression {
    type Language = ();

    fn enter(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<bool> {
        match node.kind() {
            "program" => {
                self.source = node.text()?.to_string();
                self.last_index = 0;
            }
            "sequence_expression" => {
                if let Some(replacement) = Self::reduce_sequence_text(node) {
                    trace!("ReduceSequenceExpression: reducing comma sequence to last expression");
                    self.replace_node_with_text(node, &replacement);
                    return Ok(false);
                }
            }
            _ => {}
        }
        Ok(true)
    }

    fn leave(&mut self, _node: &Node<'a, Self::Language>) -> MinusOneResult<()> {
        Ok(())
    }
}
