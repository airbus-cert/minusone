use ps::Powershell;
use tree::Node;
use error::{MinusOneResult, Error};
use ps::Powershell::Raw;
use ps::Value::{Str, Num, Bool};

pub struct Linter {
    pub output: String,
    tab: String,
    is_inline_statement: bool
}

impl Linter {
    pub fn new() -> Self {
        Linter {
            output: String::new(),
            tab: String::new(),
            is_inline_statement: false
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
                },
                Raw(Bool(true)) => {
                    self.output.push_str("$true".to_string().as_str());
                    return Ok(())
                },
                Raw(Bool(false)) => {
                    self.output.push_str("$false".to_string().as_str());
                    return Ok(())
                },
                _ => () // We will only manage raw value
            }
        }

        match node.kind() {
            // Statement separated rule
            "program" => self.transparent(node)?,

            // Space separated token
            "pipeline" | "command" |
            "assignment_expression" | "left_assignment_expression" |
            "command_elements" | "foreach_statement" |
            "while_condition" |
            "trap_statement" | "data_statement" |
            "try_statement" | "catch_clauses" | "catch_clause" |
            "finally_clause" | "catch_type_list" |
            "if_statement" | "else_clause" | "elseif_clause" |
            "named_block" | "logical_expression" | "bitwise_expression" |
            "comparison_expression" | "additive_expression" |
            "multiplicative_expression" | "format_expression" |
            "unary_expression" | "argument_expression" |
            "range_argument_expression" | "format_argument_expression" |
            "multiplicative_argument_expression" | "additive_argument_expression" |
            "comparison_argument_expression" | "bitwise_argument_expression" |
            "logical_argument_expression" | "hash_literal_expression" |
            "do_statement" | "elseif_clauses" => self.space_sep(node, None)?,

            "array_literal_expression" | "argument_expression_list" => self.list_sep(node)?,

            "statement_block" => self.statement_block(node)?,

            "script_block_expression" => {
                let staked_value = self.is_inline_statement;
                self.is_inline_statement = true;
                self.space_sep(node, None)?;
                self.is_inline_statement = staked_value;
            },

            "sub_expression" => {
                let staked_value = self.is_inline_statement;
                self.is_inline_statement = true;
                self.transparent(node)?;
                self.is_inline_statement = staked_value;
            },

            "parenthesized_expression" => self.parenthesized_expression(node)?,

            "command_name_expr" | "element_access" |
            "invokation_expression" | "argument_list" |
            "range_expression" | "member_access" |
            "post_increment_expression" | "post_decrement_expression" |
            "type_literal" | "cast_expression" |
            "member_name" | "expression_with_unary_operator" => self.transparent(node)?,

            "empty_statement" => {}, // Do nothing

            "named_block_list" => self.newline_sep(node)?,

            "script_block" => {
                if self.is_inline_statement {
                    self.transparent(node)?;
                }
                else {
                    self.indent(node)?;
                }
            },

            "script_block_body" => self.transparent(node)?,

            "for_statement" => self.for_statement(node)?,

            "function_statement" | "function_parameter_declaration" |
            "param_block" => self.space_sep(node, Some(1))?,

            "statement_list" => {
                if self.is_inline_statement {
                    self.inline_statement_list(node)?;
                }
                else {
                    self.statement_list(node)?;
                }
            },

            "while_statement" => self.conditional_statement(node)?,

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

    fn space_sep(&mut self, node: &Node<Powershell>, end: Option<usize>) -> MinusOneResult<()>{
        let mut nb_childs = node.child_count() - end.unwrap_or(0);
        for child in node.iter() {
            self.print(&child)?;
            if nb_childs > 0 {
                nb_childs -= 1;
                if nb_childs > 0 {
                    self.output += " ";
                }
            }
        }
        Ok(())
    }

    fn list_sep(&mut self, node: &Node<Powershell>) -> MinusOneResult<()>{
        let mut nb_childs = node.child_count();
        for child in node.range(None, None, Some(2)) {
            self.print(&child)?;
            if nb_childs > 1 {
                nb_childs -= 2;
                if nb_childs > 0 {
                    self.output += ", ";
                }
            }
        }
        Ok(())
    }

    fn statement_list(&mut self, node: &Node<Powershell>) -> MinusOneResult<()> {
        let mut is_first = true;
        for child in node.iter() {

            if child.kind() == "empty_statement" {
                continue;
            }

            if !is_first {
                self.output += "\n";
                self.output += &self.tab;
            }
            else {
                is_first = false;
            }

            //self.output += &self.tab;
            self.print(&child)?;
        }

        Ok(())
    }

    fn inline_statement_list(&mut self, node: &Node<Powershell>) -> MinusOneResult<()> {
        let mut is_first = true;
        for child in node.iter() {
            if child.kind() == "empty_statement" {
                continue;
            }

            if !is_first {
                self.output += "; ";
            }
            else {
                is_first = false;
            }

            self.print(&child)?;
        }

        Ok(())
    }

    fn newline_sep(&mut self, node: &Node<Powershell>) -> MinusOneResult<()> {
        let mut child_count = node.child_count();
        self.output += "\n";
        for child in node.iter() {
            self.output += &self.tab;
            self.print(&child)?;
            child_count -= 1;
            if child_count > 0 {
                self.output += "\n";
            }
        }
        Ok(())
    }

    fn statement_block(&mut self, node: &Node<Powershell>) -> MinusOneResult<()> {
        let old_tab = self.tab.clone();
        self.tab.push_str(" ");

        self.output += "{\n";
        self.output += &self.tab;

        if let Some(statement_list) = node.named_child("statement_list") {
            self.statement_list(&statement_list)?;
        }

        self.output += "\n";
        self.output += &old_tab;
        self.output += "}";

        self.tab = old_tab;
        Ok(())
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
            self.output += "(";
            self.print(&node.child(1).ok_or(Error::invalid_child())?)?;
            self.output += ")";
        }
        else {
            self.print(&node.child(1).ok_or(Error::invalid_child())?)?;
        }

        Ok(())
    }

    fn for_statement(&mut self, node: &Node<Powershell>) -> MinusOneResult<()> {

        let for_initializer = node.named_child("for_initializer");
        let for_condition = node.named_child("for_condition");
        let for_iterator = node.named_child("for_iterator");

        self.output += "for ( ";
        if let Some(n) = for_initializer {
            self.space_sep(&n, None)?;
        }
        self.output += " ; ";
        if let Some(n) = for_condition {
            self.space_sep(&n, None)?;
        }
        self.output += " ; ";
        if let Some(n) = for_iterator {
            self.space_sep(&n, None)?;
        }
        self.output += " ) ";

        self.statement_block(&node.child(node.child_count() - 1).unwrap())?;
        Ok(())
    }

    fn indent(&mut self, node: &Node<Powershell>) -> MinusOneResult<()> {
        self.output += "\n";
        let old_tab = self.tab.clone();
        self.tab += " ";
        for child in node.iter() {
            self.output += &self.tab;
            self.print(&child)?;
            self.output += "\n";
        }
        self.tab = old_tab;
        Ok(())
    }

    fn conditional_statement(&mut self, node: &Node<Powershell>) -> MinusOneResult<()> {
        if let Some(condition) = node.named_child("condition") {
            // dead code elysium
            if condition.data() != Some(&Raw(Bool(false))) {
                self.space_sep(node, None)?
            }
        }
        Ok(())
    }
}