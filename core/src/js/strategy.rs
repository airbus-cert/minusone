use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::Raw;
use crate::js::Value::Bool;
use crate::tree::BranchFlow::{Predictable, Unpredictable};
use crate::tree::ControlFlow::{Break, Continue};
use crate::tree::{ControlFlow, Node, Strategy};

#[derive(Default)]
pub struct JavaScriptStrategy;

impl Strategy<JavaScript> for JavaScriptStrategy {
    fn control(&self, node: Node<JavaScript>) -> MinusOneResult<ControlFlow> {
        match node.kind() {
            "statement_block" => {
                if let Some(parent) = node.parent() {
                    match parent.kind() {
                        "if_statement" => {
                            if let Some(condition) = parent.named_child("condition") {
                                return match condition.data() {
                                    Some(Raw(Bool(true))) => Ok(Continue(Predictable)),
                                    Some(Raw(Bool(false))) => Ok(Break),
                                    _ => Ok(Continue(Unpredictable)),
                                };
                            }
                            Ok(Continue(Unpredictable))
                        }
                        "else_clause" => {
                            // predictable if the if condition is known
                            if let Some(if_statement) = parent.parent()
                                && let Some(condition) = if_statement.named_child("condition") {
                                    return match condition.data() {
                                        Some(Raw(Bool(false))) => Ok(Continue(Predictable)),
                                        Some(Raw(Bool(true))) => Ok(Break),
                                        _ => Ok(Continue(Unpredictable)),
                                    };
                                }
                            Ok(Continue(Unpredictable))
                        }
                        "while_statement" | "do_statement" | "for_statement"
                        | "for_in_statement" => Ok(Continue(Unpredictable)),
                        "try_statement" | "catch_clause" | "finally_clause" => {
                            Ok(Continue(Unpredictable))
                        }
                        "switch_case" | "switch_default" => Ok(Continue(Unpredictable)),
                        // function bodies are predictable
                        "function_declaration"
                        | "function"
                        | "arrow_function"
                        | "method_definition"
                        | "generator_function_declaration"
                        | "generator_function" => Ok(Continue(Predictable)),
                        _ => Ok(Continue(Predictable)),
                    }
                } else {
                    Ok(Continue(Predictable))
                }
            }
            // else_clause itself: decide whether to visit based on if condition
            "else_clause" => {
                if let Some(if_statement) = node.parent()
                    && let Some(condition) = if_statement.named_child("condition") {
                        return match condition.data() {
                            Some(Raw(Bool(true))) => Ok(Break),
                            Some(Raw(Bool(false))) => Ok(Continue(Predictable)),
                            _ => Ok(Continue(Unpredictable)),
                        };
                    }
                Ok(Continue(Unpredictable))
            }
            _ => Ok(Continue(Predictable)),
        }
    }
}
