use crate::ps::Powershell;
use crate::ps::Powershell::{Array, Bytes, Stream};
use crate::ps::Value::Num;

pub fn bytes_from_array(values: &[crate::ps::Value]) -> Option<Vec<u8>> {
    values
        .iter()
        .map(|v| match v {
            Num(n) if (0..=255).contains(n) => Some(*n as u8),
            _ => None,
        })
        .collect()
}

pub fn bytes_from_data(data: &Powershell) -> Option<Vec<u8>> {
    match data {
        Array(a) => bytes_from_array(a),
        Bytes(b) => Some(b.clone()),
        Stream(b) => Some(b.clone()),
        _ => None,
    }
}

pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn from_hex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}
