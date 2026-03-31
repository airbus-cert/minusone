use crate::error::{Error, MinusOneErrorKind, MinusOneResult};
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

    fn pretty_print_source(&self, source: &str) -> String {
        let mut output = String::new();
        let mut chars = source.chars().peekable();

        let mut indent_level = 0usize;
        let mut paren_depth = 0usize;

        let mut in_string: Option<char> = None;
        let mut escaped = false;
        let mut in_line_comment = false;
        let mut in_block_comment = false;

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
                    output.push('(');
                }
                ')' => {
                    paren_depth = paren_depth.saturating_sub(1);
                    output.push(')');
                }
                '{' => {
                    Self::trim_trailing_spaces(&mut output);
                    if !output.ends_with('\n') && !output.is_empty() {
                        output.push(' ');
                    }
                    output.push('{');
                    output.push('\n');
                    indent_level += 1;
                    Self::write_indent(&mut output, indent_level, &self.indent);
                }
                '}' => {
                    Self::trim_trailing_spaces(&mut output);
                    Self::ensure_newline(&mut output);
                    indent_level = indent_level.saturating_sub(1);
                    Self::write_indent(&mut output, indent_level, &self.indent);
                    output.push('}');

                    if matches!(chars.peek(), Some(';')) {
                        continue;
                    }
                    if chars.peek().is_some() {
                        output.push('\n');
                        Self::write_indent(&mut output, indent_level, &self.indent);
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
                '\n' | '\r' | '\t' | ' ' => {
                    if !output.ends_with(' ') && !output.ends_with('\n') {
                        output.push(' ');
                    }
                }
                _ => output.push(ch),
            }
        }

        output.trim().to_string()
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
            PrinterMode::Compact => Err(Error::new(
                MinusOneErrorKind::Unknown,
                "Compact JavaScript printer mode is not implemented",
            )),
            PrinterMode::Unchanged => Ok(source),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::error::Error;
    use crate::js::build_javascript_tree;
    use crate::js::printer::JavaScriptPrinter;
    use crate::printer::{Printer, PrinterMode};

    #[test]
    fn test_js_printer_unchanged_returns_original_source() {
        let src = "function a(){return 1+2;}";
        let tree = build_javascript_tree(src).unwrap();
        let mut printer = JavaScriptPrinter::default();

        let output = printer.print(&tree, PrinterMode::Unchanged).unwrap();

        assert_eq!(output, src);
    }

    #[test]
    fn test_js_printer_compact_is_unimplemented() {
        let tree = build_javascript_tree("const x=1;").unwrap();
        let mut printer = JavaScriptPrinter::default();

        let result = printer.print(&tree, PrinterMode::Compact);

        assert!(result.is_err());
        match result {
            Err(Error::MinusOneError(err)) => {
                assert!(err.message.contains("not implemented"));
            }
            _ => panic!("Expected MinusOneError for compact mode"),
        }
    }

    #[test]
    fn test_js_printer_pretty_adds_basic_structure() {
        let tree = build_javascript_tree("function a(){if(true){x=1;}} ").unwrap();
        let mut printer = JavaScriptPrinter::with_indent("  ");

        let output = printer.print(&tree, PrinterMode::Pretty).unwrap();
        println!("Pretty Printed Output:\n{}", output);

        assert!(output.contains("function a() {"));
        assert!(output.contains("\n  if(true) {"));
        assert!(output.contains("\n    x=1;"));
    }
}
