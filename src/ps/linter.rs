use crate::error::MinusOneResult;
use crate::ps::tool::StringTool;
use crate::ps::var::{find_variable_node, UnusedVar, Var};
use crate::ps::Powershell;
use crate::ps::Powershell::Raw;
use crate::ps::Value::{Bool, Num, Str};
use crate::regex::Regex;
use crate::rule::Rule;
use crate::tree::Node;

fn escape_string(src: &str) -> String {
    let mut result = String::new();
    let mut previous = None;
    for c in src.chars() {
        if c == '"' && previous != Some('`') {
            result.push('`');
        }
        result.push(c);
        previous = Some(c);
    }
    result
}

fn remove_useless_token(src: &str) -> String {
    src.replace("`", "")
}

fn uppercase_first(src: &str) -> String {
    let mut v = src.to_lowercase();
    let s = v.get_mut(0..1);
    if let Some(s) = s {
        s.make_ascii_uppercase()
    }
    v
}

fn is_inline<T>(node: &Node<T>) -> bool {
    node.get_parent_of_types(vec!["pipeline"]).is_some()
}

pub struct Linter {
    pub output: String,
    tab: Vec<String>,
    tab_char: String,
    new_line_chr: String,
    comment: bool,
    is_param_block: bool,
    statement_block_tab: Vec<bool>,
    is_multiline: bool,
}

impl<'a> Rule<'a> for Linter {
    type Language = Powershell;

