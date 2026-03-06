use engine::{DeobfuscateEngine, DeobfuscationBackend};
use error::MinusOneResult;
use js;
use js::{build_javascript_tree_for_storage, remove_javascript_extra};
use rule::RuleSetBuilderType;
use tree::{HashMapStorage, Tree};

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
        Ok(linter.output)
    }

    fn language_rules<'a>() -> Vec<&'a str> {
        js::JavaScriptRuleSet::new(RuleSetBuilderType::WithoutRules(vec![])).names()
    }
}

impl<'a> DeobfuscateEngine<'a, JavaScriptBackend> {
    pub fn from_javascript(src: &'a str) -> MinusOneResult<Self> {
        Self::from_source(src)
    }
}
