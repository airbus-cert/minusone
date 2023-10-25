use tree_sitter;
use tree_sitter_powershell::language as powershell_language;
use ps::integer::{ParseInt, AddInt, MultInt};
use ps::forward::Forward;
use ps::string::{ParseString, ConcatString, StringReplaceMethod, StringReplaceOp, FormatString};
use error::MinusOneResult;
use tree::{Tree, HashMapStorage};
use ps::var::{Var, StaticVar};
use ps::cast::{Cast, CastNull};
use ps::array::{ParseArrayLiteral, ParseRange, ComputeArrayExpr};
use ps::access::AccessString;
use ps::join::{JoinComparison, JoinStringMethod, JoinOperator};
use ps::foreach::{PSItemInferrator, ForEach};
use ps::hash::ParseHash;
use ps::bool::{ParseBool, Comparison, Not};
use ps::method::Length;

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
pub mod method;


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
    fn to_i32(&self) -> Option<i32> {
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
                Some(*i)
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
    Forward,                // Special rule that will forward inferred value in case the node is transparent
    ParseInt,               // Parse integer
    AddInt,                 // +, - operations on integer
    MultInt,                // *, / operations on integer
    ParseString,            // Parse string token, including multiline strings
    ConcatString,           // String concatenation operation
    Var,                    // Variable replacement in case of predictable flow
    Cast,                   // cast operation, like [char]0x65
    ParseArrayLiteral,      // It will parse array declared using separate value (integer or string) by a comma
    ParseRange,             // It will parse .. operator and generate an array
    AccessString,           // The access operator [] apply to a string : "foo"[0] => "f"
    JoinComparison,         // It will infer join string operation using the -join operator : @('a', 'b', 'c') -join '' => "abc"
    JoinStringMethod,       // It will infer join string operation using the [string]::join method : [string]::join('', @('a', 'b', 'c'))
    JoinOperator,           // It will infer join string operation using the -join unary operator -join @('a', 'b', 'c')
    PSItemInferrator,       // PsItem is used to inferred commandlet pattern like % { [char] $_ }
    ForEach,                // It will used PSItem rules to inferred foreach-object command
    StringReplaceMethod,    // It will infer replace method apply to a string : "foo".replace("oo", "aa") => "faa"
    ComputeArrayExpr,       // It will infer array that start with @
    StringReplaceOp,        // It will infer replace method apply to a string by using the -replace operator
    StaticVar,              // It will infer value of known variable : $pshome, $shellid
    CastNull,               // It will infer value of +$() or -$() which will produce 0
    ParseHash,              // Parse hashtable
    FormatString,           // It will infer string when format operator is used ; "{1}-{0}" -f "Debug", "Write"
    ParseBool,              // It will infer boolean operator
    Comparison,             // It will infer comparison when it's possible
    Length,                 // It will infer length value of a predictable array or string
    Not                     // It will infer the ! operator
);

pub fn build_powershell_tree(source: &str) -> MinusOneResult<Tree<HashMapStorage<Powershell>>> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(powershell_language()).unwrap();

    // Powershell is case insensitive
    // And the grammar is specified in lowercase
    let tree_sitter = parser.parse( source.to_lowercase().as_str(), None).unwrap();
    Ok(Tree::<HashMapStorage<Powershell>>::new(source.as_bytes(), tree_sitter))
}