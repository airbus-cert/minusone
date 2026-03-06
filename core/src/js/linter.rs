use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::rule::Rule;
use crate::tree::Node;

#[derive(Default)]
struct RemoveCode {
    source: String,
    pub output: String,
    last_index: usize,
}

impl RemoveCode {
    // todo: factorize with powershell
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

/// Removes single-line (`//`) and multi-line (`/* */`) comments from JavaScript source.
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
    type Language = JavaScript;

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

    fn leave(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<()> {
        if node.kind() == "program" {
            self.manager.end_program()?;
        }
        Ok(())
    }
}

/// Reconstructs the JavaScript source while preserving original whitespace
#[derive(Default)]
pub struct Linter {
    pub output: String,
    source: String,
    last_index: usize,
}

impl Linter {
    fn copy_until(&mut self, end: usize) {
        if end > self.last_index {
            self.output += &self.source[self.last_index..end];
        }
        self.last_index = end;
    }
    fn skip_until(&mut self, end: usize) {
        self.last_index = end;
    }
}

impl<'a> Rule<'a> for Linter {
    type Language = JavaScript;

    fn enter(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<bool> {
        match node.kind() {
            "program" => {
                self.source = node.text()?.to_string();
                self.last_index = 0;
                return Ok(true);
            }
            "comment" => {
                self.copy_until(node.start_abs());
                self.skip_until(node.end_abs());
                return Ok(false);
            }
            _ => (),
        }

        if let Some(data) = node.data() {
            self.copy_until(node.start_abs());
            self.output += &data.to_string();
            self.skip_until(node.end_abs());
            return Ok(false);
        }

        Ok(true)
    }

    fn leave(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<()> {
        if node.kind() == "program" {
            let len = self.source.len();
            self.copy_until(len);
        }
        Ok(())
    }
}
