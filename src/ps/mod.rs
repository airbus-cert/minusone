use std::collections::BTreeMap;
use tree_sitter_powershell::LANGUAGE as powershell_language;

use crate::error::{Error, MinusOneResult};
use crate::tree::{HashMapStorage, Storage, Tree};

use crate::ps::{
    access::{AccessArray, AccessHashMap, AccessString},
    array::{AddArray, ComputeArrayExpr, ParseArrayLiteral, ParseRange},
    bool::{BoolAlgebra, Comparison, Not, ParseBool},
    cast::{Cast, CastNull},
    foreach::{ForEach, PSItemInferrator},
    forward::Forward,
    hash::ParseHash,
    integer::{AddInt, MultInt, ParseInt},
    join::{JoinComparison, JoinOperator, JoinStringMethod},
    linter::RemoveComment,
    method::{DecodeBase64, FromUTF, Length},
    string::{
        ConcatString, FormatString, ParseString, StringReplaceMethod, StringReplaceOp,
        StringSplitMethod,
    },
    typing::ParseType,
    var::{StaticVar, Var},
};

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
mod tool;
pub mod typing;
pub mod var;

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub enum Value {
    Num(i64),
    Str(String),
    Bool(bool),
}

impl Value {
    fn normalize(&self) -> Value {
        match self {
            Value::Str(x) => Value::Str(x.to_lowercase()),
            x => x.clone(),
        }
    }
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

#[derive(Debug, Clone, PartialEq)]
pub enum Powershell {
    Raw(Value),
    Array(Vec<Value>),
    PSItem(Vec<Value>),
    Null,
    HashMap(BTreeMap<Value, Value>),
    HashEntry(Value, Value),
    Type(String), // Will infer type
    Unknown,
}

/// This is the rule set use to perform
/// inferred type in Powershell deobfuscation

pub type RuleSet = (
    Forward,      // Special rule that will forward inferred value in case the node is transparent
    ParseInt,     // Parse integer
    AddInt,       // +, - operations on integer
    MultInt,      // *, / operations on integer
    ParseString,  // Parse string token, including multiline strings
    ConcatString, // String concatenation operation
    Cast,         // cast operation, like [char]0x65
    ParseArrayLiteral, // It will parse array declared using separate value (integer or string) by a comma
    ParseRange,        // It will parse .. operator and generate an array
    AccessString,      // The access operator [] apply to a string : "foo"[0] => "f"
    JoinComparison, // It will infer join string operation using the -join operator : @('a', 'b', 'c') -join '' => "abc"
    JoinStringMethod, // It will infer join string operation using the [string]::join method : [string]::join('', @('a', 'b', 'c'))
    JoinOperator, // It will infer join string operation using the -join unary operator -join @('a', 'b', 'c')
    PSItemInferrator, // PsItem is used to inferred commandlet pattern like % { [char] $_ }
    ForEach,      // It will used PSItem rules to inferred foreach-object command
    StringReplaceMethod, // It will infer replace method apply to a string : "foo".replace("oo", "aa") => "faa"
    ComputeArrayExpr,    // It will infer array that start with @
    StringReplaceOp, // It will infer replace method apply to a string by using the -replace operator
    StaticVar,       // It will infer value of known variable : $pshome, $shellid
    CastNull,        // It will infer value of +$() or -$() which will produce 0
    ParseHash,       // Parse hashtable
    FormatString, // It will infer string when format operator is used ; "{1}-{0}" -f "Debug", "Write"
    ParseBool,    // It will infer boolean operator
    Comparison,   // It will infer comparison when it's possible
    Not,          // It will infer the ! operator
    ParseType,    // Parse type
    DecodeBase64, // Decode calls to FromBase64
    FromUTF,      // Decode calls to FromUTF{8,16}.GetText
    Length,       // Decode attribute length of string and array
    BoolAlgebra,  // Add support to boolean algebra (or and)
    Var,          // Variable replacement in case of predictable flow
    AddArray,     // Array concat using +, operator
    StringSplitMethod, // Handle split method
    AccessArray,  // Handle static array element access
    AccessHashMap, // Handle hashmap access
);

pub fn remove_powershell_extra(source: &str) -> MinusOneResult<String> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&powershell_language.into())
        .expect("Error loading powershell grammar");

    // Trim to assert program is at the beginning
    let source = source.trim();

    // And the grammar is specified in lowercase
    let tree_sitter_remove_extra = parser.parse(source, None).unwrap();
    let root = Tree::<HashMapStorage<Powershell>>::new(source.as_bytes(), tree_sitter_remove_extra);

    let root_node = root.root().or(Err(Error::invalid_program()))?;
    if root_node.kind() != "program" {
        return Err(Error::invalid_program());
    }
    if root_node.start_abs() != 0 {
        return Err(Error::invalid_program_index(root_node.start_abs()));
    }

    let mut source_without_extra = RemoveComment::new();
    root.apply(&mut source_without_extra)?;
    source_without_extra.clear()
}

pub fn build_powershell_tree(source: &str) -> MinusOneResult<Tree<'_, HashMapStorage<Powershell>>> {
    build_powershell_tree_for_storage::<HashMapStorage<Powershell>>(source)
}

pub fn build_powershell_tree_for_storage<T: Storage + Default>(
    source: &str,
) -> MinusOneResult<Tree<T>> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&powershell_language.into())
        .expect("Error loading powershell grammar");

    // And the grammar is specified in lowercase
    let tree_sitter = parser.parse(source, None).unwrap();
    Ok(Tree::<T>::new(source.as_bytes(), tree_sitter))
}
