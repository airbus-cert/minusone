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
    // If a new type is added, try `(...)["constructor"]["name"]` in the console
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
        Buffer(_) => "Buffer",
    }
}

fn string_builtins(s: &str) -> HashMap<String, JavaScript> {
    let mut map = HashMap::new();

    map.insert("length".to_string(), Raw(Num(s.chars().count() as f64)));

    let tags = vec![
        ("big", "big"),
        ("blink", "blink"),
        ("bold", "b"),
        ("fixed", "tt"),
        ("italics", "i"),
        ("small", "small"),
        ("strike", "strike"),
        ("sub", "sub"),
        ("sup", "sup"),
    ];

    for (tag, html_tag) in tags {
        map.insert(
            tag.to_string(),
            Function {
                source: format!("function {tag}() {{ [native code] }}"),
                return_value: Some(Box::new(Raw(Str(format!("<{html_tag}>{s}</{html_tag}>"))))),
            },
        );
    }

    map.insert(
        "fontcolor".to_string(),
        Function {
            source: "function fontcolor() { [native code] }".to_string(),
            return_value: Some(Box::new(Raw(Str(format!(
                "<font color=\"undefined\">{s}</font>"
            ))))),
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
    if !matches!(value, NaN | Undefined | Buffer(_)) {
        map.insert(
            "toString".to_string(),
            Function {
                source: "function toString() {}".to_string(),
                return_value: Some(Box::new(Raw(Str(value.to_string())))),
            },
        );
    }

    if let Array(array) = value {
        map.insert("at".to_string(), native_function("at"));
        let reversed_array = array.clone().iter().rev().cloned().collect();
        map.insert(
            "reverse".to_string(),
            Function {
                source: "function reverse() {}".to_string(),
                return_value: Some(Box::new(Array(reversed_array))),
            },
        );
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

#[cfg(test)]
mod tests_builtins {
    use crate::js::build_javascript_tree;
    use crate::js::functions::fncall::FnCall;
    use crate::js::linter::Linter;
    use crate::js::objects::object::*;
    use crate::js::string::*;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            ParseObject::default(),
            ObjectField::default(),
            FnCall::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_string_builtins() {
        // length
        assert_eq!(deobfuscate("'minusone'.length"), "8");

        // tags
        assert_eq!(deobfuscate("'minusone'.big()"), "'<big>minusone</big>'");
        assert_eq!(
            deobfuscate("'minusone'.blink()"),
            "'<blink>minusone</blink>'"
        );
        assert_eq!(deobfuscate("'minusone'.bold()"), "'<b>minusone</b>'");
        assert_eq!(deobfuscate("'minusone'.fixed()"), "'<tt>minusone</tt>'");
        assert_eq!(deobfuscate("'minusone'.italics()"), "'<i>minusone</i>'");
        assert_eq!(
            deobfuscate("'minusone'.small()"),
            "'<small>minusone</small>'"
        );
        assert_eq!(
            deobfuscate("'minusone'.strike()"),
            "'<strike>minusone</strike>'"
        );
        assert_eq!(deobfuscate("'minusone'.sub()"), "'<sub>minusone</sub>'");
        assert_eq!(deobfuscate("'minusone'.sup()"), "'<sup>minusone</sup>'");
        assert_eq!(
            deobfuscate("'minusone'.fontcolor()"),
            "'<font color=\"undefined\">minusone</font>'"
        );
    }
}
