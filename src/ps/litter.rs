use ps::Powershell;
use tree::Node;
use error::{MinusOneResult, Error};
use ps::Powershell::Raw;
use ps::Value::{Str, Num};

pub struct Litter {
    pub output: String,
    tab: String,
}

impl Litter {
    pub fn new() -> Self {
        Litter {
            output: String::new(),
            tab: String::new(),
        }
    }

    pub fn print(&mut self, node: &Node<Powershell>) -> MinusOneResult<()> {
        // handle inferred type first
        if let Some(inferred_type) = node.data() {
            match inferred_type {
                Raw(Str(str)) => {
                    self.output += "\"";
                    self.output += str;
                    self.output += "\"";
                    return Ok(());
                }
                Raw(Num(number)) => {
                    self.output.push_str(number.to_string().as_str());
                    return Ok(());
                }
                _ => () // We will only manage raw value
            }
        }

        match node.kind() {
            // Statement separated rule
            "program" => self.statements(node)?,

            // Space separated token
            "pipeline" | "command" |
            "assignment_expression" | "left_assignment_expression" |
            "command_elements" => self.space_sep(node)?,

            "logical_expression" | "bitwise_expression" |
            "comparison_expression" | "additive_expression" |
            "multiplicative_expression" | "format_expression" |
            "range_expression" | "array_literal_expression" |
            "unary_expression" => self.expression(node)?,

            "statement_block" => self.statement_block(node)?,
            "if_statement" => self.if_statement(node)?,

            "sub_expression" => self.sub_expression(node)?,

            "script_block_expression" => self.script_block_expression(node)?,

            "script_block" => self.script_block(node, false)?,

            "parenthesized_expression" => self.parenthesized_expression(node)?,

            "command_name_expr" => self.transparent(node)?,

            "empty_statement" => {}, // Do nothing

            "while_statement" => self.while_statement(node)?,

            // Unmodified tokens
            _ => {
                self.output += &node.text()?.to_lowercase()
            }
        }

        Ok(())
    }

    fn transparent(&mut self, node: &Node<Powershell>) -> MinusOneResult<()> {
        for child in node.iter() {
            self.print(&child)?;
        }
        Ok(())
    }

    fn expression(&mut self, node: &Node<Powershell>) -> MinusOneResult<()> {
        self.print(&node.child(0).ok_or(Error::invalid_child())?)?;
        if let (Some(operator), Some(right_node)) = (node.child(1), node.child(2)) {
            self.output += " ";
            self.output += operator.text()?;
            self.output += " ";
            self.print(&right_node)?;
        }
        Ok(())
    }

    fn sub_expression(&mut self, node: &Node<Powershell>) -> MinusOneResult<()> {
        self.output += "$(";
        if let Some(statements) = node.named_child("statements") {
            for statement in statements.iter() {
                self.print(&statement)?;
            }
        }
        self.output += ")";
        Ok(())
    }

    fn space_sep(&mut self, node: &Node<Powershell>) -> MinusOneResult<()>{
        let mut nb_childs = node.child_count();
        for child in node.iter() {
            self.print(&child)?;
            nb_childs -= 1;
            if nb_childs > 0 {
                self.output += " ";
            }
        }

        Ok(())
    }

    fn statements(&mut self, node: &Node<Powershell>) -> MinusOneResult<()> {
        let mut is_first = true;
        for child in node.iter() {

            if child.kind() == "empty_statement" {
                continue;
            }

            if !is_first {
                self.output += "\n";
            }
            else {
                is_first = false;
            }

            self.output += &self.tab;
            self.print(&child)?;
        }

        Ok(())
    }

    fn inline_statements(&mut self, node: &Node<Powershell>) -> MinusOneResult<()> {
        let mut is_first = true;
        for child in node.iter() {
            if child.kind() == "empty_statement" {
                continue;
            }

            if !is_first {
                self.output += ";";
            }
            else {
                is_first = false;
            }

            self.output += &self.tab;
            self.print(&child)?;
        }

        Ok(())
    }

    fn statement_block(&mut self, node: &Node<Powershell>) -> MinusOneResult<()> {
        let old_tab = self.tab.clone();
        self.tab.push_str(" ");

        self.output += &old_tab;
        self.output += "{";

        // all statement seperated by a line
        for child in node.range(Some(1  ), Some(node.child_count() - 1), None) {
            self.output += "\n";
            self.output += &self.tab;
            self.print(&child)?;
        }

        self.output += &old_tab;
        self.output += "}\n";

        self.tab = old_tab;
        Ok(())
    }

    fn if_statement(&mut self, node: &Node<Powershell>) -> MinusOneResult<()> {
        self.output += &self.tab;
        self.output += "if ( ";
        self.print(&node.child(2).ok_or(Error::invalid_child())?)?;
        self.output += " )\n";
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

    fn script_block_expression(&mut self, node: &Node<Powershell>) -> MinusOneResult<()> {
        self.output += "{ ";
        self.script_block(&node.child(1).ok_or(Error::invalid_child())?, true)?;
        self.output += " }";

        Ok(())
    }

    fn script_block(&mut self, node: &Node<Powershell>, inline: bool) -> MinusOneResult<()> {
        self.script_block_body(&node.child(0).ok_or(Error::invalid_child())?, inline)?;
        Ok(())
    }

    fn script_block_body(&mut self, node: &Node<Powershell>, inline: bool) -> MinusOneResult<()> {
        let child = node.child(0).ok_or(Error::invalid_child())?;

        if child.kind() == "named_block_list" {
            self.print(&child)
        }
        else {
            if inline {
                self.inline_statements(&node)
            }
            else {
                self.statements(&node)
            }
        }
    }

    fn parenthesized_expression(&mut self, node: &Node<Powershell>) -> MinusOneResult<()> {
        let mut is_priority = false;

        let mut parent = node.parent();
        loop {
            if let Some(parent_node) = &parent {
                // find a pipeline node => end exploring
                if parent_node.kind() == "pipeline" {
                    break;
                }
                // If my parent node have more than one child
                // is considering as complex
                if parent_node.child_count() != 1 {
                    is_priority = true;
                    break;
                }
                parent = parent_node.parent();
            }
            else {
                // end exploring
                break;
            }
        }

        // I know that i'm part of a complex expression
        // Check if i'm a complex expression too
        // If i'm here I don't have inferred type
        if is_priority {
            is_priority = false;

            let mut son = node.child(1);

            loop {
                if let Some(son_node) = &son {
                    if son_node.kind() == "unary_expression" {
                        break;
                    }
                    if son_node.child_count() > 1 {
                        is_priority = true;
                        break;
                    }
                    son = son_node.child(0);
                }
                else {
                    break;
                }
            }
        }

        // if priority is needed let's keep parenthesis
        if is_priority {
            self.output += "( ";
            self.print(&node.child(1).ok_or(Error::invalid_child())?)?;
            self.output += " )";
        }
        else {
            self.print(&node.child(1).ok_or(Error::invalid_child())?)?;
        }

        Ok(())
    }

    fn while_statement(&mut self, node: &Node<Powershell>) -> MinusOneResult<()> {
        let while_condition = node.child(2).ok_or(Error::invalid_child())?;
        let statement_block = node.child(4).ok_or(Error::invalid_child())?;

        self.output += "while ( ";
        self.print(&while_condition)?;
        self.output += " )\n";
        self.print(&statement_block)?;
        Ok(())
    }
}