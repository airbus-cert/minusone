use crate::ps::Powershell;
use crate::ps::Powershell::{Array, Bytes};
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
        _ => None,
    }
}
