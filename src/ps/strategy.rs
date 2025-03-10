use error::MinusOneResult;
use ps::Powershell;
use ps::Powershell::Raw;
use ps::Value::Bool;
use tree::BranchFlow::{Predictable, Unpredictable};
use tree::ControlFlow::{Break, Continue};
use tree::{ControlFlow, Node, Strategy};

#[derive(Default)]
pub struct PowershellStrategy;

impl Strategy<Powershell> for PowershellStrategy {
    fn control(&self, node: Node<Powershell>) -> MinusOneResult<ControlFlow> {
        match node.kind() {
            "statement_block" => {
                let parent = node.parent().unwrap();
                match parent.kind() {
                    "while_statement" => {
                        // Don't visit node inferred node with a branch set to false
                        if let Some(condition) = parent.named_child("condition") {
                            if condition.data() == Some(&Raw(Bool(false))) {
                                return Ok(Break);
                            }
                        }
                        // Any other inferred type led to unpredictable branch visit
                        Ok(Continue(Unpredictable))
                    }
                    "if_statement" => {
                        // if ($true) control flow
                        if let Some(condition) = parent.named_child("condition") {
                            return match condition.data() {
                                Some(Raw(Bool(true))) => Ok(Continue(Predictable)),
                                Some(Raw(Bool(false))) => Ok(Break),
                                _ => Ok(Continue(Unpredictable)),
                            };
                        }
                        Ok(Continue(Unpredictable))
                    }
                    "elseif_clause" => {
                        // elseif_clause is evaluated only if the if statement if false or unpredictable
                        // We have to check if previous clause are predictable or not
                        let elseif_clauses = parent.parent().unwrap();

                        // loop over all elseif clauses
                        for elseif_clause in elseif_clauses.iter() {
                            if elseif_clause == parent {
                                break;
                            }

                            if let Some(condition) = elseif_clause.named_child("condition") {
                                return match condition.data() {
                                    Some(Raw(Bool(false))) => continue,
                                    Some(Raw(Bool(true))) => return Ok(Break),
                                    _ => Ok(Continue(Unpredictable)),
                                };
                            }

                            return Ok(Continue(Unpredictable));
                        }

                        // elseif ($true) control flow
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
                        // else clause is visited only if the main if is false
                        // but we have to check elseif_clause
                        if let Some(elseif_clauses) =
                            parent.parent().unwrap().named_child("elseif_clauses")
                        {
                            for elseif_clause in elseif_clauses.iter() {
                                if let Some(condition) = elseif_clause.named_child("condition") {
                                    match condition.data() {
                                        Some(Raw(Bool(false))) => continue,
                                        Some(Raw(Bool(true))) => return Ok(Break), // no need to evaluate the else branch
                                        _ => return Ok(Continue(Unpredictable)),
                                    }
                                } else {
                                    return Ok(Continue(Unpredictable));
                                }
                            }
                        }
                        Ok(Continue(Predictable))
                    }
                    // function are predictable
                    "function_statement" => Ok(Continue(Predictable)),
                    _ => Ok(Continue(Predictable)),
                }
            }
            // We can add condition to visit these node depending on the main if condition
            "elseif_clauses" | "else_clause" => {
                let if_statement = node.parent().unwrap();
                if let Some(condition) = if_statement.named_child("condition") {
                    return match condition.data() {
                        // don't visit elseif_clauses if the main if is true
                        Some(Raw(Bool(true))) => Ok(Break),
                        // We have to evaluate all elseif_clause
                        Some(Raw(Bool(false))) => Ok(Continue(Predictable)),
                        // The inferred type is not boolean
                        // We don't known which clauses will be used
                        _ => Ok(Continue(Unpredictable)),
                    };
                }

                // We were not able to infer type so we are in an unpredictable case
                Ok(Continue(Unpredictable))
            }
            // All this statement are labeled and not inferred at this moment
            "trap_statement" | "try_statement" | "catch_clause" | "finally_clause"
            | "data_statement" | "parallel_statement" | "sequence_statement"
            | "switch_statement" | "foreach_statement" | "for_statement" | "while_statement" => {
                Ok(Continue(Unpredictable))
            }
            // Any other node than statement block become unpredictable
            _ => Ok(Continue(Predictable)),
        }
    }
}
