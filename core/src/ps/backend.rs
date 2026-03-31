use crate::engine::{CleanBackend, CleanEngine, DeobfuscateEngine, DeobfuscationBackend};
use crate::error::MinusOneResult;
use crate::init::Init;
use crate::ps;
use crate::rule::RuleSetBuilderType;
use crate::tree::{EmptyStorage, HashMapStorage, Tree};
use ps::linter::RemoveUnusedVar;
use ps::{build_powershell_tree_for_storage, remove_powershell_extra};

pub struct PowershellBackend;

impl DeobfuscationBackend for PowershellBackend {
    type Language = ps::Powershell;

    fn remove_extra(src: &str) -> MinusOneResult<String> {
        remove_powershell_extra(src)
    }

    fn build_deob_tree<'a>(
        src: &'a str,
    ) -> MinusOneResult<Tree<'a, HashMapStorage<Self::Language>>> {
        build_powershell_tree_for_storage(src)
    }

    fn deobfuscate_tree(root: &mut Tree<HashMapStorage<Self::Language>>) -> MinusOneResult<()> {
        root.apply_mut_with_strategy(
            &mut ps::PowershellDefaultRuleSet::init(),
            ps::strategy::PowershellStrategy,
        )?;
        Ok(())
    }

    fn deobfuscate_tree_with_custom_ruleset(
        root: &mut Tree<HashMapStorage<Self::Language>>,
        ruleset: Vec<&str>,
    ) -> MinusOneResult<()> {
        root.apply_mut_with_strategy(
            &mut ps::PowershellRuleSet::new(RuleSetBuilderType::WithRules(ruleset)),
            ps::strategy::PowershellStrategy,
        )?;
        Ok(())
    }

    fn deobfuscate_tree_without_custom_ruleset(
        root: &mut Tree<HashMapStorage<Self::Language>>,
        ruleset: Vec<&str>,
    ) -> MinusOneResult<()> {
        root.apply_mut_with_strategy(
            &mut ps::PowershellRuleSet::new(RuleSetBuilderType::WithoutRules(ruleset)),
            ps::strategy::PowershellStrategy,
        )?;
        Ok(())
    }

    fn lint_tree<'a>(
        root: &Tree<'a, HashMapStorage<Self::Language>>,
        tab_chr: &str,
    ) -> MinusOneResult<String> {
        let mut ps_linter_view = ps::linter::Linter::default().set_tab(tab_chr);
        let linted = crate::printer::code_string(&mut ps_linter_view, root)?;

        CleanEngine::<PowershellBackend>::from_source(&linted)?.clean()
    }

    fn language_rules<'a>() -> Vec<&'a str> {
        ps::PowershellRuleSet::new(RuleSetBuilderType::WithoutRules(vec![])).names()
    }
}

impl CleanBackend for PowershellBackend {
    fn build_clean_tree<'a>(src: &'a str) -> MinusOneResult<Tree<'a, EmptyStorage>> {
        build_powershell_tree_for_storage(src)
    }

    fn clean_tree(root: &Tree<EmptyStorage>) -> MinusOneResult<String> {
        let mut rule = ps::var::UnusedVar::default();
        root.apply(&mut rule)?;
        let mut clean_view = RemoveUnusedVar::new(rule);
        root.apply(&mut clean_view)?;
        clean_view.clear()
    }
}

impl<'a> DeobfuscateEngine<'a, PowershellBackend> {
    pub fn from_powershell(src: &'a str) -> MinusOneResult<Self> {
        Self::from_source(src)
    }
}

impl<'a> CleanEngine<'a, PowershellBackend> {
    pub fn from_powershell(src: &'a str) -> MinusOneResult<Self> {
        Self::from_source(src)
    }
}
