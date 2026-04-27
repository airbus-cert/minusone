use crate::js::JavaScript::*;
use crate::js::Value::*;
use crate::js::array::flatten_array;
use crate::js::b64::js_bytes_to_string;
use crate::js::string::escape_js_string;
use crate::js::{JavaScript, Value};
use num::{ToPrimitive, Zero};
use std::fmt::Display;

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
            Buffer(b) => {
                let hex = b
                    .iter()
                    .map(|byte| format!("{:02x}", byte))
                    .collect::<String>();
                write!(f, "Buffer.from('{}', 'hex')", hex)
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
                        n => format_f64_to_string(n),
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
    pub fn as_js_num(&self) -> JavaScript {
        // If a new type is added, try `+...` in the console with different values of that type
        match self {
            Raw(raw) => match raw {
                Num(n) => Raw(Num(*n)),
                Str(s) => {
                    let trimmed = s.trim();
                    if trimmed.is_empty() {
                        Raw(Num(0.0))
                    } else if let Ok(n) = trimmed.parse::<f64>() {
                        Raw(Num(n))
                    } else if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
                        match u64::from_str_radix(&trimmed[2..], 16) {
                            Ok(n) => Raw(Num(n as f64)),
                            Err(_) => NaN,
                        }
                    } else if trimmed.starts_with("0b") || trimmed.starts_with("0B") {
                        match u64::from_str_radix(&trimmed[2..], 2) {
                            Ok(n) => Raw(Num(n as f64)),
                            Err(_) => NaN,
                        }
                    } else if trimmed.starts_with("0o") || trimmed.starts_with("0O") {
                        match u64::from_str_radix(&trimmed[2..], 8) {
                            Ok(n) => Raw(Num(n as f64)),
                            Err(_) => NaN,
                        }
                    } else {
                        NaN
                    }
                }
                Bool(b) => Raw(Num(if *b { 1.0 } else { 0.0 })),
                BigInt(b) => {
                    if b.is_zero() {
                        Raw(Num(0.0))
                    } else if let Some(n) = b.to_f64() {
                        Raw(Num(n))
                    } else {
                        // BigInt too big to fit in a f64 becomes Infinity
                        Raw(Num(f64::INFINITY))
                    }
                }
            },
            Array(array) => {
                if array.is_empty() {
                    Raw(Num(0.0))
                } else {
                    match flatten_array(array, None).parse::<f64>() {
                        Ok(n) => Raw(Num(n)),
                        Err(_) => NaN,
                    }
                }
            }
            Regex { .. } => NaN,
            Function { .. } => NaN,
            Undefined => NaN,
            NaN => NaN,
            Null => Raw(Num(0.0)),
            Bytes(bytes) => Raw(Str(js_bytes_to_string(bytes))).as_js_num(),
            Object { .. } => NaN,
            Buffer(b) => match String::from_utf8(b.clone()) {
                Ok(s) => Raw(Str(s)).as_js_num(),
                Err(_) => NaN,
            },
        }
    }

    pub fn as_bool(&self) -> bool {
        // If a new type is added, try `!!...` in the console with different values of that type
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

                true
            }
            Object { .. } => true,
            Buffer(_) => true,
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
            Buffer(_) => "object",
        }
    }
}

fn format_f64_to_string(n: f64) -> String {
    if n.is_nan() {
        return "NaN".to_string();
    }
    if n.is_infinite() {
        return if n.is_sign_positive() {
            "Infinity".to_string()
        } else {
            "-Infinity".to_string()
        };
    }

    let abs = n.abs();
    if abs == 0.0 {
        return "0".to_string();
    }

    if abs >= 1e21 || abs < 1e-6 {
        to_js_scientific(n)
    } else {
        format!("{}", n)
    }
}

fn to_js_scientific(n: f64) -> String {
    let abs = n.abs();
    let sign = if n.is_sign_positive() { "" } else { "-" };
    let exp = abs.log10().floor() as i32;
    let s = format!("{}", abs);

    let raw: String = if let Some(e_pos) = s.find('e') {
        s[..e_pos].replace('.', "")
    } else if let Some(dot_pos) = s.find('.') {
        let joined = format!("{}{}", &s[..dot_pos], &s[dot_pos + 1..]);
        joined.trim_start_matches('0').to_string()
    } else {
        s.trim_end_matches('0').to_string()
    };

    let raw = raw.trim_end_matches('0');
    let raw = if raw.is_empty() { "0" } else { raw };

    let first = &raw[..1];
    let rest = &raw[1..];
    let mantissa = if rest.is_empty() {
        first.to_string()
    } else {
        format!("{}.{}", first, rest)
    };

    if exp >= 0 {
        format!("{}{}e+{}", sign, mantissa, exp)
    } else {
        format!("{}{}e{}", sign, mantissa, exp)
    }
}

#[cfg(test)]
mod tests_converter {
    use crate::js::converter::format_f64_to_string;

    #[test]
    fn test_format_f64_to_string() {
        // Big numbers
        assert_eq!(
            "100000000000000000000",
            format_f64_to_string(100000000000000000000.0)
        );
        assert_eq!("1e+21", format_f64_to_string(1000000000000000000000.0));
        assert_eq!(
            "1e+21",
            format_f64_to_string(1000000000000000000000.99999999)
        );
        assert_eq!(
            "1.234567898123456e+32",
            format_f64_to_string(123456789812345600900000000000001.99999999)
        );

        // Small numbers
        assert_eq!("0", format_f64_to_string(0.0));
        assert_eq!("0.000001", format_f64_to_string(0.000001));
        assert_eq!("1e-7", format_f64_to_string(0.0000001));
        assert_eq!("9.9999999e-7", format_f64_to_string(0.00000099999999));
        assert_eq!(
            "1.234567898123456e-32",
            format_f64_to_string(0.00000000000000000000000000000001234567898123456)
        );
    }
}
