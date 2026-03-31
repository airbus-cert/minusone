use crate::engine::{CleanBackend, CleanEngine, DeobfuscateEngine, DeobfuscationBackend};
use crate::error::MinusOneResult;
use crate::js::deadcode::{RemoveUnusedVar, UnusedVar};
use crate::js::strategy::JavaScriptStrategy;
use crate::js::{
    JavaScript, JavaScriptRuleSet, build_javascript_tree_for_storage, remove_javascript_extra,
};
use crate::rule::RuleSetBuilderType;
use crate::tree::{EmptyStorage, HashMapStorage, Tree};
use log::{error, trace};

pub struct JavaScriptBackend;

impl DeobfuscationBackend for JavaScriptBackend {
    type Language = JavaScript;

    fn remove_extra(src: &str) -> MinusOneResult<String> {
        remove_javascript_extra(src)
    }

    fn build_deob_tree<'a>(
        src: &'a str,
    ) -> MinusOneResult<Tree<'a, HashMapStorage<Self::Language>>> {
        build_javascript_tree_for_storage(src)
    }

    fn deobfuscate_tree(root: &mut Tree<HashMapStorage<Self::Language>>) -> MinusOneResult<()> {
        root.apply_mut_with_strategy(
            &mut JavaScriptRuleSet::new(RuleSetBuilderType::WithoutRules(vec![])),
            JavaScriptStrategy,
        )?;
        Ok(())
    }

    fn deobfuscate_tree_with_custom_ruleset(
        root: &mut Tree<HashMapStorage<Self::Language>>,
        ruleset: Vec<&str>,
    ) -> MinusOneResult<()> {
        root.apply_mut_with_strategy(
            &mut JavaScriptRuleSet::new(RuleSetBuilderType::WithRules(ruleset)),
            JavaScriptStrategy,
        )?;
        Ok(())
    }

    fn deobfuscate_tree_without_custom_ruleset(
        root: &mut Tree<HashMapStorage<Self::Language>>,
        ruleset: Vec<&str>,
    ) -> MinusOneResult<()> {
        root.apply_mut_with_strategy(
            &mut JavaScriptRuleSet::new(RuleSetBuilderType::WithoutRules(ruleset)),
            JavaScriptStrategy,
        )?;
        Ok(())
    }

    fn lint_tree<'a>(
        root: &Tree<'a, HashMapStorage<Self::Language>>,
        _tab_chr: &str,
    ) -> MinusOneResult<String> {
        let mut linter = crate::js::linter::Linter::default();
        let linted = crate::printer::code_string(&mut linter, root)?;

        // fallback to returning the linted output without cleaning if the clean pass fails
        match CleanEngine::<JavaScriptBackend>::from_source(&linted)
            .and_then(|mut e| e.clean())
        {
            Ok(cleaned) => Ok(cleaned),
            Err(e) => {
                error!(
                    "Clean pass failed during linting: {:?}. Returning linted output without cleaning.",
                    e
                );
                Ok(linted)
            }
        }
    }

    fn language_rules<'a>() -> Vec<&'a str> {
        JavaScriptRuleSet::new(RuleSetBuilderType::WithoutRules(vec![])).names()
    }
}

impl CleanBackend for JavaScriptBackend {
    fn build_clean_tree<'a>(src: &'a str) -> MinusOneResult<Tree<'a, EmptyStorage>> {
        build_javascript_tree_for_storage(src)
    }

    fn clean_tree(root: &Tree<EmptyStorage>) -> MinusOneResult<String> {
        let mut current = root.root()?.text()?.to_string();

        // re-run deadcode elimination until no more nodes are removed, this handles cascading cases
        for i in 0..16 {
            trace!(
                "Clean pass iteration {}: current code length = {}",
                i + 1,
                current.len()
            );
            let tree = build_javascript_tree_for_storage::<EmptyStorage>(&current)?;

            let mut rule = UnusedVar::default();
            tree.apply(&mut rule)?;

            let mut clean_view = RemoveUnusedVar::new(rule);
            tree.apply(&mut clean_view)?;
            let next = clean_view.clear()?;

            if next == current {
                return Ok(next);
            }

            current = next;
        }

        Ok(current)
    }
}

impl<'a> DeobfuscateEngine<'a, JavaScriptBackend> {
    pub fn from_javascript(src: &'a str) -> MinusOneResult<Self> {
        Self::from_source(src)
    }
}

impl<'a> CleanEngine<'a, JavaScriptBackend> {
    pub fn from_javascript(src: &'a str) -> MinusOneResult<Self> {
        Self::from_source(src)
    }
}
