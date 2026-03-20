use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, Node, NodeMut};
use log::trace;

/// Parses function expressions into `Function(_)`.
///
/// This keeps function-valued object fields lossless (source-preserving)
/// while still allowing object propagation to carry them around.
#[derive(Default)]
pub struct ParseFunction;

fn is_function_kind(kind: &str) -> bool {
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

fn is_function_expression_kind(kind: &str) -> bool {
    matches!(kind, "function" | "function_expression" | "arrow_function" | "generator_function")
}

fn looks_like_function_source(source: &str) -> bool {
    let trimmed = source.trim();
    trimmed.starts_with("function")
        || trimmed.starts_with("async function")
        || trimmed.contains("=>")
}

fn walk_for_returns(
    node: &Node<JavaScript>,
    return_value: &mut Option<JavaScript>,
    found_count: &mut usize,
) {
    for child in node.iter() {
        match child.kind() {
            "return_statement" => {
                *found_count += 1;
                if *found_count == 1 {
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
            | "generator_function" => {}
            "if_statement" | "while_statement" | "do_statement" | "for_statement"
            | "for_in_statement" | "switch_statement" | "try_statement" => {
                let mut inner_count = 0;
                count_returns_in_subtree(&child, &mut inner_count);
                if inner_count > 0 {
                    *found_count += inner_count;
                }
            }
            _ => walk_for_returns(&child, return_value, found_count),
        }
    }
}

fn count_returns_in_subtree(node: &Node<JavaScript>, count: &mut usize) {
    for child in node.iter() {
        match child.kind() {
            "return_statement" => *count += 1,
            "function_declaration"
            | "function"
            | "arrow_function"
            | "generator_function_declaration"
            | "generator_function" => {}
            _ => count_returns_in_subtree(&child, count),
        }
    }
}

fn find_single_return_value(body: &Node<JavaScript>) -> Option<JavaScript> {
    let mut return_value: Option<JavaScript> = None;
    let mut found_count = 0;
    walk_for_returns(body, &mut return_value, &mut found_count);
    if found_count == 1 { return_value } else { None }
}

pub fn function_value_from_node(node: &Node<JavaScript>) -> Option<JavaScript> {
    let source = node.text().ok()?.to_string();
    if !is_function_kind(node.kind()) && !looks_like_function_source(&source) {
        return None;
    }

    let return_value = node.named_child("body").and_then(|body| {
        if body.kind() == "statement_block" {
            find_single_return_value(&body)
        } else {
            body.data().cloned()
        }
    });

    Some(JavaScript::Function {
        source,
        return_value: return_value.map(Box::new),
    })
}

impl<'a> RuleMut<'a> for ParseFunction {
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
        if !is_function_expression_kind(view.kind()) {
            return Ok(());
        }

        if let Some(function_value) = function_value_from_node(&view) {
            trace!("ParseFunction (L): function expression => {:?}", function_value);
            node.reduce(function_value);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::js::build_javascript_tree;
    use crate::js::function::ParseFunction;
    use crate::js::linter::Linter;

    #[test]
    fn test_parse_arrow_function_literal() {
        let mut tree = build_javascript_tree("const f = () => 1;").unwrap();
        tree.apply_mut(&mut ParseFunction::default()).unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();

        assert_eq!(linter.output, "const f = () => 1;");
    }
}


