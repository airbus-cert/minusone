use ps::Powershell;
use tree::Node;
use error::{MinusOneResult, Error};
use ps::Powershell::Raw;
use ps::Value::{Str, Num, Bool};

fn escape_string(src: &str) -> String {
    let mut result = String::new();
    let mut previous = None;
    for c in src.chars() {
        if c == '"' && previous != Some('`'){
            result.push('`');
        }
        result.push(c);
        previous = Some(c);
    }
    result
}


pub struct Linter {
    pub output: String,
    tab: String,
    tab_char: String,
    new_line_chr: String,
    is_inline_statement: bool
}

impl Linter {
    pub fn new() -> Self {
        Linter {
            output: String::new(),
            tab: String::new(),
            tab_char: " ".to_string(),
            new_line_chr: "\n".to_string(),
            is_inline_statement: false
        }
    }

    pub fn tab(mut self, tab_chr: &str) -> Self{
        self.tab_char = tab_chr.to_string();
        self
    }

    pub fn print(&mut self, node: &Node<Powershell>) -> MinusOneResult<()> {
        // handle inferred type first
        if let Some(inferred_type) = node.data() {
            match inferred_type {
                Raw(Str(str)) => {
                    self.output += "\"";
                    // normalisation of command
                    if node.kind() == "command_name_expr" {
                        self.output += &escape_string(&str.to_lowercase());
                    }
                    else {
                        self.output += &escape_string(str);
                    }
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
            "else_clause" | "elseif_clause" |
            "named_block" | "logical_expression" | "bitwise_expression" |
            "comparison_expression" | "additive_expression" |
            "multiplicative_expression" | "format_expression" |
            "unary_expression" | "argument_expression" |
            "range_argument_expression" | "format_argument_expression" |
            "multiplicative_argument_expression" | "additive_argument_expression" |
            "comparison_argument_expression" | "bitwise_argument_expression" |
            "logical_argument_expression" | "hash_literal_expression" |
            "do_statement" | "elseif_clauses" |
            "foreach_command" | "flow_control_statement" => self.space_sep(node, None)?,

            "array_literal_expression" | "argument_expression_list"  => self.list_sep(node)?,

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
            "member_name" | "script_parameter" |
            "string_literal" | "path_command_name" => self.transparent(node)?,

            "empty_statement" => {}, // Do nothing

            "named_block_list" => self.newline_sep(node, None)?,

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

            "function_statement" => {
                self.output += &self.new_line_chr;
                self.space_sep(node, Some(1))?
            },
            "function_parameter_declaration" => self.space_sep(node, None)?,

            "param_block" => {
                self.space_sep(node, Some(1))?;
            },

            "parameter_list" => {
                if node.parent().unwrap().kind() == "function_parameter_declaration" {
                    self.list_sep(node)?
                }
                else {
                    let old_ab = self.tab.clone();
                    self.tab += &self.tab_char;
                    self.output += "\n";
                    self.output += &self.tab;
                    self.list_sep_newline(node)?;
                    self.tab = old_ab;
                    self.output += "\n";
                    self.output += &self.tab;
                }
            },

            "attribute_list" => self.newline_sep(node, None)?,

            "statement_list" => {
                if self.is_inline_statement {
                    self.inline_statement_list(node)?;
                }
                else {
                    self.statement_list(node)?;
                }
            },

            "while_statement" => self.while_statement(node)?,
            "if_statement" => self.if_statement(node)?,
            "expandable_string_literal" | "expandable_here_string_literal" => self.expandable_string_literal(node)?,
            "expression_with_unary_operator" => {
                if let Some(operator) = node.child(0) {
                    match operator.text()? {
                        "-join" | "-not" | "-bnot" | "-split" => self.space_sep(node, None)?,
                        _ => self.transparent(node)?
                    }
                }
                else {
                    self.transparent(node)?
                }
            },
            "command_invokation_operator" => {
                self.output += "&"
            },
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

            nb_childs = nb_childs.saturating_sub(1);

            if child.kind() == "command_argument_sep" {
                continue;
            }

            self.print(&child)?;

            if nb_childs > 0 {
                self.output += " ";
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

    fn list_sep_newline(&mut self, node: &Node<Powershell>) -> MinusOneResult<()>{
        let mut nb_childs = node.child_count();
        for child in node.range(None, None, Some(2)) {
            self.print(&child)?;
            if nb_childs > 1 {
                nb_childs -= 2;
                if nb_childs > 0 {
                    self.output += ",\n";
                    self.output += &self.tab;
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
                match child.kind() {
                    "if_statement" | "try_statement" |
                    "while_statement" => self.output += "\n",
                    _ => ()
                }

                self.output += &self.new_line_chr;
                self.output += &self.tab;
            }
            else {
                is_first = false;
            }

            //self.output += &self.tab;
            self.print(&child)?;

            if !is_first {
                match child.kind() {
                    "if_statement" | "try_statement" |
                    "while_statement" => self.output += &self.new_line_chr,
                    _ => ()
                }
            }
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

    fn newline_sep(&mut self, node: &Node<Powershell>, end: Option<usize>) -> MinusOneResult<()> {
        let mut nb_childs = node.child_count() - end.unwrap_or(0);
        for child in node.iter() {
            self.print(&child)?;

            nb_childs = nb_childs.saturating_sub(1);
            if nb_childs > 0 {
                self.output += &self.new_line_chr;
                self.output += &self.tab;
            }
        }
        Ok(())
    }

    fn statement_block(&mut self, node: &Node<Powershell>) -> MinusOneResult<()> {
        let old_tab = self.tab.clone();
        self.tab.push_str(&self.tab_char);

        self.output += "{";
        self.output += &self.new_line_chr;
        self.output += &self.tab;

        if let Some(statement_list) = node.named_child("statement_list") {
            self.statement_list(&statement_list)?;
        }

        self.output += &self.new_line_chr;
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
        self.output += &self.new_line_chr;
        let old_tab = self.tab.clone();
        self.tab += &self.tab_char;
        for child in node.iter() {
            self.output += &self.tab;
            self.print(&child)?;
            self.output += &self.new_line_chr;
        }
        self.tab = old_tab;
        Ok(())
    }

    fn while_statement(&mut self, node: &Node<Powershell>) -> MinusOneResult<()> {
        if let Some(condition) = node.named_child("condition") {
            // dead code elysium
            if condition.data() != Some(&Raw(Bool(false))) {
                self.space_sep(node, None)?
            }
        }
        Ok(())
    }

    fn if_statement(&mut self, node: &Node<Powershell>) -> MinusOneResult<()> {
        if let Some(condition) = node.named_child("condition") {
            // dead code elysium
            // this branch is the only one
            if let Some(&Raw(Bool(bool_condition))) = condition.data() {
                if bool_condition {
                    let statement_block = node.child(4).ok_or(Error::invalid_child())?;
                    if let Some(statement_list) = statement_block.named_child("statement_list") {
                        self.statement_list(&statement_list)?;
                    }
                }
                else {
                    if let Some(elsif_clauses) = node.named_child("elseif_clauses") {
                        // handle multiple elseif clause
                        for elsif_clause in elsif_clauses.iter() {
                            if let Some(elsif_condition) = elsif_clause.named_child("condition") {

                                match elsif_condition.data() {
                                    Some(&Raw(Bool(true))) => {
                                        let statement_block = elsif_clause.child(4).ok_or(Error::invalid_child())?;
                                        if let Some(statement_list) = statement_block.named_child("statement_list") {
                                            self.statement_list(&statement_list)?;
                                        }
                                        return Ok(());
                                    },
                                    Some(&Raw(Bool(false))) => {
                                        continue;
                                    },
                                    _ => {
                                        self.space_sep(node, None)?;
                                        return Ok(());
                                    }
                                }
                            }
                            else {
                                // Normal rendering
                                self.space_sep(node, None)?;
                                return Ok(());
                            }
                        }
                    }

                    if let Some(else_clause) = node.named_child("else_clause") {
                        // in this case the else clause is the only branch that will be printed
                        let statement_block = else_clause.child(1).ok_or(Error::invalid_child())?;
                        if let Some(statement_list) = statement_block.named_child("statement_list") {
                            self.statement_list(&statement_list)?;
                        }
                    }
                    // Here there is nothing to print as the if condition is always false and no fallback condition
                }
            }
            else {
                // Normal rendering
                self.space_sep(node, None)?;
            }
        }
        Ok(())
    }

    fn expandable_string_literal(&mut self, node: &Node<Powershell>) -> MinusOneResult<()> {
        let mut result = String::new();
        let mut index = 0;
        for child in node.iter() {
            result.push_str(&node.text()?[index..child.start()]);
            index = child.end();
            match child.data(){
                Some(Raw(Str(s))) => result.push_str(&s),
                Some(Raw(Num(v))) => result.push_str(&v.to_string()),

                _ => {
                    if child.kind() == "sub_expression" {
                        // invoke linter in case of subexpression to output accurate deobfuscation
                        let mut linter = Self::new();
                        linter.print(&child)?;
                        // Don't escape char in subexpression...
                        // Powershell allows both but with tree-sitter we can't
                        result.push_str(&linter.output);
                    }
                    else {
                        result.push_str(child.text()?)
                    }
                }
            }
        }
        // add the end
        result.push_str(&node.text()?[index..]);
        self.output += &result;
        Ok(())
    }
}