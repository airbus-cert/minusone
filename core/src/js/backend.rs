use engine::{CleanBackend, CleanEngine, DeobfuscateEngine, DeobfuscationBackend};
use error::MinusOneResult;
use js;
use js::deadcode::{RemoveUnusedVar, UnusedVar};
use js::{build_javascript_tree_for_storage, remove_javascript_extra};
use rule::RuleSetBuilderType;
use tree::{EmptyStorage, HashMapStorage, Tree};

pub struct JavaScriptBackend;

impl DeobfuscationBackend for JavaScriptBackend {
    type Language = js::JavaScript;

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
            &mut js::JavaScriptRuleSet::new(RuleSetBuilderType::WithoutRules(vec![])),
            js::strategy::JavaScriptStrategy,
        )?;
        Ok(())
    }

    fn deobfuscate_tree_with_custom_ruleset(
        root: &mut Tree<HashMapStorage<Self::Language>>,
        ruleset: Vec<&str>,
    ) -> MinusOneResult<()> {
        root.apply_mut_with_strategy(
            &mut js::JavaScriptRuleSet::new(RuleSetBuilderType::WithRules(ruleset)),
            js::strategy::JavaScriptStrategy,
        )?;
        Ok(())
    }

    fn deobfuscate_tree_without_custom_ruleset(
        root: &mut Tree<HashMapStorage<Self::Language>>,
        ruleset: Vec<&str>,
    ) -> MinusOneResult<()> {
        root.apply_mut_with_strategy(
            &mut js::JavaScriptRuleSet::new(RuleSetBuilderType::WithoutRules(ruleset)),
            js::strategy::JavaScriptStrategy,
        )?;
        Ok(())
    }

    fn lint_tree<'a>(
        root: &Tree<'a, HashMapStorage<Self::Language>>,
        _tab_chr: &str,
    ) -> MinusOneResult<String> {
        let mut linter = js::linter::Linter::default();
        root.apply(&mut linter)?;

        CleanEngine::<JavaScriptBackend>::from_source(&linter.output)?.clean()
    }

    fn language_rules<'a>() -> Vec<&'a str> {
        js::JavaScriptRuleSet::new(RuleSetBuilderType::WithoutRules(vec![])).names()
    }
}

impl CleanBackend for JavaScriptBackend {
    fn build_clean_tree<'a>(src: &'a str) -> MinusOneResult<Tree<'a, EmptyStorage>> {
        build_javascript_tree_for_storage(src)
    }

    fn clean_tree(root: &Tree<EmptyStorage>) -> MinusOneResult<String> {
        let mut rule = UnusedVar::default();
        root.apply(&mut rule)?;
        let mut clean_view = RemoveUnusedVar::new(rule);
        root.apply(&mut clean_view)?;
        clean_view.clear()
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
