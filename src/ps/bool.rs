use rule::RuleMut;
use ps::Powershell;
use tree::NodeMut;
use error::MinusOneResult;
use ps::Value::{Bool, Str, Num};
use ps::Powershell::Raw;

#[derive(Default)]
pub struct ParseBool;

impl<'a> RuleMut<'a> for ParseBool {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        let view = node.view();
        // Booleans in powershell are variables
        if view.kind() == "variable" {
            match view.text()?.to_lowercase().as_str() {
                "$true" => node.set(Raw(Bool(true))),
                "$false" => node.set(Raw(Bool(false))),
                _ => ()
            }
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct Comparison;

impl<'a> RuleMut<'a> for Comparison {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        let view = node.view();
        // Booleans in powershell are variables
        if view.kind() == "comparison_expression" {
            if let (Some(left_node), Some(operator), Some(right_node)) = (view.child(0), view.child(1), view.child(2)) {
                match (left_node.data(), operator.text()?.to_lowercase().as_str(), right_node.data()) {
                    // String comparison
                    (Some(Raw(Str(left_value))), "-eq", Some(Raw(Str(right_value)))) => node.set(Raw(Bool(left_value == right_value))),
                    (Some(Raw(Str(left_value))), "-ne", Some(Raw(Str(right_value)))) => node.set(Raw(Bool(left_value != right_value))),
                    (Some(Raw(Str(left_value))), "-ge", Some(Raw(Str(right_value)))) => node.set(Raw(Bool(left_value >= right_value))),
                    (Some(Raw(Str(left_value))), "-gt", Some(Raw(Str(right_value)))) => node.set(Raw(Bool(left_value > right_value))),
                    (Some(Raw(Str(left_value))), "-le", Some(Raw(Str(right_value)))) => node.set(Raw(Bool(left_value <= right_value))),
                    (Some(Raw(Str(left_value))), "-lt", Some(Raw(Str(right_value)))) => node.set(Raw(Bool(left_value < right_value))),

                    // Integer comparison
                    (Some(Raw(Num(left_value))), "-eq", Some(Raw(Num(right_value)))) => node.set(Raw(Bool(left_value == right_value))),
                    (Some(Raw(Num(left_value))), "-ne", Some(Raw(Num(right_value)))) => node.set(Raw(Bool(left_value != right_value))),
                    (Some(Raw(Num(left_value))), "-ge", Some(Raw(Num(right_value)))) => node.set(Raw(Bool(left_value >= right_value))),
                    (Some(Raw(Num(left_value))), "-gt", Some(Raw(Num(right_value)))) => node.set(Raw(Bool(left_value > right_value))),
                    (Some(Raw(Num(left_value))), "-le", Some(Raw(Num(right_value)))) => node.set(Raw(Bool(left_value <= right_value))),
                    (Some(Raw(Num(left_value))), "-lt", Some(Raw(Num(right_value)))) => node.set(Raw(Bool(left_value < right_value))),

                    // Boolean comparison
                    // Seems to be standardized with Rust???
                    (Some(Raw(Bool(left_value))), "-eq", Some(Raw(Bool(right_value)))) => node.set(Raw(Bool(left_value == right_value))),
                    (Some(Raw(Bool(left_value))), "-ne", Some(Raw(Bool(right_value)))) => node.set(Raw(Bool(left_value != right_value))),
                    (Some(Raw(Bool(left_value))), "-ge", Some(Raw(Bool(right_value)))) => node.set(Raw(Bool(left_value >= right_value))),
                    (Some(Raw(Bool(left_value))), "-gt", Some(Raw(Bool(right_value)))) => node.set(Raw(Bool(left_value > right_value))),
                    (Some(Raw(Bool(left_value))), "-le", Some(Raw(Bool(right_value)))) => node.set(Raw(Bool(left_value <= right_value))),
                    (Some(Raw(Bool(left_value))), "-lt", Some(Raw(Bool(right_value)))) => node.set(Raw(Bool(left_value < right_value))),

                    // Mixed type comparison
                    // Str and bool comparison
                    (Some(Raw(Str(left_value))), "-eq", Some(Raw(Bool(right_value)))) => {
                        node.set(Raw(Bool((left_value == "True" && *right_value == true) || (left_value == "False" && *right_value == false))))
                    },
                    (Some(Raw(Bool(left_value))), "-eq", Some(Raw(Str(right_value)))) => {
                        node.set(Raw(Bool((right_value.len() != 0 && *left_value) || (right_value.len() == 0 && !*left_value))))
                    },
                    (Some(Raw(Str(left_value))), "-ne", Some(Raw(Bool(right_value)))) => {
                        node.set(Raw(Bool(!((left_value == "True" && *right_value == true) || (left_value == "False" && *right_value == false)))))
                    },
                    (Some(Raw(Bool(left_value))), "-ne", Some(Raw(Str(right_value)))) => {
                        node.set(Raw(Bool(!((right_value.len() != 0 && *left_value) || (right_value.len() == 0 && !*left_value)))))
                    },

                    // true or false compare to string
                    (Some(Raw(Bool(true))), "-gt", Some(Raw(Str(right_value)))) => {
                        node.set(Raw(Bool(right_value.len()==0)))
                    },
                    (Some(Raw(Bool(true))), "-ge", Some(Raw(Str(_)))) => {
                        node.set(Raw(Bool(true)))
                    },
                    (Some(Raw(Bool(false))), "-gt", Some(Raw(_))) => {
                        node.set(Raw(Bool(false)))
                    },
                    (Some(Raw(Bool(false))), "-ge", Some(Raw(Str(right_value)))) => {
                        node.set(Raw(Bool(right_value.len()==0)))
                    },

                    // String to number comparison
                    (Some(Raw(Str(left_value))), "-eq", Some(Raw(Num(right_value)))) => node.set(Raw(Bool(*left_value == right_value.to_string()))),
                    (Some(Raw(Str(left_value))), "-ne", Some(Raw(Num(right_value)))) => node.set(Raw(Bool(*left_value != right_value.to_string()))),
                    (Some(Raw(Str(left_value))), "-ge", Some(Raw(Num(right_value)))) => node.set(Raw(Bool(*left_value >= right_value.to_string()))),
                    (Some(Raw(Str(left_value))), "-gt", Some(Raw(Num(right_value)))) => node.set(Raw(Bool(*left_value > right_value.to_string()))),
                    (Some(Raw(Str(left_value))), "-le", Some(Raw(Num(right_value)))) => node.set(Raw(Bool(*left_value <= right_value.to_string()))),
                    (Some(Raw(Str(left_value))), "-lt", Some(Raw(Num(right_value)))) => node.set(Raw(Bool(*left_value < right_value.to_string()))),

                    // number to string comparison
                    (Some(Raw(Num(left_value))), "-eq", Some(Raw(Str(right_value)))) => node.set(Raw(Bool(left_value.to_string() == *right_value))),
                    (Some(Raw(Num(left_value))), "-ne", Some(Raw(Str(right_value)))) => node.set(Raw(Bool(left_value.to_string() != *right_value))),
                    (Some(Raw(Num(left_value))), "-ge", Some(Raw(Str(right_value)))) => node.set(Raw(Bool(left_value.to_string() >= *right_value))),
                    (Some(Raw(Num(left_value))), "-gt", Some(Raw(Str(right_value)))) => node.set(Raw(Bool(left_value.to_string() > *right_value))),
                    (Some(Raw(Num(left_value))), "-le", Some(Raw(Str(right_value)))) => node.set(Raw(Bool(left_value.to_string() <= *right_value))),
                    (Some(Raw(Num(left_value))), "-lt", Some(Raw(Str(right_value)))) => node.set(Raw(Bool(left_value.to_string() < *right_value))),

                    _ => ()
                }
            }

        }
        Ok(())
    }
}