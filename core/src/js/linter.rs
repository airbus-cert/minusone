use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::printer::{Printer, PrinterMode};
use crate::rule::Rule;
use crate::tree::{Node, Storage, Tree};

#[derive(Default)]
struct RemoveCode {
    source: String,
    pub output: String,
    last_index: usize,
}

impl RemoveCode {
    pub fn start_program<T>(&mut self, root: &Node<T>) -> MinusOneResult<()> {
        self.source = root.text()?.to_string();
        Ok(())
    }

    pub fn end_program(&mut self) -> MinusOneResult<()> {
        self.output += &self.source[self.last_index..];
        Ok(())
    }

    pub fn remove_node<T>(&mut self, node: &Node<T>) -> MinusOneResult<()> {
        while self.source.chars().nth(self.last_index) == Some('\n') {
            self.last_index += 1;
        }
        self.output += &self.source[self.last_index..node.start_abs()];
        self.last_index = node.end_abs();
        Ok(())
    }
}

/// Removes single-line (`//`) and multi-line (`/* */`) comments from JavaScript source.
#[derive(Default)]
pub struct RemoveComment {
    manager: RemoveCode,
}

impl RemoveComment {
    pub fn clear(self) -> MinusOneResult<String> {
        Ok(self.manager.output)
    }
}

impl<'a> Rule<'a> for RemoveComment {
    type Language = JavaScript;

    fn enter(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<bool> {
        match node.kind() {
            "program" => {
                self.manager.start_program(node)?;
            }
            "comment" => {
                self.manager.remove_node(node)?;
            }
            _ => (),
        }
        Ok(true)
    }

    fn leave(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<()> {
        if node.kind() == "program" {
            self.manager.end_program()?;
        }
        Ok(())
    }
}

/// Reconstructs the JavaScript source while preserving original whitespace
#[derive(Default)]
pub struct Linter {
    pub output: String,
    source: String,
    last_index: usize,
}

impl Linter {
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

    fn copy_until(&mut self, end: usize) {
        if end > self.last_index {
            self.output += &self.source[self.last_index..end];
        }
        self.last_index = end;
    }
    fn skip_until(&mut self, end: usize) {
        self.last_index = end;
    }
}

impl<'a> Rule<'a> for Linter {
    type Language = JavaScript;

    fn enter(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<bool> {
        match node.kind() {
            "program" => {
                self.source = node.text()?.to_string();
                self.last_index = 0;
                return Ok(true);
            }
            "comment" => {
                self.copy_until(node.start_abs());
                self.skip_until(node.end_abs());
                return Ok(false);
            }
            _ => (),
        }

        if let Some(data) = node.data() {
            // fn nodes may cache their original source, but we still want to emit transformed children from the latest AST
            if Self::is_function_like_kind(node.kind()) {
                return Ok(true);
            }

            // keep identifiers stable when they refer to fn values
            if node.kind() == "identifier" && matches!(data, JavaScript::Function { .. }) {
                return Ok(true);
            }

            self.copy_until(node.start_abs());
            // Preserve parentheses for conditions in control-flow statements to keep the output as valid JavaScript
            if node.kind() == "parenthesized_expression" {
                if let Some(parent) = node.parent() {
                    match parent.kind() {
                        "if_statement" | "while_statement" | "do_statement" | "for_statement"
                        | "switch_statement" => {
                            self.output += &format!("({})", data);
                            self.skip_until(node.end_abs());
                            return Ok(false);
                        }
                        _ => {}
                    }
                }
            }
            self.output += &data.to_string();
            self.skip_until(node.end_abs());
            return Ok(false);
        }

        Ok(true)
    }

    fn leave(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<()> {
        if node.kind() == "program" {
            let len = self.source.len();
            self.copy_until(len);
        }
        Ok(())
    }
}

impl Printer for Linter {
    type Language = JavaScript;

    fn print<S>(&mut self, tree: &Tree<'_, S>, _mode: PrinterMode) -> MinusOneResult<String>
    where
        S: Storage<Component = Self::Language> + Default,
    {
        self.output.clear();
        self.source.clear();
        self.last_index = 0;
        tree.apply(self)?;
        Ok(self.output.clone())
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
    use crate::js::var::Var;

    #[test]
    fn test_linter_emits_simplified_function_expression_body() {
        let mut tree = build_javascript_tree("let x = function () { return 1 + 2; };").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                SubAddInt::default(),
                ParseFunction::default(),
            ),
            JavaScriptStrategy::default(),
        )
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();

        assert_eq!(linter.output, "let x = function () { return 3; };");
    }

    #[test]
    fn test_linter_keeps_identifier_on_function_object_assignment() {
        let mut tree = build_javascript_tree(
            "let a = {}; let x = function (n) { return n + 1; }; a.t = x; console.log(a.t(1));",
        )
        .unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                SubAddInt::default(),
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

        assert!(linter.output.contains("a.t = x;"));
    }
}
