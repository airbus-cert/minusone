use ps::integer::{ParseInt, AddInt, MultInt};
use ps::forward::Forward;
use ps::string::{ParseString, ConcatString};
use error::MinusOneResult;
use tree::{Tree, HashMapStorage};
use tree_sitter::{Parser};
use tree_sitter_powershell::language as powershell_language;
use ps::var::Var;
use ps::cast::Cast;
use ps::array::{ParseArrayLiteral, ParseRange};
use ps::access::AccessString;
use ps::join::{JoinComparison, JoinStringMethod, JoinOperator};
use ps::foreach::{PSItemInferrator, ForEach};

pub mod string;
pub mod integer;
pub mod forward;
pub mod var;
pub mod litter;
pub mod cast;
pub mod array;
pub mod foreach;
pub mod access;
pub mod join;


#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Value {
    Num(i32),
    Str(String)
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Powershell {
    Raw(Value),
    Array(Vec<Value>),
    PSItem(Vec<Value>)
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
    ForEach
);

pub fn from_powershell_src(source: &str) -> MinusOneResult<Tree<HashMapStorage<Powershell>>> {
    let mut parser = Parser::new();
    parser.set_language(powershell_language()).unwrap();

    // Powershell is case insensitive
    // And the grammar is specified in lowercase
    let tree_sitter = parser.parse( source.to_lowercase().as_str(), None).unwrap();
    Ok(Tree::<HashMapStorage<Powershell>>::new(source.as_bytes(), tree_sitter))
}