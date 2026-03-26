use crate::js::JavaScript;
use crate::js::JavaScript::*;
use crate::js::Value::*;
use std::collections::HashMap;

fn native_function(name: &str) -> JavaScript {
    Function {
        source: format!("function {name}() {{ [native code] }}"),
        return_value: None,
    }
}

fn function_name_from_source(source: &str) -> String {
    let trimmed = source.trim();
    if let Some(rest) = trimmed.strip_prefix("function ")
        && let Some((name, _)) = rest.split_once('(')
    {
        let name = name.trim();
        if !name.is_empty() {
            return name.to_string();
        }
    }

    "anonymous".to_string()
}

fn constructor_name(value: &JavaScript) -> &'static str {
    match value {
        Undefined => "undefined",
        NaN => "Number",
        Raw(v) => match v {
            Num(_) => "Number",
            Str(_) => "String",
            Bool(_) => "Boolean",
            BigInt(_) => "BigInt",
        },
        Array(_) => "Array",
        Regex { .. } => "RegExp",
        Function { .. } => "Function",
        Bytes(_) => "String",
        Null => "null",
        Object { .. } => "Object",
    }
}

fn string_builtins(s: &str) -> HashMap<String, JavaScript> {
    let mut map = HashMap::new();

    map.insert(
        "fontcolor".to_string(),
        Function {
            source: "function fontcolor() {}".to_string(),
            return_value: Some(Box::new(Raw(Str(format!(
                "<font color=\"undefined\">{s}</font>"
            ))))),
        },
    );
    map.insert(
        "italics".to_string(),
        Function {
            source: "function italics() {}".to_string(),
            return_value: Some(Box::new(Raw(Str(format!("<i>{s}</i>"))))),
        },
    );

    map
}

fn array_builtins(_v: Vec<JavaScript>) -> HashMap<String, JavaScript> {
    let mut map = HashMap::new();

    map.insert(
        "entries".to_string(),
        Function {
            source: "[object Array Iterator]".to_string(),
            return_value: Some(Box::new(Raw(Str("[object Array Iterator]".to_string())))),
        },
    );
    map.insert(
        "flat".to_string(),
        Function {
            source: "function flat() { [native code] }".to_string(),
            return_value: None,
        },
    );

    map
}

pub fn as_object(value: &JavaScript) -> Option<JavaScript> {
    if let Object {
        map,
        to_string_override,
    } = value
    {
        let mut obj = map.clone();
        obj.entry("constructor".to_string())
            .or_insert_with(|| native_function("Object"));
        return Some(Object {
            map: obj,
            to_string_override: to_string_override.clone(),
        });
    }

    let mut map = HashMap::new();
    map.insert(
        "constructor".to_string(),
        native_function(constructor_name(value)),
    );

    if matches!(value, Array(_)) {
        map.insert("at".to_string(), native_function("at"));
    }

    if let Function { source, .. } = value {
        map.insert(
            "name".to_string(),
            Raw(Str(function_name_from_source(source))),
        );
    }

    if let Raw(Str(s)) = value {
        map.extend(string_builtins(s));
    }

    if let Array(arr) = value {
        map.extend(array_builtins(arr.clone()));
    }

    Some(Object {
        map,
        to_string_override: None,
    })
}
