use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::printer::{Printer, PrinterMode};
use crate::tree::{Storage, Tree};

pub struct JavaScriptPrinter {
    indent: String,
}

impl Default for JavaScriptPrinter {
    fn default() -> Self {
        Self {
            indent: "    ".to_string(),
        }
    }
}

impl JavaScriptPrinter {
    pub fn with_indent(indent: &str) -> Self {
        Self {
            indent: indent.to_string(),
        }
    }

    fn write_indent(output: &mut String, level: usize, indent: &str) {
        for _ in 0..level {
            output.push_str(indent);
        }
    }

    fn trim_trailing_spaces(output: &mut String) {
        while output.ends_with(' ') || output.ends_with('\t') {
            output.pop();
        }
    }

    fn ensure_newline(output: &mut String) {
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }

    fn last_non_whitespace_char(output: &str) -> Option<char> {
        output.chars().rev().find(|c| !c.is_whitespace())
    }

    fn on_blank_line(output: &str) -> bool {
        output
            .rsplit('\n')
            .next()
            .is_some_and(|tail| tail.chars().all(|c| c == ' ' || c == '\t'))
    }

    fn peek_next_non_whitespace_char(
        chars: &std::iter::Peekable<std::str::Chars<'_>>,
    ) -> Option<char> {
        let mut clone = chars.clone();
        clone.find(|c| !c.is_whitespace())
    }

    fn peek_next_word(chars: &std::iter::Peekable<std::str::Chars<'_>>) -> Option<String> {
        let mut clone = chars.clone();
        let first = clone.find(|c| !c.is_whitespace())?;
        if !first.is_ascii_alphabetic() {
            return None;
        }
        let mut word = String::from(first);
        for c in clone {
            if !c.is_ascii_alphabetic() {
                break;
            }
            word.push(c);
        }
        Some(word)
    }

    fn last_word(output: &str) -> Option<&str> {
        let trimmed = output.trim_end_matches([' ', '\t']);
        let start = trimmed
            .rfind(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
            .map_or(0, |i| i + trimmed[i..].chars().next().unwrap().len_utf8());
        let word = &trimmed[start..];
        (!word.is_empty()).then_some(word)
    }

    fn is_control_line(line: &str) -> bool {
        let trimmed = line.trim_start();
        [
            "if(",
            "if ",
            "for(",
            "for ",
            "while(",
            "while ",
            "switch(",
            "switch ",
            "catch(",
            "catch ",
            "function ",
            "class ",
            "try",
            "else",
            "do",
            "finally",
            "case ",
            "default",
        ]
        .iter()
        .any(|k| trimmed.starts_with(k))
    }

    fn should_add_semicolon(line: &str) -> bool {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return false;
        }

        if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
            return false;
        }

        if Self::is_control_line(trimmed) {
            return false;
        }

        // Avoid adding semicolons to object-literal/destructuring property lines like `execSync: F`
        // which would break patterns such as `{ execSync: F } = require(...)` when pretty-printing.
        if trimmed.contains(':')
            && !trimmed.ends_with(':')
            && !trimmed.contains('?')
            && !trimmed.starts_with("http")
        {
            return false;
        }

        if [
            "+", "-", "*", "/", "%", "=", "==", "===", "!=", "!==", "<", ">", "<=", ">=", "&&",
            "||", "??", "?", ":", "&", "|", "^", "<<", ">>", ">>>", ",", ".",
        ]
        .iter()
        .any(|op| trimmed.ends_with(op))
        {
            return false;
        }

