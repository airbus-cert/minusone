use rule::Rule;
use ps::InferredValue;
use tree::Node;
use error::MinusOneResult;
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
            "program" => self.statement_sep(node),

            // Space separated token
            "pipeline" | "command" |
            "assignment_expression" | "left_assignment_expression" |
            "logical_expression" | "bitwise_expression" |
            "comparison_expression" | "additive_expression" |
            "multiplicative_expression" | "format_expression" |
            "range_expression" | "array_literal_expression" |
            "unary_expression"
            => self.space_sep(node),

            // Tokens
            _ => {
                self.output += node.text()?
            }
        }

        Ok(())
    }


    fn space_sep(&mut self, node: &Node<InferredValue>) {
        let mut nb_childs = node.child_count();
        for child in node.iter() {
            self.print(&child);
            nb_childs -= 1;
            if nb_childs > 0 {
                self.output += " ";
            }
        }
    }

    fn statement_sep(&mut self, node: &Node<InferredValue>) {
        for child in node.iter() {
            self.print(&child);
            self.output += "\n";
        }
    }

    fn explore(&mut self, node: &Node<InferredValue>) {
        for child in node.iter() {
            self.print(&child);
        }
    }

    fn statement_block(&mut self, node: &Node<InferredValue>) {
        self.output += "{";
        let old_tab = self.tab.clone();
        self.tab += " ";
        self.tab = old_tab;
        self.output += self.tab.as_str();
        self.output += "}";
    }
}