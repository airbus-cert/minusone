use error::MinusOneResult;
use ps::access::{AccessString, AccessArray};
use ps::array::{ComputeArrayExpr, ParseArrayLiteral, ParseRange, AddArray};
use ps::bool::{Comparison, Not, ParseBool, BoolAlgebra};
use ps::cast::{Cast, CastNull};
use ps::foreach::{ForEach, PSItemInferrator};
use ps::forward::Forward;
use ps::hash::ParseHash;
use ps::integer::{AddInt, MultInt, ParseInt};
use ps::join::{JoinComparison, JoinOperator, JoinStringMethod};
use ps::string::{ConcatString, FormatString, ParseString, StringReplaceMethod, StringReplaceOp, StringSplitMethod};
use ps::typing::ParseType;
use ps::var::{StaticVar, Var};
use tree::{HashMapStorage, Tree};
use tree_sitter_powershell::language as powershell_language;
use ps::method::{DecodeBase64, FromUTF, Length};

pub mod access;
pub mod array;
pub mod bool;
pub mod cast;
pub mod foreach;
pub mod forward;
pub mod hash;
pub mod integer;
pub mod join;
pub mod linter;
pub mod method;
pub mod strategy;
pub mod string;
pub mod typing;
pub mod var;
pub mod r#static;

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd)]
pub enum Value {
    Num(i64),
    Str(String),
    Bool(bool),
}

impl ToString for Value {
    fn to_string(&self) -> String {
        match self {
            Value::Num(e) => e.to_string(),
            Value::Str(s) => s.clone(),
            Value::Bool(true) => "True".to_string(),
            Value::Bool(false) => "False".to_string(),
        }
    }
}

impl Value {
    fn to_i64(&self) -> Option<i64> {
        match self {
            Value::Str(s) => {
                if let Ok(number) = s.parse::<i64>() {
                    Some(number)
                } else if s.len() > 2 {
                    u32::from_str_radix(&s[2..], 16).map(|e| e as i64).ok()
                } else {
                    None
                }
            }
            Value::Num(i) => Some(*i),
            Value::Bool(_) => None,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Powershell {
    Raw(Value),
    Array(Vec<Value>),
    PSItem(Vec<Value>),
    Null,
    HashMap,      // We don't infer this time, but it's planed
    Type(String), // Will infer type
}

/// This is the rule set use to perform
/// inferred type in Powershell deobfuscation

pub type RuleSet = (
    Forward,            // Special rule that will forward inferred value in case the node is transparent
    ParseInt,           // Parse integer
    AddInt,             // +, - operations on integer
    MultInt,            // *, / operations on integer
    ParseString,        // Parse string token, including multiline strings
    ConcatString,       // String concatenation operation
    Cast,               // cast operation, like [char]0x65
    ParseArrayLiteral,  // It will parse array declared using separate value (integer or string) by a comma
    ParseRange,         // It will parse .. operator and generate an array
    AccessString,       // The access operator [] apply to a string : "foo"[0] => "f"
    JoinComparison,     // It will infer join string operation using the -join operator : @('a', 'b', 'c') -join '' => "abc"
    JoinStringMethod,   // It will infer join string operation using the [string]::join method : [string]::join('', @('a', 'b', 'c'))
    JoinOperator,       // It will infer join string operation using the -join unary operator -join @('a', 'b', 'c')
    PSItemInferrator,   // PsItem is used to inferred commandlet pattern like % { [char] $_ }
    ForEach,            // It will used PSItem rules to inferred foreach-object command
    StringReplaceMethod,// It will infer replace method apply to a string : "foo".replace("oo", "aa") => "faa"
    ComputeArrayExpr,   // It will infer array that start with @
    StringReplaceOp,    // It will infer replace method apply to a string by using the -replace operator
    StaticVar,          // It will infer value of known variable : $pshome, $shellid
    CastNull,           // It will infer value of +$() or -$() which will produce 0
    ParseHash,          // Parse hashtable
    FormatString,       // It will infer string when format operator is used ; "{1}-{0}" -f "Debug", "Write"
    ParseBool,          // It will infer boolean operator
    Comparison,         // It will infer comparison when it's possible
    Not,                // It will infer the ! operator
    ParseType,          // Parse type
    DecodeBase64,       // Decode calls to FromBase64
    FromUTF,            // Decode calls to FromUTF{8,16}.GetText
    Length,             // Decode attribute length of string and array
    BoolAlgebra,        // Add support to boolean algebra (or and)
    Var,                // Variable replacement in case of predictable flow
    AddArray,           // Array concat using +, operator
    StringSplitMethod,  // Handle split method
    AccessArray,        // Handle static array element access
);

pub fn build_powershell_tree<T>(source: &str) -> MinusOneResult<Tree<HashMapStorage<T>>> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&powershell_language()).unwrap();

    // Powershell is case insensitive
    // And the grammar is specified in lowercase
    let tree_sitter = parser.parse(source.to_lowercase().as_str(), None).unwrap();
    Ok(Tree::<HashMapStorage<T>>::new(
        source.as_bytes(),
        tree_sitter,
    ))
}