        !trimmed.ends_with(';')
            && !trimmed.ends_with('{')
            && !trimmed.ends_with('}')
            && !trimmed.ends_with(':')
            && !trimmed.ends_with(',')
    }

    fn add_missing_semicolons_multiline(source: &str) -> String {
        let mut output = String::new();
        for line in source.lines() {
            let trimmed_right = line.trim_end();
            if Self::should_add_semicolon(trimmed_right) {
                output.push_str(trimmed_right);
                output.push(';');
            } else {
                output.push_str(trimmed_right);
            }
            output.push('\n');
        }

        output.trim().to_string()
    }

    fn compact_source(source: &str) -> String {
        let mut out = String::new();
        let mut chars = source.chars().peekable();
        let mut logical_line = String::new();

        let mut in_string: Option<char> = None;
        let mut escaped = false;
        let mut in_line_comment = false;
        let mut in_block_comment = false;

        while let Some(ch) = chars.next() {
            if in_line_comment {
                if ch == '\n' {
                    let candidate = logical_line.trim_end();
                    if Self::should_add_semicolon(candidate) && !out.ends_with(';') {
                        out.push(';');
                    }
                    logical_line.clear();
                    in_line_comment = false;
                    if !out.ends_with(' ') && !out.ends_with(';') {
                        out.push(' ');
                    }
                } else {
                    out.push(ch);
                }
                continue;
            }

            if in_block_comment {
                out.push(ch);
                if ch == '*' && chars.peek() == Some(&'/') {
                    out.push('/');
                    let _ = chars.next();
                    in_block_comment = false;
                }
                continue;
            }

            if let Some(quote) = in_string {
                out.push(ch);
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == quote {
                    in_string = None;
                }
                continue;
            }

            match ch {
                '\'' | '"' | '`' => {
                    out.push(ch);
                    logical_line.push(ch);
                    in_string = Some(ch);
                }
                '/' => {
                    if chars.peek() == Some(&'/') {
                        out.push('/');
                        out.push('/');
                        let _ = chars.next();
                        in_line_comment = true;
                    } else if chars.peek() == Some(&'*') {
                        out.push('/');
                        out.push('*');
                        let _ = chars.next();
                        in_block_comment = true;
                    } else {
                        out.push('/');
                        logical_line.push('/');
                    }
                }
                '}' => {
                    let candidate = logical_line.trim_end();
                    let tail_statement = candidate.rsplit('{').next().unwrap_or(candidate).trim();
                    if Self::should_add_semicolon(tail_statement) && !out.ends_with(';') {
                        out.push(';');
                    }
                    out.push('}');
                    logical_line.clear();
                    logical_line.push('}');
                }
                '\n' | '\r' => {
                    let candidate = logical_line.trim_end();
                    if Self::should_add_semicolon(candidate) && !out.ends_with(';') {
                        out.push(';');
                    }
                    logical_line.clear();
                }
                '\t' | ' ' => {
                    let next = chars.peek().copied().unwrap_or('\0');
                    let prev = out.chars().last().unwrap_or('\0');
                    let prev_word = prev.is_ascii_alphanumeric() || prev == '_' || prev == '$';
                    let next_word = next.is_ascii_alphanumeric() || next == '_' || next == '$';

                    if prev_word && next_word && !out.ends_with(' ') {
                        out.push(' ');
                        logical_line.push(' ');
                    }
                }
                _ => {
                    out.push(ch);
                    logical_line.push(ch);
                }
            }
        }

        let candidate = logical_line.trim_end();
        if Self::should_add_semicolon(candidate) && !out.ends_with(';') {
            out.push(';');
        }

        out.trim().to_string()
    }

    fn pretty_print_source(&self, source: &str) -> String {
        let mut output = String::new();
        let mut chars = source.chars().peekable();

        let mut indent_level = 0usize;
        let mut paren_depth = 0usize;

        let mut in_string: Option<char> = None;
        let mut escaped = false;
        let mut in_line_comment = false;
        let mut in_block_comment = false;
        let mut brace_inline_stack: Vec<bool> = vec![];

        while let Some(ch) = chars.next() {
            if in_line_comment {
                output.push(ch);
                if ch == '\n' {
                    in_line_comment = false;
                    Self::write_indent(&mut output, indent_level, &self.indent);
                }
                continue;
            }

            if in_block_comment {
                output.push(ch);
                if ch == '*' && chars.peek() == Some(&'/') {
                    output.push('/');
                    let _ = chars.next();
                    in_block_comment = false;
                }
                continue;
            }

            if let Some(quote) = in_string {
                output.push(ch);
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == quote {
                    in_string = None;
                }
                continue;
            }

            match ch {
                '\'' | '"' | '`' => {
                    in_string = Some(ch);
                    output.push(ch);
                }
                '/' => {
                    if chars.peek() == Some(&'/') {
                        output.push('/');
                        output.push('/');
                        let _ = chars.next();
                        in_line_comment = true;
                    } else if chars.peek() == Some(&'*') {
                        output.push('/');
                        output.push('*');
                        let _ = chars.next();
                        in_block_comment = true;
                    } else {
                        output.push('/');
                    }
                }
                '(' => {
                    paren_depth += 1;
                    if Self::last_word(&output) == Some("function") {
                        Self::trim_trailing_spaces(&mut output);
                    }
                    output.push('(');
                }
                ')' => {
                    paren_depth = paren_depth.saturating_sub(1);
                    output.push(')');
                }
                '{' => {
                    let prev = Self::last_non_whitespace_char(&output);
                    let is_inline_brace = matches!(
                        prev,
                        Some('=') | Some(',') | Some('(') | Some('[') | Some(':')
                    );
                    let is_empty_block = !is_inline_brace
                        && Self::peek_next_non_whitespace_char(&chars) == Some('}');

                    if is_inline_brace || is_empty_block {
                        Self::trim_trailing_spaces(&mut output);
                        if is_empty_block && !output.ends_with('\n') && !output.is_empty() {
                            output.push(' ');
                        }
                        output.push('{');
                        brace_inline_stack.push(true);
                        if is_empty_block {
                            while matches!(chars.peek(), Some(c) if c.is_whitespace()) {
                                chars.next();
                            }
                        }
                    } else {
                        Self::trim_trailing_spaces(&mut output);
                        if !output.ends_with('\n') && !output.is_empty() {
                            output.push(' ');
                        }
                        output.push('{');
                        output.push('\n');
                        indent_level += 1;
                        Self::write_indent(&mut output, indent_level, &self.indent);
                        brace_inline_stack.push(false);
                    }
                }
                '}' => {
                    let is_inline_brace = brace_inline_stack.pop().unwrap_or(false);
                    if is_inline_brace {
                        Self::trim_trailing_spaces(&mut output);
                        output.push('}');
                    } else {
                        Self::trim_trailing_spaces(&mut output);
                        Self::ensure_newline(&mut output);
                        indent_level = indent_level.saturating_sub(1);
                        Self::write_indent(&mut output, indent_level, &self.indent);
                        output.push('}');

                        if matches!(chars.peek(), Some(';')) {
                            continue;
                        }

                        let same_line = matches!(
                            Self::peek_next_word(&chars).as_deref(),
                            Some("else") | Some("catch") | Some("finally")
                        ) || Self::peek_next_non_whitespace_char(&chars)
                            == Some('=');

                        if same_line {
                            while matches!(chars.peek(), Some(c) if c.is_whitespace()) {
                                chars.next();
                            }
                            output.push(' ');
                        } else if chars.peek().is_some() {
                            output.push('\n');
                            Self::write_indent(&mut output, indent_level, &self.indent);
                        }
                    }
                }
                ';' => {
                    output.push(';');
                    if paren_depth == 0 {
                        output.push('\n');
                        Self::write_indent(&mut output, indent_level, &self.indent);
                    }
                }
                ':' => {
                    output.push(':');
                    output.push(' ');
                }
                ',' => {
                    output.push(',');
                    if paren_depth == 0 {
                        output.push(' ');
                    }
                }
                '\n' | '\r' => {
                    if paren_depth == 0 {
                        if !Self::on_blank_line(&output) {
                            Self::trim_trailing_spaces(&mut output);
                            output.push('\n');
                            Self::write_indent(&mut output, indent_level, &self.indent);
                        }
                    } else if !output.ends_with(' ') && !output.ends_with('\n') {
                        output.push(' ');
                    }
                }
                '\t' | ' ' => {
                    if !output.ends_with(' ') && !output.ends_with('\n') {
                        output.push(' ');
                    }
                }
                _ => output.push(ch),
            }
        }

        let pretty = output.trim().to_string();
        Self::add_missing_semicolons_multiline(&pretty)
    }
}

impl Printer for JavaScriptPrinter {
    type Language = JavaScript;

    fn print<S>(&mut self, tree: &Tree<'_, S>, mode: PrinterMode) -> MinusOneResult<String>
    where
        S: Storage<Component = Self::Language> + Default,
    {
        let source = tree.root()?.text()?.to_string();

        match mode {
            PrinterMode::Pretty => Ok(self.pretty_print_source(&source)),
            PrinterMode::Compact => Ok(Self::compact_source(&source)),
            PrinterMode::Unchanged => Ok(source),
        }
    }
}
