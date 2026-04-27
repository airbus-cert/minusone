use crate::js::JavaScript;
use crate::js::JavaScript::Raw;
use crate::js::Value::Str;
use crate::tree::Node;

pub fn method_name(callee: &Node<JavaScript>) -> Option<String> {
    match callee.kind() {
        "subscript_expression" => {
            let index = callee.named_child("index")?;
            match index.data() {
                Some(Raw(Str(s))) => Some(s.clone()),
                _ => index.text().ok().map(|s| s.to_string()),
            }
        }
        "member_expression" => callee
            .named_child("property")
            .and_then(|p| p.text().ok().map(|s| s.to_string())),
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
