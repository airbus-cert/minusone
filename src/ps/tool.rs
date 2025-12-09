use crate::{ps::Powershell, tree::Node};

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
        } else {
            self
        }
    }

    fn uppercase_first(self) -> String {
        let mut v = self.to_lowercase();
        let s = v.get_mut(0..1);
        if let Some(s) = s {
            s.make_ascii_uppercase()
        }
        v
    }

    fn normalize(self) -> Self {
        self.to_lowercase().remove_tilt().remove_quote()
    }
}

pub trait CommandTool<'a> {
    fn is_command(&self) -> bool;
    fn get_command_name(&self) -> String;
    fn get_command_args(&self) -> Vec<Node<'a, Powershell>>;
    fn get_command_params(&self) -> Vec<Node<'a, Powershell>>;
}

impl<'a> CommandTool<'a> for Node<'a, Powershell> {
    fn is_command(&self) -> bool {
        self.kind() == "command"
    }

    fn get_command_name(&self) -> String {
        self.child(0).unwrap().text().unwrap().to_owned()
    }

    fn get_command_args(&self) -> Vec<Node<'a, Powershell>> {
        self.child(1)
            .unwrap()
            .iter()
            .filter(|n| n.kind() != "command_argument_sep" && n.kind() != "command_parameter")
            .collect()
    }

    fn get_command_params(&self) -> Vec<Node<'a, Powershell>> {
        self.child(1)
            .unwrap()
            .iter()
            .skip(1)
            .filter(|n| n.kind() == "command_parameter")
            .collect()
    }
}
