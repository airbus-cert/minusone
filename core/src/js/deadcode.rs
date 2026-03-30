use crate::error::MinusOneResult;
use crate::rule::Rule;
use crate::tree::Node;
use log::trace;
use std::collections::HashMap;

/// Tracks which JavaScript identifiers are actually read (used) vs only written to (declared/assigned).
#[derive(Default, Clone)]
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

/// Removes variable declarations, assignments, and function declarations where the declared name is never read.
///
/// # Example
/// ```
/// use minusone::js::deadcode::{UnusedVar, RemoveUnusedVar};
/// use minusone::js::build_javascript_tree_for_storage;
/// use minusone::tree::EmptyStorage;
///
/// let source = "var a = 'hello'; console.log('world');";
/// let tree = build_javascript_tree_for_storage::<EmptyStorage>(source).unwrap();
///
/// let mut unused = UnusedVar::default();
/// tree.apply(&mut unused).unwrap();
///
/// let mut remover = RemoveUnusedVar::new(unused);
/// tree.apply(&mut remover).unwrap();
///
/// assert_eq!(remover.clear().unwrap(), "console.log('world');");
/// ```
#[derive(Clone)]
pub struct RemoveUnusedVar {
    rule: UnusedVar,
    source: String,
    output: String,
    last_index: usize,
}

impl RemoveUnusedVar {
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
                if let Some(name_node) = child.named_child("name") {
                    if name_node.kind() == "identifier" {
                        name = Some(name_node.text().ok()?.to_string());
                    }
                }
            }
        }
        name
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
}

impl<'a> Rule<'a> for RemoveUnusedVar {
    type Language = ();

