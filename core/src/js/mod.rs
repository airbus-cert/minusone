pub mod backend;
pub mod linter;
pub mod strategy;

use self::linter::RemoveComment;
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

impl Value {
    fn normalize(&self) -> Value {
        match self {
            Value::Str(x) => Value::Str(x.to_lowercase()),
            x => x.clone(),
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Value::Num(e) => e.to_string(),
                Value::Str(s) => s.clone(),
                Value::Bool(true) => "True".to_string(),
                Value::Bool(false) => "False".to_string(),
            }
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum JavaScript {
    Raw(Value),
}

impl Display for JavaScript {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JavaScript::Raw(v) => write!(f, "{}", v),
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

impl_javascript_ruleset!();

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
