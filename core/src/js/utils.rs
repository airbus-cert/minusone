use crate::js::JavaScript;
use crate::js::JavaScript::Raw;
use crate::js::Value::Str;
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
    let mut current = node.parent();
    while let Some(parent) = current {
        match parent.kind() {
            "variable_declarator" => {
                if let Some(name_child) = parent.child(0) {
                    return node.start_abs() >= name_child.start_abs()
                        && node.end_abs() <= name_child.end_abs();
                }
            }
            "assignment_expression" | "augmented_assignment_expression" => {
                if let Some(left) = parent.child(0) {
                    return node.start_abs() >= left.start_abs()
                        && node.end_abs() <= left.end_abs();
                }
            }
            "update_expression" => return true,
            _ => {}
        }

        current = parent.parent();
    }

    false
}