    fn enter(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<bool> {
        // depending on what am i
        match node.kind() {
            "script_block" => {
                if !is_inline(node) {
                    self.tab();
                }
            }
            // ignore this node
            "command_argument_sep" | "empty_statement" => return Ok(false),
            // ignore comment only if it was requested
            "comment" => return Ok(self.comment),
            "command_invokation_operator" => {
                // normalize operator
                self.write("& ");
                return Ok(false);
            }
            // add a new line space before special statement
            "while_statement" | "if_statement" | "function_statement" => {
                self.enter();
            }
            "param_block" => self.is_param_block = true,
            "attribute" | "variable" => {
                if self.is_param_block {
                    self.enter();
                }
            }
            "statement_block" => self.statement_block_tab.push(true),
            // Normalize command name
            // If it's a Verb-Action name parse it and print it normalize
            "command_name" => {
                let re = Regex::new(r"([a-z]+)-([a-z]+)").unwrap();
                if let Some(m) = re.captures(node.text()?.to_lowercase().as_str()) {
                    if let (Some(verb), Some(action)) = (m.get(1), m.get(2)) {
                        self.write(uppercase_first(verb.as_str()).as_str());
                        self.write("-");
                        self.write(uppercase_first(action.as_str()).as_str());
                        return Ok(false);
                    }
                } else {
                    self.write(node.text()?.to_lowercase().as_str());
                    return Ok(false);
                }
            }
            // If a member_name is infered as a string
            "member_name" => {
                if let Some(Raw(Str(m))) = node.data() {
                    self.write(&m.clone().remove_tilt().uppercase_first());
                    return Ok(false);
                }
            }
            "expandable_string_literal" => {
                self.write(node.text()?);
                return Ok(false);
            }
            "type_spec" => {
                self.write(&node.text()?.to_string().uppercase_first());
                return Ok(false);
            }
            _ => (),
        }

        // depending on what my parent are
        if let Some(parent) = node.parent() {
            match parent.kind() {
                "statement_list" => {
                    // check inline statement by checking parent
                    if is_inline(&parent) {
                        self.write(" ");
                    } else {
                        self.enter();
                    }
                }
                "function_statement" => match node.kind() {
                    "}" => self.untab(),
                    "{" => self.write(" "),
                    _ => (),
                },
                "statement_block" => {
                    // tab for new block
                    if node.kind() == "statement_list" {
                        if !is_inline(node) && *self.statement_block_tab.last().unwrap_or(&true) {
                            self.tab();
                        }
                    }
                    // ignore these tokens if we are in case of code elysium
                    else if (node.kind() == "{" || node.kind() == "}")
                        && !*self.statement_block_tab.last().unwrap_or(&true)
                    {
                        return Ok(false);
                    }
                }
                "command_elements" => self.write(" "),
                "param_block" => {
                    if node.text()? == "(" {
                        self.tab();
                    } else if node.text()? == ")" {
                        self.untab();
                        self.enter();
                    }
                }

                "if_statement" => {
                    // handling if clause
                    if let Some(condition) = parent.named_child("condition") {
                        // dead code elysium
                        // this branch is the only one

                        if let Some(&Raw(Bool(bool_condition))) = condition.data() {
                            match node.kind() {
                                // this is the IF clause
                                "statement_block" => {
                                    if !bool_condition {
                                        self.statement_block_tab.pop();
                                        return Ok(false);
                                    } else if let Some(last) = self.statement_block_tab.last_mut() {
                                        *last = false;
                                    }
                                }
                                // else clause will be handle by next match
                                "else_clause" => (),
                                // every other token will not be printed
                                _ => return Ok(false),
                            }
                        }
                    }
                }
                "else_clause" => {
                    if let Some(if_statement) = parent.parent() {
                        if let Some(condition) = if_statement.named_child("condition") {
                            // dead code elysium
                            // this branch is the only one
                            if let Some(&Raw(Bool(bool_condition))) = condition.data() {
                                match node.kind() {
                                    "statement_block" => {
                                        if bool_condition {
                                            self.statement_block_tab.pop();
                                            return Ok(false);
                                        } else if let Some(last) =
                                            self.statement_block_tab.last_mut()
                                        {
                                            *last = false;
                                        }
                                    }
                                    _ => return Ok(false),
                                }
                            }
                        }
                    }
                }
                _ => (),
            }
        }

        // Special token
        if node.child_count() == 0 {
            match node.text()?.to_lowercase().as_str() {
                "=" | "!=" | "+=" | "*=" | "/=" | "%=" | "+" | "-" | "*" | "|" |
                ">" | ">>" | "2>" | "2>>" | "3>" | "3>>" | "4>" | "4>>" |
                "5>" | "5>>" | "6>" | "6>>" | "*>" | "*>>" | "<" |
                "*>&1" | "2>&1" | "3>&1" | "4>&1" | "5>&1" | "6>&1" |
                "*>&2" | "1>&2" | "3>&2" | "4>&2" | "5>&2" | "6>&2" |
                "-as" | "-ccontains" | "-ceq" |
                "-cge" | "-cgt" | "-cle" |
                "-clike" | "-clt" | "-cmatch" |
                "-cne" | "-cnotcontains" | "-cnotlike" |
                "-cnotmatch" | "-contains" | "-creplace" |
                "-csplit" | "-eq" | "-ge" |
                "-gt" | "-icontains" | "-ieq" |
                "-ige" | "-igt" | "-ile" |
                "-ilike" | "-ilt" | "-imatch" |
                "-in" | "-ine" | "-inotcontains" |
                "-inotlike" | "-inotmatch" | "-ireplace" |
                "-is" | "-isnot" | "-isplit" |
                "-join" | "-le" | "-like" |
                "-lt" | "-match" | "-ne" |
                "-notcontains" | "-notin" | "-notlike" |
                "-notmatch" | "-replace" | "-shl" |
                "-shr" | "-split" | "in" | "-f" |
                "-regex" | "-wildcard" |
                "-exact" | "-caseinsensitive" | "-parallel" |
                "-and" | "-or" | "-xor" | "-band" | "-bor" | "-bxor" |
                "until" |
                "-file" => self.write(" "),
                "catch" | "finally" | "else" | "elseif" |
                //  begin process end are not statements
                "begin" | "process" | "end" | "param" | "}" => {
                    if is_inline(node) {
                        self.write(" ");
                    } else {
                        self.enter();
                    }
                },
                _ => ()
            }
        }

        // inferred type
        if let Some(inferred_type) = node.data() {
            match inferred_type {
                Raw(Str(str)) => {
                    self.write("\"");
                    // normalisation of command
                    if node.kind() == "command_name_expr" {
                        self.write(&escape_string(&str.to_lowercase()));
                    } else {
                        self.write(&escape_string(str));
                    }
                    self.write("\"");
                    return Ok(false);
                }
                Raw(Num(number)) => {
                    self.write(number.to_string().as_str());
                    return Ok(false);
                }
                Raw(Bool(true)) => {
                    self.write("$true".to_string().as_str());
                    return Ok(false);
                }
                Raw(Bool(false)) => {
                    self.write("$false".to_string().as_str());
                    return Ok(false);
                }
                _ => (),
            }
        }
        Ok(true)
    }

    /// the down to top
    fn leave(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<()> {
        // leaf node => just print the token
        if node.child_count() == 0 {
            self.write(&remove_useless_token(node.text()?));
        }

        // depending on what my parent are
        if let Some(parent) = node.parent() {
            match parent.kind() {
                "statement_list" => {
                    // check inline statement by checking parent
                    if is_inline(&parent) {
                        self.write(";");
                    }
                }
                "statement_block" => {
                    // new statement in a block
                    if node.kind() == "statement_list"
                        && *self.statement_block_tab.last().unwrap_or(&true)
                        && !is_inline(node)
                    {
                        self.untab();
                    }
                }
                _ => (),
            }
        }

        match node.kind() {
            "param_block" => self.is_param_block = false,
            "statement_block" => {
                self.statement_block_tab.pop();
            }
            _ => (),
        }

        // post process token
        if node.child_count() == 0 {
            match node.text()?.to_lowercase().as_str() {
                "=" | "!=" | "+=" | "*=" | "/=" | "%=" | "+" | "-" | "*" | "|" | ">" | ">>"
                | "2>" | "2>>" | "3>" | "3>>" | "4>" | "4>>" | "5>" | "5>>" | "6>" | "6>>"
                | "*>" | "*>>" | "<" | "*>&1" | "2>&1" | "3>&1" | "4>&1" | "5>&1" | "6>&1"
                | "*>&2" | "1>&2" | "3>&2" | "4>&2" | "5>&2" | "6>&2" | "-as" | "-ccontains"
                | "-ceq" | "-cge" | "-cgt" | "-cle" | "-clike" | "-clt" | "-cmatch" | "-cne"
                | "-cnotcontains" | "-cnotlike" | "-cnotmatch" | "-contains" | "-creplace"
                | "-csplit" | "-eq" | "-ge" | "-gt" | "-icontains" | "-ieq" | "-ige" | "-igt"
                | "-ile" | "-ilike" | "-ilt" | "-imatch" | "-in" | "-ine" | "-inotcontains"
                | "-inotlike" | "-inotmatch" | "-ireplace" | "-is" | "-isnot" | "-isplit"
                | "-join" | "-le" | "-like" | "-lt" | "-match" | "-ne" | "-notcontains"
                | "-notin" | "-notlike" | "-notmatch" | "-replace" | "-shl" | "-shr" | "-split"
                | "in" | "-f" | "param" | "-regex" | "-wildcard" | "-exact"
                | "-caseinsensitive" | "-parallel" | "-file" | "," | "%" | "function" | "if"
                | "while" | "elseif" | "switch" | "foreach" | "for" | "do" | "filter"
                | "workflow" | "try" | "else" | "-and" | "-or" | "-xor" | "-band" | "-bor"
                | "-bxor" | "until" | "return" => self.write(" "),
                _ => (),
            }
        }

        Ok(())
    }
}

impl Default for Linter {
    fn default() -> Self {
        Self {
            output: String::new(),
            tab: vec!["".to_string()],
            tab_char: " ".to_string(),
            new_line_chr: "\n".to_string(),
            comment: false,
            is_param_block: false,
            statement_block_tab: vec![],
            is_multiline: true,
        }
    }
}

impl Linter {
    pub fn tab(&mut self) {
        let current = self.current_tab().clone() + &self.tab_char;
        self.tab.push(current);
    }

    pub fn current_tab(&self) -> String {
        self.tab.last().map_or("".to_string(), |x| x.clone())
    }

    pub fn untab(&mut self) {
        if !self.tab.is_empty() {
            self.tab.pop();
        }
    }

    pub fn enter(&mut self) {
        if !self.is_multiline {
            self.output += &self.new_line_chr;
            self.output += &self.current_tab().clone();
            self.is_multiline = true;
        }
    }

    pub fn write(&mut self, new: &str) {
        self.is_multiline = false;
        self.output += new;
    }

    pub fn set_tab(mut self, tab_chr: &str) -> Self {
        self.tab_char = tab_chr.to_string();
        self
    }

    pub fn set_comment(mut self, comment: bool) -> Self {
        self.comment = comment;
        self
    }
}

#[derive(Default)]
pub struct RemoveCode {
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
    type Language = Powershell;

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

    // the down to top
    fn leave(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<()> {
        if node.kind() == "program" {
            self.manager.end_program()?
        }

        Ok(())
    }
}

pub struct RemoveUnusedVar {
    rule: UnusedVar,
    manager: RemoveCode,
}

impl RemoveUnusedVar {
    pub fn new(rule: UnusedVar) -> Self {
        Self {
            manager: RemoveCode::default(),
            rule,
        }
    }
    pub fn clear(self) -> MinusOneResult<String> {
        Ok(self.manager.output)
    }
}

impl<'a> Rule<'a> for RemoveUnusedVar {
    type Language = ();

    fn enter(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<bool> {
        match node.kind() {
            "program" => {
                self.manager.start_program(node)?;
            }
            "assignment_expression" => {
                if let Some(var) = find_variable_node(node) {
                    if let Some(var_name) = Var::extract(var.text()?) {
                        if self.rule.is_unused(&var_name) {
                            self.manager.remove_node(node)?;
                        }
                    }
                }
            }
            _ => (),
        }
        Ok(true)
    }

    fn leave(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<()> {
        if node.kind() == "program" {
            self.manager.end_program()?
        }

        Ok(())
    }
}
