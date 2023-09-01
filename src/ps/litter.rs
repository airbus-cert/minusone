use rule::Rule;
use ps::InferredValue;
use tree::Node;
use error::{MinusOneResult, Error};
use std::ops::Add;

pub struct PowershellLitter {
    pub output: String,
    tab: String,
}

impl PowershellLitter {
    pub fn new() -> Self {
        PowershellLitter {
            output: String::new(),
            tab: String::new(),
        }
    }

    pub fn print(&mut self, node: &Node<InferredValue>) -> MinusOneResult<()> {
        // handle inferred type first
        if let Some(inferred_type) = node.data() {
            match inferred_type {
                InferredValue::Str(str) => {
                    self.output.push_str(str);
                    return Ok(());
                }
                InferredValue::Number(number) => {
                    self.output.push_str(number.to_string().as_str());
                    return Ok(());
                }
                _ => ()
            }
        }

        match node.kind() {
            // Statement separated rule
            "program" => self.statement_sep(node)?,

            // Space separated token
            "pipeline" | "command" |
            "assignment_expression" | "left_assignment_expression" |
            "logical_expression" | "bitwise_expression" |
            "comparison_expression" | "additive_expression" |
            "multiplicative_expression" | "format_expression" |
            "range_expression" | "array_literal_expression" |
            "unary_expression"
            => self.space_sep(node)?,

            "statement_block" => self.statement_block(node)?,
            "if_statement" => self.if_statement(node)?,

            // Un modified tokens
            _ => {
                self.output += node.text()?
            }
        }

        Ok(())
    }


    fn space_sep(&mut self, node: &Node<InferredValue>) -> MinusOneResult<()>{
        let mut nb_childs = node.child_count();
        for child in node.iter() {
            self.print(&child);
            nb_childs -= 1;
            if nb_childs > 0 {
                self.output += " ";
            }
        }

        Ok(())
    }

    fn statement_sep(&mut self, node: &Node<InferredValue>) -> MinusOneResult<()> {
        for child in node.iter() {
            self.output += &self.tab;
            self.print(&child)?;
            self.output += "\n";
        }

        Ok(())
    }

    fn explore(&mut self, node: &Node<InferredValue>) -> MinusOneResult<()> {
        for child in node.iter() {
            self.print(&child);
        }

        Ok(())
    }

    fn statement_block(&mut self, node: &Node<InferredValue>) -> MinusOneResult<()> {
        let old_tab = self.tab.clone();
        self.tab.push_str(" ");

        self.output += &old_tab;
        self.output += "{\n";

        // all statement seperated by a line
        for child in node.range(Some(1), Some(node.child_count() - 1), None) {
            self.output += &self.tab;
            self.print(&child);
            self.output += "\n";
        }

        self.output += &old_tab;
        self.output += "}\n";

        self.tab = old_tab;
        Ok(())
    }

    fn if_statement(&mut self, node: &Node<InferredValue>) -> MinusOneResult<()> {
        self.output += &self.tab;
        self.output += "if (";
        self.print(&node.child(2).ok_or(Error::invalid_child())?)?;
        self.output += ")\n";
        self.print(&node.child(4).ok_or(Error::invalid_child())?)?;

        if let Some(elseif_clauses) = node.named_child("elseif_clauses") {
            for elseif_clause in elseif_clauses.iter() {
                self.output += &self.tab;
                self.output += "elseif (";
                self.print(&elseif_clause.child(2).ok_or(Error::invalid_child())?)?;
                self.output += ")\n";
                self.print(&elseif_clause.child(4).ok_or(Error::invalid_child())?)?;
            }
        }

        if let Some(else_clause) = node.named_child("else_clause") {
            self.output += &self.tab;
            self.output += "else\n";
            self.print(&else_clause.child(1).ok_or(Error::invalid_child())?)?;
        }

        Ok(())
    }
}