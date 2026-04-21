use crate::engine::{CleanBackend, CleanEngine, DeobfuscateEngine, DeobfuscationBackend};
use crate::error::MinusOneResult;
use crate::js::post_process::{BracketCallToMember, ForToWhile, RemoveUnused, UnusedVar};
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
        // remove comments and other non-code nodes
        let mut current = remove_javascript_extra(src)?;

        // remove obvious dead code
        let mut i = 0;
        loop {
            i += 1;
            trace!(
                "Pre-clean pass iteration {}: current code length = {}",
                i,
                current.len()
            );

            let tree = build_javascript_tree_for_storage::<EmptyStorage>(&current)?;

            let mut rule = UnusedVar::default();
            tree.apply(&mut rule)?;

            let mut clean_view = RemoveUnused::new(rule);
            tree.apply(&mut clean_view)?;
            let next = clean_view.clear()?;

            if next == current {
                current = next;
                break;
            }

            current = next;
        }

        // simplify bracket calls to member expressions to help some rules
        {
            let tree = build_javascript_tree_for_storage::<EmptyStorage>(&current)?;
            let mut bracket_to_member = BracketCallToMember::default();
            tree.apply(&mut bracket_to_member)?;
            current = bracket_to_member.clear()?;
        }

        Ok(current)
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
        root.apply(&mut linter)?;

        // fallback to returning the linted output without cleaning if the clean pass fails
        match CleanEngine::<JavaScriptBackend>::from_source(&linter.output)
            .and_then(|mut e| e.clean())
        {
            Ok(cleaned) => Ok(cleaned),
            Err(e) => {
                error!(
                    "Clean pass failed during linting: {:?}. Returning linted output without cleaning.",
                    e
                );
                Ok(linter.output)
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

        // remove remaining dead code
        let mut i = 0;
        loop {
            i += 1;
            trace!(
                "Post-lean pass iteration {}: current code length = {}",
                i,
                current.len()
            );

            let tree = build_javascript_tree_for_storage::<EmptyStorage>(&current)?;

            let mut rule = UnusedVar::default();
            tree.apply(&mut rule)?;

            let mut clean_view = RemoveUnused::new(rule);
            tree.apply(&mut clean_view)?;
            let next = clean_view.clear()?;

            if next == current {
                current = next;
                break;
            }

            current = next;
        }

        // simplify bracket calls to member expressions to make it more "human readable"
        let tree = build_javascript_tree_for_storage::<EmptyStorage>(&current)?;
        let mut bracket_to_member = BracketCallToMember::default();
        tree.apply(&mut bracket_to_member)?;
        current = bracket_to_member.clear()?;

        // simplify some for loops to while loops
        let tree = build_javascript_tree_for_storage::<EmptyStorage>(&current)?;
        let mut for_to_while = ForToWhile::default();
        tree.apply(&mut for_to_while)?;
        current = for_to_while.clear()?;

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
