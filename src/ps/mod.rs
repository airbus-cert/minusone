use ps::integer::{ParseInt, AddInt};
use ps::forward::Forward;
use ps::string::{ParseString, ConcatString};
use error::MinusOneResult;
use tree::{Tree, HashMapStorage};
use tree_sitter::{Parser};
use tree_sitter_powershell::language as powershell_language;
use ps::var::Var;

pub mod string;
pub mod integer;
pub mod forward;
pub mod var;
pub mod litter;


#[derive(Debug, Clone, Eq, PartialEq)]
pub enum InferredValue {
    Number(i32),
    Str(String)
}

pub type InferredValueRules = (Forward, ParseInt, AddInt, ParseString, ConcatString, Var);

pub fn from_powershell_src(source: &str) -> MinusOneResult<Tree<HashMapStorage<InferredValue>>> {
    let mut parser = Parser::new();
    parser.set_language(powershell_language()).unwrap();

    let tree_sitter = parser.parse( source, None).unwrap();
    Ok(Tree::<HashMapStorage<InferredValue>>::new(source.as_bytes(), tree_sitter))
}