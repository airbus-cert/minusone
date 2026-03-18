pub mod array;
pub mod b64;
pub mod backend;
pub mod bool;
pub mod comparator;
pub mod deadcode;
pub mod fncall;
pub mod forward;
pub mod integer;
pub mod linter;
pub mod specials;
pub mod strategy;
pub mod string;
pub mod var;

use self::array::*;
use self::b64::*;
use self::bool::*;
use self::comparator::*;
use self::fncall::*;
use self::forward::*;
use self::integer::*;
use self::linter::RemoveComment;
use self::specials::*;
use self::string::*;
use self::var::*;
use crate::error::{Error, MinusOneResult};
use crate::js::JavaScript::*;
use crate::js::Value::*;
#[cfg(test)]
use crate::js::linter::Linter;
use crate::rule::{RuleMut, RuleSet, RuleSetBuilderType};
use crate::tree::{HashMapStorage, Storage, Tree};
use log::error;
use num::Zero;
use std::fmt::Display;
use tree_sitter_javascript::LANGUAGE as javascript_language;

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Value {
    Num(f64),
    Str(String),
    Bool(bool),
    BigInt(num_bigint::BigInt),
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Num(n) => {
                    match *n {
                        f64::INFINITY => "Infinity".to_string(),
                        f64::NEG_INFINITY => "-Infinity".to_string(),
                        n => n.to_string(),
                    }
                }
                Str(s) => escape_js_string(s),
                Bool(true) => "true".to_string(),
                Bool(false) => "false".to_string(),
                BigInt(n) => n.to_string() + "n",
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
    Null,
    At, // This is a special value that represents ƒ -> at() { [native code] }
    Constructor(Box<JavaScript>), // This is a special value that represents ƒ -> JavaScript() { [native code] }
    Bytes(Vec<u8>),
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

impl Display for JavaScript {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Raw(v) => write!(f, "{}", v),
            Array(arr) => {
                let arr_str = arr
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                write!(f, "[{}]", arr_str)
            }
            Undefined => write!(f, "undefined"),
            NaN => write!(f, "NaN"),
            At => write!(f, "[]['at']"),
            Constructor(inner) => {
                let value = match **inner {
                    Undefined => "undefined".to_string(),
                    NaN => "Number".to_string(),
                    At => "[]['at']".to_string(),
                    Raw(ref v) => match v {
                        Num(_) => "0".to_string(),
                        Str(_) => "''".to_string(),
                        Bool(_) => "true".to_string(),
                        BigInt(_) => "0n".to_string(),
                    },
                    Array(_) => "[]".to_string(),
                    Constructor(_) => "['constructor']".to_string(),
                    Bytes(_) => "''".to_string(),
                    Null => {
                        error!(
                            "Null constructor should crash the JS runtime, but we will return 'null' here for safety."
                        );
                        "null".to_string()
                    }
                };

                write!(f, "{}['constructor']", value)
            }
            Bytes(b) => write!(f, "{}", js_bytes_to_string(b)),
            Null => write!(f, "null"),
        }
    }
}

pub struct JavaScriptRuleSet<'a> {
    ruleset: RuleSet<'a, JavaScript>,
}

impl JavaScript {
    pub fn as_bool(&self) -> bool {
        match self {
            Raw(raw) => match raw {
                Num(n) => *n != 0.0 && !n.is_nan(),
                Str(s) => !s.is_empty(),
                Bool(b) => *b,
                BigInt(b) => !b.is_zero(),
            },

            Array(_) => true,
            Undefined => false,
            NaN => false,
            Null => false,
            At => true,
            Constructor(_) => true,
            Bytes(bytes) => {
                if bytes.is_empty() {
                    return false;
                }

                for byte in bytes {
                    if *byte != 0 {
                        return true;
                    }
                }

                false
            }
        }
    }

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
    ParseInt,               // Parse integer literals (decimal, hex, octal, binary)
    ParseBool,              // Parse boolean literals (true, false)
    ParseString,            // Parse string literals (single and double quotes)
    ParseArray,             // Parse arrays
    ParseSpecials,          // Parse specials (undefined, NaN, At, ...)
    NegInt,                 // Infer unary - operations on integers
    SubAddInt,              // Infer + and - operations on integers
    MultInt,                // Infer *, / and % operations on integers
    PowInt,                 // Infer ** operations on integers
    ShiftInt,               // Infer <<, >> and >>> operations on integers
    BitwiseInt,             // Infer &, |, ^ and ~ operations on integers
    NotBool,                // Infer unary ! operations on booleans
    BoolAlgebra,            // Infer boolean algebra operations (&&, ||)
    AddBool,                // Infer + and - operations on booleans
    CombineArrays,          // Infer + operations on two arrays
    CharAt, // Infer charAt calls on string literals and reduces them to single-character string literals using arrays indexes
    Forward, // Forward inferred type in the most simple cases
    StringPlusMinus, // Infer + and - unary operations on string literals
    ArrayPlusMinus, // Infer unary plus and minus on arrays
    BoolPlusMinus, // Infer + and - operations on booleans
    Concat, // Infer string concatenation with + operator on string literals
    GetArrayElement, // Get element at array index
    AddSubSpecials, // Infer add and sub on Undefined and NaN
    AtTrick, // Infer the at trick (e.g. []['at'] -> ƒ -> at() { [native code] }
    ConstructorAccessTrick, // Infer the constructor access trick
    ConstructorTrick, // Infer the constructor trick (e.g. []['constructor'] -> ƒ -> Array() { [native code] }
    ToString,         // Infer toString calls
    B64,              // Infer atob & btoa calls and reduce them to string literals
    Var,              // Track variable assignments and propagate known values to usage sites
    FnCall,           // Resolve predictable function calls to their return values
    StrictEq,         // Infer strict equality === and !==
    LooseEq,          // Infer strict equality == and !=
    CmpOrd            // Infer comparison operators <, >, <= and >=
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
