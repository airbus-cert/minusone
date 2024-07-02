use ps::Powershell;
use tree::Node;
use error::{MinusOneResult, Error};
use ps::Powershell::Raw;
use ps::Value::{Str, Num, Bool};
use rule::Rule;

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

fn remove_useless_token(src: &str) -> String {
    let mut result = String::new();
    for c in src.chars() {
        if c != '`' {
            result.push(c);
        }
    }
    result
}

fn is_inline<T>(node: &Node<T>) -> bool {
    !node.get_parent_of_types(vec!["pipeline"]).is_none()
}

pub struct Linter {
    pub output: String,
    tab: Vec<String>,
    tab_char: String,
    new_line_chr: String,
    comment: bool,
    is_param_block: bool,
    is_first_statement: Vec<bool>
}

impl<'a> Rule<'a> for Linter {
    type Language = Powershell;

    fn enter(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<bool>{

        // depending on what am i
        match node.kind() {
            "statement_block" | "script_block" => {
                if !is_inline(node) {
                    self.tab();
                }
            },
            // ignore this node
            "command_argument_sep" | "empty_statement" => return Ok(false),
            // ignore comment only if it was requested
            "comment" => return Ok(self.comment),
            "command_invokation_operator" => {
                // normalize operator
                self.output += "& ";
                return Ok(false);
            },
            // add a new line space before special statement
            "while_statement" | "if_statement" | "function_statement" => self.output += &self.new_line_chr,
            "param_block" => {
                self.is_param_block = true
            },
            "attribute" | "variable" => {
                if self.is_param_block {
                    self.output += &self.new_line_chr;
                    self.output += &self.current_tab().clone();
                }
            },
            "statement_list" => self.is_first_statement.push(false),
            _ => ()
        }

        // depending on what my parent are
        if let Some(parent) = node.parent() {
            match parent.kind() {
                "statement_list" => {
                    if *self.is_first_statement.last().unwrap_or(&true) {
                        // check inline statement by checking parent
                        if is_inline(&parent) {
                            self.output += " ";
                        }
                        else {
                            self.output += &self.new_line_chr;
                            self.output += &self.current_tab().clone();
                        }
                    }
                    else {
                        *self.is_first_statement.last_mut().unwrap() = true;
                    }
                },
                "command_elements" => self.output += " ",
                "param_block" => {
                    if node.text()? == "(" {
                        self.tab();
                    }
                    else if node.text()? == ")" {
                        self.untab();
                        self.output += &self.new_line_chr;
                        self.output += &self.current_tab().clone();
                    }
                }
                _ => ()
            }
        }

        // Special token
        if node.child_count() == 0 {
            match node.text()?.to_lowercase().as_str() {
                "{" => self.output += " ",
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
                "-file" => self.output += " ",
                "catch" | "finally" | "else" | "elseif" |
                //  begin process end are not statements
                "begin" | "process" | "end" | "param" => {
                    if is_inline(node) {
                        self.output += " ";
                    }
                    else {
                        self.output += &self.new_line_chr;
                        self.output += &self.current_tab().clone();
                    }
                }
                "}" => {
                    if is_inline(node) {
                        self.output += " ";
                    }
                    else {
                        self.output += &self.new_line_chr;
                        self.untab();
                        self.output += &self.current_tab().clone();
                    }
                },
                _ => ()
            }
        }

        // inferred type
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
                    return Ok(false);
                }
                Raw(Num(number)) => {
                    self.output.push_str(number.to_string().as_str());
                    return Ok(false);
                },
                Raw(Bool(true)) => {
                    self.output.push_str("$true".to_string().as_str());
                    return Ok(false)
                },
                Raw(Bool(false)) => {
                    self.output.push_str("$false".to_string().as_str());
                    return Ok(false)
                },
                _ => ()
            }
        }

        Ok(true)
    }

    /// During the down to top travel we will manage the tab decrement
    fn leave(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<()> {

        match node.kind() {
            "param_block" => self.is_param_block = false,
            "statement_list" => {
                if !self.is_first_statement.is_empty(){
                    self.is_first_statement.pop();
                }
            },
            _ => ()
        }

        // leaf node => just print the token
        if node.child_count() == 0 {
            self.output += &remove_useless_token(&node.text()?.to_lowercase());
        }

        // depending on what my parent are
        if let Some(parent) = node.parent() {
            match parent.kind() {
                "statement_list" => {
                    // check inline statement by checking parent
                    if is_inline(&parent) {
                        self.output += ";";
                    }
                },
                _ => ()
            }
        }

        // post process token
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
                "param" | "-regex" | "-wildcard" |
                "-exact" | "-caseinsensitive" | "-parallel" |
                "-file" |  "," |
                "function" | "if" | "while" | "else" |
                "elseif" | "switch" | "foreach" | "for" | "do" |
                "filter" | "workflow" | "try" => self.output += " ",
                _ => ()
            }
        }

        Ok(())
    }
}

impl Linter {
    pub fn new() -> Self {
        Linter {
            output: String::new(),
            tab: vec!["".to_string()],
            tab_char: " ".to_string(),
            new_line_chr: "\n".to_string(),
            comment: false,
            is_param_block: false,
            is_first_statement: vec![]
        }
    }

    pub fn tab(&mut self) {
        let current = self.current_tab().clone() + &self.tab_char;
        self.tab.push(current);
    }

    pub fn current_tab(&self) -> String {
        self.tab.last().map_or("".to_string(), |x| x.clone())
    }

    pub fn untab(&mut self) {
        if !self.tab.is_empty() {
            self.tab.pop().unwrap();
        }
    }

    pub fn set_tab(mut self, tab_chr: &str) -> Self{
        self.tab_char = tab_chr.to_string();
        self
    }

    pub fn set_comment(mut self, comment: bool) -> Self{
        self.comment = comment;
        self
    }
}