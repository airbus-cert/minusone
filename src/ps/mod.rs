use ps::integer::{ParseInt, AddInt, MultInt};
use ps::forward::Forward;
use ps::string::{ParseString, ConcatString, StringReplaceMethod, StringReplaceOp, FormatString};
use error::MinusOneResult;
use tree::{Tree, HashMapStorage};
use tree_sitter::{Parser};
use tree_sitter_powershell::language as powershell_language;
use ps::var::{Var, StaticVar};
use ps::cast::{Cast, CastNull};
use ps::array::{ParseArrayLiteral, ParseRange, ComputeArrayExpr, ArrayLength};
use ps::access::AccessString;
use ps::join::{JoinComparison, JoinStringMethod, JoinOperator};
use ps::foreach::{PSItemInferrator, ForEach};
use ps::hash::ParseHash;
use ps::bool::{ParseBool, Comparison, Not};

pub mod string;
pub mod integer;
pub mod forward;
pub mod var;
pub mod linter;
pub mod cast;
pub mod array;
pub mod foreach;
pub mod access;
pub mod join;
pub mod hash;
pub mod bool;
pub mod strategy;


#[derive(Debug, Clone, Eq, PartialEq, PartialOrd)]
pub enum Value {
    Num(i32),
    Str(String),
    Bool(bool)
}

impl ToString for Value {
    fn to_string(&self) -> String {
        match self {
            Value::Num(e) => e.to_string(),
            Value::Str(s) => s.clone(),
            Value::Bool(true) => "True".to_string(),
            Value::Bool(false) => "False".to_string()
        }
    }
}

impl Value {
    fn to_i32(self) -> Option<i32> {
        match self {
            Value::Str(s) => {
                if let Ok(number) = s.parse::<i32>() {
                    Some(number)
                }
                else if s.len() > 2 {
                    u32::from_str_radix(&s[2..], 16).map(|e| e as i32).ok()
                }
                else {
                    None
                }
            },
            Value::Num(i) => {
                Some(i)
            },
            Value::Bool(_) => None
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Powershell {
    Raw(Value),
    Array(Vec<Value>),
    PSItem(Vec<Value>),
    Null,
    HashMap // We don't infer this time, but it's planed
}

/// This is the rule set use to perform
/// inferred type in Powershell deobfuscation

pub type RuleSet = (
    Forward,
    ParseInt,
    AddInt,
    MultInt,
    ParseString,
    ConcatString,
    Var,
    Cast,
    ParseArrayLiteral,
    ParseRange,
    AccessString,
    JoinComparison,
    JoinStringMethod,
    JoinOperator,
    PSItemInferrator,
    ForEach,
    StringReplaceMethod,
    ComputeArrayExpr,
    StringReplaceOp,
    StaticVar,
    CastNull,
    ParseHash,
    FormatString,
    ParseBool,
    Comparison,
    ArrayLength,
    Not
);

pub fn from_powershell_src(source: &str) -> MinusOneResult<Tree<HashMapStorage<Powershell>>> {
    let mut parser = Parser::new();
    parser.set_language(powershell_language()).unwrap();

    // Powershell is case insensitive
    // And the grammar is specified in lowercase
    let tree_sitter = parser.parse( source.to_lowercase().as_str(), None).unwrap();
    Ok(Tree::<HashMapStorage<Powershell>>::new(source.as_bytes(), tree_sitter))
}