use crate::engine::{CleanBackend, CleanEngine, DeobfuscateEngine, DeobfuscationBackend};
use crate::error::MinusOneResult;
use crate::init::Init;
use crate::ps;
use crate::rule::RuleSetBuilderType;
use crate::tree::{EmptyStorage, HashMapStorage, Tree};
use ps::linter::RemoveUnusedVar;
use ps::{build_powershell_tree_for_storage, remove_powershell_extra};

pub struct PowershellBackend;

impl PowershellBackend {
    pub fn remove_extra_traced(src: &str) -> MinusOneResult<(String, Vec<crate::trace::Step>)> {
        let mut steps = Vec::new();
        let out = remove_powershell_extra(src)?;
        crate::trace::push_text_step(&mut steps, "pre", "RemoveComment", &out);
        Ok((out, steps))
    }

    pub fn lint_traced(
        root: &Tree<HashMapStorage<ps::Powershell>>,
        tab_chr: &str,
        keep_dead_code: bool,
    ) -> MinusOneResult<(String, Vec<crate::trace::Step>)> {
        let mut steps = Vec::new();

        let mut ps_linter_view = ps::linter::Linter::default().set_tab(tab_chr);
        root.apply(&mut ps_linter_view)?;
        crate::trace::push_text_step(&mut steps, "post", "Linter", &ps_linter_view.output);

        let out = CleanEngine::<PowershellBackend>::from_source(&ps_linter_view.output)?
            .clean(keep_dead_code)?;
        crate::trace::push_text_step(&mut steps, "post", "RemoveUnusedVar", &out);

        Ok((out, steps))
    }
}

impl DeobfuscationBackend for PowershellBackend {
    type Language = ps::Powershell;

    fn remove_extra(src: &str, _keep_dead_code: bool) -> MinusOneResult<String> {
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
        keep_dead_code: bool,
    ) -> MinusOneResult<String> {
        let mut ps_linter_view = ps::linter::Linter::default().set_tab(tab_chr);
        root.apply(&mut ps_linter_view)?;

        CleanEngine::<PowershellBackend>::from_source(&ps_linter_view.output)?.clean(keep_dead_code)
    }

    fn language_rules<'a>() -> Vec<&'a str> {
        ps::PowershellRuleSet::new(RuleSetBuilderType::WithoutRules(vec![])).names()
    }
}

impl CleanBackend for PowershellBackend {
    fn build_clean_tree<'a>(src: &'a str) -> MinusOneResult<Tree<'a, EmptyStorage>> {
        build_powershell_tree_for_storage(src)
    }

    fn clean_tree(root: &Tree<EmptyStorage>, keep_dead_code: bool) -> MinusOneResult<String> {
        if !keep_dead_code {
            let mut rule = ps::var::UnusedVar::default();
            root.apply(&mut rule)?;
            let mut clean_view = RemoveUnusedVar::new(rule);
            root.apply(&mut clean_view)?;

            clean_view.clear()
        } else {
            Ok(root.root()?.text()?.to_string())
        }
    }
}

impl<'a> DeobfuscateEngine<'a, PowershellBackend> {
    pub fn from_powershell(src: &'a str) -> MinusOneResult<Self> {
        Self::from_source(src)
    }

    pub fn deobfuscate_traced(&mut self) -> MinusOneResult<Vec<crate::trace::Step>> {
        let mut tracer = ps::trace::TracingRuleSet::new(ps::PowershellRuleSet::new(
            RuleSetBuilderType::WithoutRules(vec![]),
        ));
        self.root_mut()
            .apply_mut_with_strategy(&mut tracer, ps::strategy::PowershellStrategy)?;
        Ok(tracer.steps)
    }
}

impl<'a> CleanEngine<'a, PowershellBackend> {
    pub fn from_powershell(src: &'a str) -> MinusOneResult<Self> {
        Self::from_source(src)
    }
}
