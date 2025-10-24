use crate::ps::Powershell;
use crate::ps::Powershell::Raw;
use crate::ps::Value::{Bool, Num, Str};
use crate::tree::Node;

pub fn infer_comparison(
    left: &Node<Powershell>,
    operator: &Node<Powershell>,
    right: &Node<Powershell>,
) -> Option<bool> {
    match (
        left.data(),
        operator.text().ok()?.to_lowercase().as_str(),
        right.data(),
    ) {
        // String comparison
        (Some(Raw(Str(left_value))), "-eq", Some(Raw(Str(right_value)))) => {
            Some(left_value == right_value)
        }
        (Some(Raw(Str(left_value))), "-ne", Some(Raw(Str(right_value)))) => {
            Some(left_value != right_value)
        }
        (Some(Raw(Str(left_value))), "-ge", Some(Raw(Str(right_value)))) => {
            Some(left_value >= right_value)
        }
        (Some(Raw(Str(left_value))), "-gt", Some(Raw(Str(right_value)))) => {
            Some(left_value > right_value)
        }
        (Some(Raw(Str(left_value))), "-le", Some(Raw(Str(right_value)))) => {
            Some(left_value <= right_value)
        }
        (Some(Raw(Str(left_value))), "-lt", Some(Raw(Str(right_value)))) => {
            Some(left_value < right_value)
        }

        // Integer comparison
        (Some(Raw(Num(left_value))), "-eq", Some(Raw(Num(right_value)))) => {
            Some(left_value == right_value)
        }
        (Some(Raw(Num(left_value))), "-ne", Some(Raw(Num(right_value)))) => {
            Some(left_value != right_value)
        }
        (Some(Raw(Num(left_value))), "-ge", Some(Raw(Num(right_value)))) => {
            Some(left_value >= right_value)
        }
        (Some(Raw(Num(left_value))), "-gt", Some(Raw(Num(right_value)))) => {
            Some(left_value > right_value)
        }
        (Some(Raw(Num(left_value))), "-le", Some(Raw(Num(right_value)))) => {
            Some(left_value <= right_value)
        }
        (Some(Raw(Num(left_value))), "-lt", Some(Raw(Num(right_value)))) => {
            Some(left_value < right_value)
        }

        // Boolean comparison
        // Seems to be standardized with Rust???
        (Some(Raw(Bool(left_value))), "-eq", Some(Raw(Bool(right_value)))) => {
            Some(left_value == right_value)
        }
        (Some(Raw(Bool(left_value))), "-ne", Some(Raw(Bool(right_value)))) => {
            Some(left_value != right_value)
        }
        (Some(Raw(Bool(left_value))), "-ge", Some(Raw(Bool(right_value)))) => {
            Some(left_value >= right_value)
        }
        (Some(Raw(Bool(left_value))), "-gt", Some(Raw(Bool(right_value)))) => {
            Some(left_value > right_value)
        }
        (Some(Raw(Bool(left_value))), "-le", Some(Raw(Bool(right_value)))) => {
            Some(left_value <= right_value)
        }
        (Some(Raw(Bool(left_value))), "-lt", Some(Raw(Bool(right_value)))) => {
            Some(left_value < right_value)
        }

        // Mixed type comparison
        // Str and bool comparison
        (Some(Raw(Str(left_value))), "-eq", Some(Raw(Bool(right_value)))) => Some(
            (left_value.to_lowercase() == "true" && *right_value)
                || (left_value.to_lowercase() == "false" && !(*right_value)),
        ),
        (Some(Raw(Bool(left_value))), "-eq", Some(Raw(Str(right_value)))) => Some(
            (!right_value.is_empty() && *left_value) || (right_value.is_empty() && !*left_value),
        ),
        (Some(Raw(Str(left_value))), "-ne", Some(Raw(Bool(right_value)))) => Some(
            !((left_value.to_lowercase() == "true" && *right_value)
                || (left_value.to_lowercase() == "false" && !(*right_value))),
        ),
        (Some(Raw(Bool(left_value))), "-ne", Some(Raw(Str(right_value)))) => Some(
            !((!right_value.is_empty() && *left_value) || (right_value.is_empty() && !*left_value)),
        ),

        // true or false compare to string
        (Some(Raw(Bool(true))), "-gt", Some(Raw(Str(right_value)))) => Some(right_value.is_empty()),
        (Some(Raw(Bool(true))), "-ge", Some(Raw(Str(_)))) => Some(true),
        (Some(Raw(Bool(false))), "-gt", Some(Raw(_))) => Some(false),
        (Some(Raw(Bool(false))), "-ge", Some(Raw(Str(right_value)))) => {
            Some(right_value.is_empty())
        }

        // String to number comparison
        (Some(Raw(Str(left_value))), "-eq", Some(Raw(Num(right_value)))) => {
            Some(*left_value == right_value.to_string())
        }
        (Some(Raw(Str(left_value))), "-ne", Some(Raw(Num(right_value)))) => {
            Some(*left_value != right_value.to_string())
        }
        (Some(Raw(Str(left_value))), "-ge", Some(Raw(Num(right_value)))) => {
            Some(*left_value >= right_value.to_string())
        }
        (Some(Raw(Str(left_value))), "-gt", Some(Raw(Num(right_value)))) => {
            Some(*left_value > right_value.to_string())
        }
        (Some(Raw(Str(left_value))), "-le", Some(Raw(Num(right_value)))) => {
            Some(*left_value <= right_value.to_string())
        }
        (Some(Raw(Str(left_value))), "-lt", Some(Raw(Num(right_value)))) => {
            Some(*left_value < right_value.to_string())
        }

        // number to string comparison
        (Some(Raw(Num(left_value))), "-eq", Some(Raw(Str(right_value)))) => {
            Some(left_value.to_string() == *right_value)
        }
        (Some(Raw(Num(left_value))), "-ne", Some(Raw(Str(right_value)))) => {
            Some(left_value.to_string() != *right_value)
        }
        (Some(Raw(Num(left_value))), "-ge", Some(Raw(Str(right_value)))) => {
            Some(left_value.to_string() >= *right_value)
        }
        (Some(Raw(Num(left_value))), "-gt", Some(Raw(Str(right_value)))) => {
            Some(left_value.to_string() > *right_value)
        }
        (Some(Raw(Num(left_value))), "-le", Some(Raw(Str(right_value)))) => {
            Some(left_value.to_string() <= *right_value)
        }
        (Some(Raw(Num(left_value))), "-lt", Some(Raw(Str(right_value)))) => {
            Some(left_value.to_string() < *right_value)
        }

        _ => None,
    }
}