    fn enter(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<bool> {
        match node.kind() {
            "program" => {
                self.source = node.text()?.to_string();
                self.last_index = 0;
            }
            // var x = ...; / let x = ...; / const x = ...;
            "variable_declaration" | "lexical_declaration" => {
                if let Some(var_name) = Self::single_declarator_name(node) {
                    if self.rule.is_unused(&var_name) {
                        trace!("RemoveUnusedVar: removing declaration of '{}'", var_name);
                        self.remove_node(node)?;
                        return Ok(false);
                    }
                }
            }
            // x = ...;  (expression_statement wrapping an assignment_expression)
            // also removes bare literal expression statements (e.g. 1;, 'hello';, true;)
            "expression_statement" => {
                if let Some(child) = node.child(0) {
                    match child.kind() {
                        "assignment_expression" => {
                            if let Some(left) = child.child(0) {
                                if left.kind() == "identifier" {
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
                if let Some(name_node) = node.named_child("name") {
                    if name_node.kind() == "identifier" {
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
            }
            // if statements with known boolean conditions
            "if_statement" => {
                if let Some(condition) = node.named_child("condition") {
                    if Self::is_literal_bool(&condition, false) {
                        if Self::has_else_clause(node) {
                            // if (false) { ... } else { BODY } -> keep BODY
                            for child in node.iter() {
                                if child.kind() == "else_clause" {
                                    for else_child in child.iter() {
                                        if else_child.kind() == "statement_block" {
                                            if let Some(inner) = Self::block_inner_text(&else_child)
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
                            }
                        } else {
                            // if (false) { ... } with no else -> remove entirely
                            trace!("RemoveUnusedVar: removing dead if (false) statement");
                            self.remove_node(node)?;
                            return Ok(false);
                        }
                    } else if Self::is_literal_bool(&condition, true) {
                        // if (true) { BODY } ... -> keep BODY, discard else
                        if let Some(consequence) = node.named_child("consequence") {
                            if let Some(inner) = Self::block_inner_text(&consequence) {
                                trace!("RemoveUnusedVar: replacing if (true) with if body");
                                self.replace_node_with_text(node, &inner)?;
                                return Ok(false);
                            }
                        }
                    }
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

#[cfg(test)]
mod test_js_deadcode {
    use super::*;
    use crate::js::build_javascript_tree_for_storage;
    use crate::tree::EmptyStorage;

    fn clean(input: &str) -> String {
        let tree = build_javascript_tree_for_storage::<EmptyStorage>(input).unwrap();

        let mut unused = UnusedVar::default();
        tree.apply(&mut unused).unwrap();

        let mut remover = RemoveUnusedVar::new(unused);
        tree.apply(&mut remover).unwrap();
        remover.clear().unwrap()
    }

    #[test]
    fn test_remove_unused_var() {
        assert_eq!(
            clean("var a = 'hello'; console.log('world');"),
            "console.log('world');"
        );
    }

    #[test]
    fn test_keep_used_var() {
        assert_eq!(
            clean("var a = 'hello'; console.log(a);"),
            "var a = 'hello'; console.log(a);"
        );
    }

    #[test]
    fn test_remove_unused_assignment() {
        assert_eq!(
            clean("var a = 1; a = 2; console.log('ok');"),
            "console.log('ok');"
        );
    }

    #[test]
    fn test_remove_unused_function() {
        assert_eq!(
            clean("function unused() { return 1; } console.log('hello');"),
            "console.log('hello');"
        );
    }

    #[test]
    fn test_keep_used_function() {
        assert_eq!(
            clean("function test() { return 1; } test();"),
            "function test() { return 1; } test();"
        );
    }

    #[test]
    fn test_remove_multiple_unused() {
        assert_eq!(
            clean("var a = 1; var b = 2; console.log('ok');"),
            "console.log('ok');"
        );
    }

    #[test]
    fn test_keep_mixed() {
        assert_eq!(
            clean("var a = 1; var b = 2; console.log(a);"),
            "var a = 1; console.log(a);"
        );
    }

    #[test]
    fn test_remove_unused_let_const() {
        assert_eq!(
            clean("let a = 1; const b = 2; console.log('ok');"),
            "console.log('ok');"
        );
    }

    #[test]
    fn test_full_pipeline_dead_code() {
        assert_eq!(
            clean("function test() { return 'hello'; } console.log('hello');"),
            "console.log('hello');"
        );
    }

    #[test]
    fn test_remove_bare_number() {
        assert_eq!(clean("1; console.log('ok');"), "console.log('ok');");
    }

    #[test]
    fn test_remove_bare_string() {
        assert_eq!(clean("'hello'; console.log('ok');"), "console.log('ok');");
    }

    #[test]
    fn test_remove_bare_bool() {
        assert_eq!(
            clean("true; false; console.log('ok');"),
            "console.log('ok');"
        );
    }

    #[test]
    fn test_remove_bare_literal_after_fncall_inlining() {
        assert_eq!(clean("1; console.log('world');"), "console.log('world');");
    }

    #[test]
    fn test_remove_if_false() {
        assert_eq!(
            clean("if (false) { console.log('no'); } console.log('yes');"),
            "console.log('yes');"
        );
    }

    #[test]
    fn test_if_false_with_else_keeps_else_body() {
        assert_eq!(
            clean("if (false) { console.log('no'); } else { console.log('yes'); }"),
            "console.log('yes');"
        );
    }

    #[test]
    fn test_if_true_keeps_if_body() {
        assert_eq!(
            clean("if (true) { console.log('yes'); }"),
            "console.log('yes');"
        );
    }

    #[test]
    fn test_if_true_with_else_keeps_if_body() {
        assert_eq!(
            clean("if (true) { console.log('yes'); } else { console.log('no'); }"),
            "console.log('yes');"
        );
    }

    #[test]
    fn test_keep_if_variable() {
        assert_eq!(
            clean("if (x) { console.log('maybe'); }"),
            "if (x) { console.log('maybe'); }"
        );
    }

    #[test]
    fn test_no_panic_when_parent_removed_before_children() {
        assert_eq!(
            clean("function drop() { var a = 1; a = 2; 1; } console.log('ok');"),
            "console.log('ok');"
        );
    }

    #[test]
    fn test_stress_many_removals_no_slice_panic() {
        let mut input = String::new();
        for i in 0..200 {
            input += &format!("function f{}() {{ var a = 1; a = 2; 1; }} ", i);
        }
        input += "console.log('ok');";

        assert_eq!(clean(&input), "console.log('ok');");
    }
}
