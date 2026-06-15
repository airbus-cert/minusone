use crate::js::JavaScript;
use crate::js::JavaScript::Raw;
use crate::js::Value::Str;
use crate::tree::Node;

/// If `callee` is the JSFuck "level 9" universal builder returning a *bare
/// identifier* and immediately invoked with no arguments — `Function("return
/// <name>")()` or `X["constructor"]("return <name>")()` — returns `<name>`.
///
/// This is the indirection pure JSFuck uses to reach global objects such as
/// `escape`, `unescape` and `RegExp`. Only a `return <identifier>` body is
/// recognized (never a call or other expression), so nothing is executed.
pub fn builder_returned_identifier(callee: &Node<JavaScript>) -> Option<String> {
    // Descend through any surrounding parentheses: `(Function("return X")())(…)`.
    if callee.kind() == "parenthesized_expression" {
        let inner = callee.iter().find(|c| !matches!(c.kind(), "(" | ")"))?;
        return builder_returned_identifier(&inner);
    }
    if callee.kind() != "call_expression" {
        return None;
    }
    // The builder is invoked with no arguments: `(...)()`.
    if !get_positional_arguments(callee.named_child("arguments")).is_empty() {
        return None;
    }

    let inner = callee.named_child("function").or_else(|| callee.child(0))?;
    if inner.kind() != "call_expression" {
        return None;
    }

    // The inner callee must be the Function constructor (`Function` or
    // `X["constructor"]`).
    let inner_callee = inner.named_child("function").or_else(|| inner.child(0))?;
    let is_function_constructor = method_name(&inner_callee).as_deref() == Some("constructor")
        || inner_callee.text().map(|t| t == "Function").unwrap_or(false);
    if !is_function_constructor {
        return None;
    }

    let inner_args = get_positional_arguments(inner.named_child("arguments"));
    if inner_args.len() != 1 {
        return None;
    }
    let Some(Raw(Str(body))) = inner_args[0].data() else {
        return None;
    };

    let name = body.trim().strip_prefix("return")?.trim();
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
    {
        return None;
    }
    Some(name.to_string())
}

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
