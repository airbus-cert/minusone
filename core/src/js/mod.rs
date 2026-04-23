pub mod array;
pub mod b64;
pub mod backend;
pub mod bool;
pub mod comparator;
mod converter;
pub mod forward;
pub mod functions;
pub mod globals;
pub mod integer;
pub mod linter;
pub mod objects;
pub mod post_process;
pub mod regex;
pub mod specials;
pub mod strategy;
pub mod string;
mod tests;
pub mod r#typeof;
mod utils;
pub mod var;

use self::array::*;
use self::b64::*;
use self::bool::*;
use self::comparator::*;
use self::forward::*;
use self::functions::fncall::*;
use self::functions::function::*;
use self::integer::*;
use self::linter::RemoveComment;
use self::objects::object::*;
use self::regex::*;
use self::specials::*;
use self::string::*;
use self::r#typeof::*;
use self::var::*;
use crate::error::{Error, MinusOneResult};
use crate::js::JavaScript::*;
use crate::js::Value::*;
#[cfg(test)]
use crate::js::linter::Linter;
use crate::rule::{RuleMut, RuleSet, RuleSetBuilderType};
use crate::tree::{HashMapStorage, Storage, Tree};
use std::collections::HashMap;
use tree_sitter_javascript::LANGUAGE as javascript_language;

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Value {
    Num(f64),
    Str(String),
    Bool(bool),
    BigInt(num_bigint::BigInt),
}

#[derive(Debug, Clone, PartialEq)]
pub enum JavaScript {
    Raw(Value),
    Array(Vec<JavaScript>),
    Regex {
        pattern: String,
        flags: String,
    },
    Function {
        source: String,
        return_value: Option<Box<JavaScript>>,
    },
    Undefined,
    NaN,
    Null,
    // Byte is a special type that does not live long, it's used to store the result of `atob` and
    // `btoa` calls, and it's converted to string when it's used in a string context, but it gets
    // most of the type converted into a string
    Bytes(Vec<u8>),
    Object {
        map: HashMap<String, JavaScript>,
        to_string_override: Option<String>,
    },
}

impl PartialEq<JavaScript> for &JavaScript {
    fn eq(&self, other: &JavaScript) -> bool {
        match (self, other) {
            (Undefined, Undefined) => true,
            (NaN, NaN) => true,
            (Array(arr1), Array(arr2)) => arr1 == arr2,
            (Raw(Num(n1)), Raw(Num(n2))) => n1 == n2,
            (Raw(Str(s1)), Raw(Str(s2))) => s1 == s2,
            (Raw(Bool(b1)), Raw(Bool(b2))) => b1 == b2,
            _ => false,
        }
    }
}

pub struct JavaScriptRuleSet<'a> {
    ruleset: RuleSet<'a, JavaScript>,
}

impl JavaScript {
    pub fn is_string(&self) -> bool {
        matches!(self, Raw(Str(_)))
    }
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
    ParseInt,          // Parse integer literals (decimal, hex, octal, binary)
    ParseBool,         // Parse boolean literals (true, false)
    ParseString,       // Parse string literals (single and double quotes)
    ParseRegex,        // Parse regex literals and RegExp constructors
    ParseFunction,     // Parse function and arrow-function expressions as first-class values
    ParseArray,        // Parse arrays
    ParseSpecials,     // Parse specials (undefined, NaN, null)
    ParseObject,       // Parse objects
    PosNeg,            // Infer unary - operations on integers
    AddInt,            // Infer addition operations on integers
    Substract,         // Infer subtraction operations on any JavaScript value
    MultInt,           // Infer *, / and % operations on integers
    PowInt,            // Infer ** operations on integers
    ShiftInt,          // Infer <<, >> and >>> operations on integers
    BitwiseInt,        // Infer &, |, ^ and ~ operations on integers
    ObjectField,       // Track objects field assignments and access
    NotBool,           // Infer unary ! operations on booleans
    BoolAlgebra,       // Infer boolean algebra operations (&&, ||)
    AddBool,           // Infer boolean addition operations
    CombineArrays,     // Infer + operations on two arrays
    CharAt, // Infer charAt calls on string literals and reduces them to single-character string literals using arrays indexes
    CharCodeAt, // Infer charCodeAt calls on string literals and reduces them to integer literals using arrays indexes
    FromCharCode, // Infer String.fromCharCode static calls on deterministic literal arguments
    StringConstructor, // Infer String(...) coercion calls on deterministic literal arguments
    Forward,    // Forward inferred type in the most simple cases
    ArrayPlusMinus, // Infer unary plus and minus on arrays
    ArrayJoin,  // Infer array join calls on literal arrays and reduce them to string literals
    Concat,     // Infer string concatenation with + operator on string literals
    RegexConcat, // Infer regex concatenation with + operator on string literals
    ConcatFunction, // Infer function source concatenation with `+` and reduce them to single string literals
    Split,          // Infer string split calls on literal strings
    Replace,        // Infer string replace calls on literal strings
    GetArrayElement, // Get element at array index
    AddSubSpecials, // Infer add and sub on Undefined and NaN
    ToString,       // Infer toString calls
    B64,            // Infer atob & btoa calls and reduce them to string literals
    Var,            // Track variable assignments and propagate known values to usage sites
    RegexExec,      // Infer deterministic regex test/exec calls
    FnCall,         // Resolve predictable function calls to their return values
    StrictEq,       // Infer strict equality === and !==
    LooseEq,        // Infer strict equality == and !=
    CmpOrd,         // Infer comparison operators <, >, <= and >=
    Typeof          // Infer typeof calls
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

#[cfg(test)]
pub fn lint(tree: &Tree<HashMapStorage<JavaScript>>) -> String {
    let mut linter = Linter::default();
    tree.apply(&mut linter).unwrap();
    linter.output
}
