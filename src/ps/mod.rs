use ps::integer::ParseInt;
use ps::forward::Forward;

pub mod charconcat;
pub mod integer;
pub mod forward;


#[derive(Debug, Clone)]
pub enum InferredValue {
    Number(i32),
    String(String)
}

pub type InferredValueRules = (ParseInt, Forward);