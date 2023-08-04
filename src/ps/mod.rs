use ps::integer::{ParseInt, AddInt};
use ps::forward::Forward;
use ps::string::ParseString;

pub mod string;
pub mod integer;
pub mod forward;


#[derive(Debug, Clone)]
pub enum InferredValue {
    Number(i32),
    String(String)
}

pub type InferredValueRules = (Forward, ParseInt, AddInt, ParseString);