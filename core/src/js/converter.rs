use std::fmt::Display;
use log::warn;
use num::Zero;
use crate::js::b64::js_bytes_to_string;
use crate::js::{JavaScript, Value};
use crate::js::JavaScript::{Array, Bytes, Function, NaN, Null, Object, Raw, Regex, Undefined};
use crate::js::string::escape_js_string;
use crate::js::Value::{BigInt, Bool, Num, Str};


impl Display for JavaScript {
    // If a new type is added, try to put the raw value in the console and see the output
    // It's supposed to represent the value in the code source itself
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Raw(v) => write!(f, "{}", v),
            Array(arr) => {
                let arr_str = arr
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                write!(f, "[{}]", arr_str)
            }
            Regex { pattern, flags } => write!(f, "/{}/{}", pattern.replace('/', "\\/"), flags),
            Function { source, .. } => write!(f, "{}", source),
            Undefined => write!(f, "undefined"),
            NaN => write!(f, "NaN"),
            Bytes(b) => write!(f, "{}", js_bytes_to_string(b)),
            Null => write!(f, "null"),
            Object { map, .. } => {
                let obj_str = map
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect::<Vec<String>>()
                    .join(", ");
                write!(f, "{{{}}}", obj_str)
            }
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Num(n) => {
                    match *n {
                        f64::INFINITY => "Infinity".to_string(),
                        f64::NEG_INFINITY => "-Infinity".to_string(),
                        n => n.to_string(),
                    }
                }
                Str(s) => escape_js_string(s),
                Bool(true) => "true".to_string(),
                Bool(false) => "false".to_string(),
                BigInt(n) => n.to_string() + "n",
            }
        )
    }
}

impl JavaScript {
    pub fn as_bool(&self) -> bool {
        // If a new type is added, try `if(...){console.log(true)}else{console.log(false)}` in the console with different values of that type
        match self {
            Raw(raw) => match raw {
                Num(n) => *n != 0.0 && !n.is_nan(),
                Str(s) => !s.is_empty(),
                Bool(b) => *b,
                BigInt(b) => !b.is_zero(),
            },

            Array(_) => true,
            Regex { .. } => true,
            Function { .. } => true,
            Undefined => false,
            NaN => false,
            Null => false,
            Bytes(bytes) => {
                if bytes.is_empty() {
                    return false;
                }

                for byte in bytes {
                    if *byte != 0 {
                        return true;
                    }
                }

                false
            }
            Object { .. } => {
                warn!("Objects don't really have a boolean value in Js, falling back to true");
                true
            }
        }
    }
    
    pub fn r#typeof(&self) -> &str {
        // If a new type is added, try `typeof ...` in the console
        match self {
            Raw(raw) => match raw {
                Num(_) => "number",
                Str(_) => "string",
                Bool(_) => "boolean",
                BigInt(_) => "bigint",
            },
            Array(_) => "object",
            Regex { .. } => "object",
            Function { .. } => "function",
            Undefined => "undefined",
            NaN => "number",
            Null => "object", // what ?
            Bytes(_) => "string",
            Object { .. } => "object",
        }
    }
}