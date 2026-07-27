use crate::ps::Value::{Num, Str};

pub fn to_char(value: &crate::ps::Value) -> Option<char> {
    match value {
        Num(n) if *n >= 0 => char::from_u32(*n as u32),
        Str(s) if s.chars().count() == 1 => s.chars().next(),
        _ => None,
    }
}

pub fn to_chars(values: &[crate::ps::Value]) -> Option<Vec<char>> {
    values.iter().map(to_char).collect()
}
