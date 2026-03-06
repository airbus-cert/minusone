use crate::debug::DebugView;
use crate::error::MinusOneResult;
use crate::tree::{EmptyStorage, HashMapStorage, Tree};
use log::debug;
use std::fmt::Debug;
use std::marker::PhantomData;

pub trait DeobfuscationBackend {
    type Language;

    fn remove_extra(src: &str) -> MinusOneResult<String>;
    fn build_deob_tree<'a>(
        src: &'a str,
    ) -> MinusOneResult<Tree<'a, HashMapStorage<Self::Language>>>;
    fn deobfuscate_tree(root: &mut Tree<HashMapStorage<Self::Language>>) -> MinusOneResult<()>;

    fn deobfuscate_tree_with_custom_ruleset(
        root: &mut Tree<HashMapStorage<Self::Language>>,
        ruleset: Vec<&str>,
    ) -> MinusOneResult<()>;

    fn deobfuscate_tree_without_custom_ruleset(
        root: &mut Tree<HashMapStorage<Self::Language>>,
        ruleset: Vec<&str>,
    ) -> MinusOneResult<()>;
    fn lint_tree<'a>(
        root: &Tree<'a, HashMapStorage<Self::Language>>,
        tab_chr: &str,
    ) -> MinusOneResult<String>;

    fn language_rules<'a>() -> Vec<&'a str>;
}

pub struct DeobfuscateEngine<'a, B: DeobfuscationBackend> {
    root: Tree<'a, HashMapStorage<B::Language>>,
    backend: PhantomData<B>,
}

impl<'a, B: DeobfuscationBackend> DeobfuscateEngine<'a, B> {
    pub fn remove_extra(src: &str) -> MinusOneResult<String> {
        B::remove_extra(src)
    }

    pub fn from_source(src: &'a str) -> MinusOneResult<Self> {
        Ok(Self {
            root: B::build_deob_tree(src)?,
            backend: PhantomData,
        })
    }

    pub fn debug(&self)
    where
        B::Language: Debug,
    {
        let mut debug_view = DebugView::default();
        self.root.apply(&mut debug_view).unwrap();
    }

    pub fn deobfuscate(&mut self) -> MinusOneResult<()> {
        debug!(
            "Starting deobfuscation process with {} rules",
            B::language_rules().len()
        );
        B::deobfuscate_tree(&mut self.root)
    }

    pub fn lint(&mut self) -> MinusOneResult<String> {
        B::lint_tree(&self.root, "    ")
    }

    pub fn lint_format(&mut self, tab_chr: &str) -> MinusOneResult<String> {
        B::lint_tree(&self.root, tab_chr)
    }

    pub fn deobfuscate_with_custom_ruleset(&mut self, ruleset: Vec<&str>) -> MinusOneResult<()> {
        debug!(
            "Starting deobfuscation process with {} custom rules: {}",
            ruleset.len(),
            ruleset.join(", ")
        );
        B::deobfuscate_tree_with_custom_ruleset(&mut self.root, ruleset)
    }

    pub fn deobfuscate_without_custom_ruleset(&mut self, ruleset: Vec<&str>) -> MinusOneResult<()> {
        debug!(
            "Starting deobfuscation process with all rules except {}: {}",
            ruleset.len(),
            ruleset.join(", ")
        );
        B::deobfuscate_tree_without_custom_ruleset(&mut self.root, ruleset)
    }

    pub fn language_rules() -> Vec<&'a str> {
        B::language_rules()
    }
}

pub trait CleanBackend {
    fn build_clean_tree<'a>(src: &'a str) -> MinusOneResult<Tree<'a, EmptyStorage>>;
    fn clean_tree(root: &Tree<EmptyStorage>) -> MinusOneResult<String>;
}

pub struct CleanEngine<'a, B: CleanBackend> {
    root: Tree<'a, EmptyStorage>,
    backend: PhantomData<B>,
}

impl<'a, B: CleanBackend> CleanEngine<'a, B> {
    pub fn from_source(src: &'a str) -> MinusOneResult<Self> {
        Ok(Self {
            root: B::build_clean_tree(src)?,
            backend: PhantomData,
        })
    }

    pub fn clean(&mut self) -> MinusOneResult<String> {
        B::clean_tree(&self.root)
    }
}
