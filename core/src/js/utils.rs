use crate::js::JavaScript;
use crate::js::JavaScript::{Function, NaN, Raw};
use crate::js::Value::{Num, Str};
use crate::tree::Node;

pub fn method_name(callee: &Node<JavaScript>) -> Option<String> {
    match callee.kind() {
        "member_expression" => callee
            .named_child("property")
            .and_then(|p| p.text().ok().map(|s| s.to_string())),
        // compatibility fallback for dynamic bracket calls that were already inferred to strings
        "subscript_expression" => {
            let index = callee.named_child("index")?;
            match index.data() {
                Some(Raw(Str(s))) => Some(s.clone()),
                _ => None,
            }
        }
        _ => None,
    }
}

pub fn get_positional_arguments(args: Option<Node<JavaScript>>) -> Vec<Node<JavaScript>> {
    let mut positional_args = vec![];
    if let Some(arguments) = args {
        for child in arguments.iter() {
            if !matches!(child.kind(), "(" | ")" | ",") {
                positional_args.push(child);
            }
        }
    }
    positional_args
}

pub fn to_js_uint32(x: f64) -> u32 {
    if x.is_nan() || x.is_infinite() || x == 0.0 {
        return 0;
    }
    let n = x.trunc() % 4_294_967_296.0; // 2^32
    if n < 0.0 {
        (n + 4_294_967_296.0) as u32
    } else {
        n as u32
    }
}

pub fn is_write_target(node: &Node<JavaScript>) -> bool {
    let mut child_start = node.start_abs();
    let mut child_end = node.end_abs();
    let mut current = node.parent();

    while let Some(parent) = current {
        match parent.kind() {
            "variable_declarator" => {
                if let Some(name_child) = parent.child(0) {
                    return child_start >= name_child.start_abs()
                        && child_end <= name_child.end_abs();
                }
                return false;
            }
            "assignment_expression" | "augmented_assignment_expression" => {
                if let Some(left) = parent.child(0) {
                    return child_start >= left.start_abs() && child_end <= left.end_abs();
                }
                return false;
            }
            "subscript_expression" => {
                let in_index = parent
                    .named_child("index")
                    .or_else(|| parent.child(2))
                    .map(|index| child_start >= index.start_abs() && child_end <= index.end_abs())
                    .unwrap_or(false);
                if in_index {
                    return false;
                }
                child_start = parent.start_abs();
                child_end = parent.end_abs();
                current = parent.parent();
                continue;
            }
            "member_expression" => {
                let in_property = parent
                    .named_child("property")
                    .map(|prop| child_start >= prop.start_abs() && child_end <= prop.end_abs())
                    .unwrap_or(false);
                if in_property {
                    return false;
                }
                child_start = parent.start_abs();
                child_end = parent.end_abs();
                current = parent.parent();
                continue;
            }
            "update_expression" => return true,
            _ => {}
        }

        current = parent.parent();
    }

    false
}

pub fn js_index_from_optional_arg(value: Option<&JavaScript>) -> i64 {
    match value {
        None => 0,
        Some(v) => match v.as_js_num() {
            Raw(Num(n)) if n.is_finite() => n.trunc() as i64,
            Raw(Num(_)) | NaN => 0,
            _ => 0,
        },
    }
}

pub fn as_known_string(value: &JavaScript) -> String {
    match value {
        Raw(Str(s)) => s.clone(),
        any => any.to_string(),
    }
}

pub fn native_function(name: &str) -> JavaScript {
    Function {
        source: format!("function {name}() {{ [native code] }}"),
        return_value: None,
    }
}
