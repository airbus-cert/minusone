pub mod array;
pub mod backend;
pub mod bool;
pub mod forward;
pub mod integer;
pub mod linter;
pub mod strategy;
pub mod string;
mod specials;

use self::array::*;
use self::bool::*;
use self::forward::*;
use self::integer::*;
use self::linter::RemoveComment;
use self::string::*;
use error::{Error, MinusOneResult};
use rule::{RuleMut, RuleSet, RuleSetBuilderType};
use std::fmt::Display;
use tree::{HashMapStorage, Storage, Tree};
use tree_sitter_javascript::LANGUAGE as javascript_language;

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub enum Value {
    Num(i64),
    Str(String),
    Bool(bool),
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Value::Num(e) => e.to_string(),
                Value::Str(s) => escape_js_string(s),
                Value::Bool(true) => "true".to_string(),
                Value::Bool(false) => "false".to_string(),
            }
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum JavaScript {
    Raw(Value),
    Array(Vec<JavaScript>),
    Undefined,
    NaN,
}

impl Display for JavaScript {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JavaScript::Raw(v) => write!(f, "{}", v),
            JavaScript::Array(arr) => {
                let arr_str = arr
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                write!(f, "[{}]", arr_str)
            },
            JavaScript::Undefined => write!(f, "undefined"),
            JavaScript::NaN => write!(f, "NaN"),
        }
    }
}

pub struct JavaScriptRuleSet<'a> {
    ruleset: RuleSet<'a, JavaScript>,
}

macro_rules! impl_javascript_ruleset {
    ( $($ty:ident),* ) => {
        /// This is the rule set use to perform
        /// inferred type in JavaScript deobfuscation
        pub type JavaScriptDefaultRuleSet = ( $($ty,)* );

        impl<'a> JavaScriptRuleSet<'a> {
            pub fn new(ctx: RuleSetBuilderType) -> Self {
                Self {
                    ruleset: RuleSet::new(
                        vec![
                            $( (stringify!($ty), Box::new($ty::default())), )*
                        ],
                        ctx
                    )
                }
            }

            pub fn names(self) -> Vec<&'a str> {
                vec![ $( stringify!($ty), )* ]
            }

        }
    };
}

impl_javascript_ruleset!(
    ParseInt,        // Parse integer literals (decimal, hex, octal, binary)
    ParseBool,       // Parse boolean literals (true, false)
    ParseString,     // Parse string literals (single and double quotes)
    ParseArray,      // Parse arrays
    NegInt,          // Infer unary - operations on integers
    AddInt,          // Infer + and - operations on integers
    MultInt,         // Infer *, / and % operations on integers
    PowInt,          // Infer ** operations on integers
    ShiftInt,        // Infer <<, >> and >>> operations on integers
    NotBool,         // Infer unary ! operations on booleans
    BoolAlgebra,     // Infer boolean algebra operations (&&, ||)
    AddBool,         // Infer + and - operations on booleans
    CombineArrays,   // Infer + operations on two arrays
    CharAt,          // Infer charAt calls on string literals and reduces them to single-character string literals using arrays indexes
    Forward,         // Forward inferred type in the most simple cases
    StringPlusMinus, // Infer + and - unary operations on string literals
    ArrayPlusMinus,  // Infer unary plus and minus on arrays
    BoolPlusMinus,   // Infer + and - operations on booleans
    Concat,          // Infer string concatenation with + operator on string literals
    GetArrayElement  // Get element at array index
);

impl<'a> RuleMut<'a> for JavaScriptRuleSet<'a> {
    type Language = JavaScript;

    fn enter(
        &mut self,
        node: &mut crate::tree::NodeMut<'a, Self::Language>,
        flow: crate::tree::ControlFlow,
    ) -> MinusOneResult<()> {
        self.ruleset.enter(node, flow)
    }

    fn leave(
        &mut self,
        node: &mut crate::tree::NodeMut<'a, Self::Language>,
        flow: crate::tree::ControlFlow,
    ) -> MinusOneResult<()> {
        self.ruleset.leave(node, flow)
    }
}

pub fn remove_javascript_extra(source: &str) -> MinusOneResult<String> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&javascript_language.into())
        .expect("Error loading javascript grammar");

    // Trim to assert program is at the beginning
    let source = source.trim();

    // And the grammar is specified in lowercase
    let tree_sitter_remove_extra = parser.parse(source, None).unwrap();
    let root = Tree::<HashMapStorage<JavaScript>>::new(source.as_bytes(), tree_sitter_remove_extra);

    let root_node = root.root().or(Err(Error::invalid_program()))?;
    if root_node.kind() != "program" {
        return Err(Error::invalid_program());
    }
    if root_node.start_abs() != 0 {
        return Err(Error::invalid_program_index(root_node.start_abs()));
    }

    let mut source_without_extra = RemoveComment::default();
    root.apply(&mut source_without_extra)?;
    source_without_extra.clear()
}

pub fn build_javascript_tree(source: &str) -> MinusOneResult<Tree<'_, HashMapStorage<JavaScript>>> {
    build_javascript_tree_for_storage::<HashMapStorage<JavaScript>>(source)
}

pub fn build_javascript_tree_for_storage<T: Storage + Default>(
    source: &str,
) -> MinusOneResult<Tree<'_, T>> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&javascript_language.into())
        .expect("Error loading javascript grammar");

    let tree_sitter = parser.parse(source, None).unwrap();
    Ok(Tree::<T>::new(source.as_bytes(), tree_sitter))
}
