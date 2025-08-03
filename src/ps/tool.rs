pub trait StringTool {
    fn remove_tilt(self) -> Self;
    fn remove_quote(self) -> Self;
    fn uppercase_first(self) -> Self;

    fn normalize(self) -> Self;
}

impl StringTool for String {
    fn remove_tilt(self) -> Self {
        self.replace("‘", "")
    }

    fn remove_quote(self) -> Self {
        if let (Some('"'), Some('"')) = (self.chars().nth(0), self.chars().nth(self.len())) {
            self[1..self.len() - 1].to_string()
        }
        else {
            self
        }
    }

    fn uppercase_first(self) -> String {
        let mut v = self.to_lowercase();
        let s = v.get_mut(0..1);
        if let Some(s) = s { s.make_ascii_uppercase() }
        v
    }

    fn normalize(self) -> Self {
        self.to_lowercase().remove_tilt().remove_quote()
    }
}