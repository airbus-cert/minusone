use crate::engine::{CleanBackend, CleanEngine, DeobfuscateEngine, DeobfuscationBackend};
use crate::error::MinusOneResult;
use crate::js::post_process::*;
use crate::js::strategy::JavaScriptStrategy;
use crate::js::{
    JavaScript, JavaScriptRuleSet, build_javascript_tree_for_storage, remove_javascript_extra,
};
use crate::rule::RuleSetBuilderType;
use crate::tree::{EmptyStorage, HashMapStorage, Tree};
use log::{error, trace};

pub struct JavaScriptBackend;

impl JavaScriptBackend {
    /// Same as `remove_extra`, but records every named pre-processing
    /// transform that actually changed the source as a `Step`.
    pub fn remove_extra_traced(
        src: &str,
        keep_dead_code: bool,
    ) -> MinusOneResult<(String, Vec<crate::js::trace::Step>)> {
        let mut steps = Vec::new();
        let out = remove_extra_impl(src, keep_dead_code, &mut |rule, current| {
            crate::js::trace::push_text_step(&mut steps, "pre", rule, current);
        })?;
        Ok((out, steps))
    }

    /// Same as `lint_tree`, but records every named post-processing
    /// transform (linting + cleanup) that actually changed the source as a
    /// `Step`.
    pub fn lint_traced(
        root: &Tree<HashMapStorage<JavaScript>>,
        keep_dead_code: bool,
    ) -> MinusOneResult<(String, Vec<crate::js::trace::Step>)> {
        let mut steps = Vec::new();
        let out = lint_impl(root, keep_dead_code, &mut |rule, current| {
            crate::js::trace::push_text_step(&mut steps, "post", rule, current);
        })?;
        Ok((out, steps))
    }
}

/// Shared implementation of `remove_extra`, calling back `on_step` with the
/// name of each named transform and the source right after it, so a traced
/// variant can record them without duplicating this pipeline.
fn remove_extra_impl(
    src: &str,
    keep_dead_code: bool,
    on_step: &mut dyn FnMut(&str, &str),
) -> MinusOneResult<String> {
    // remove comments and other non-code nodes
    let mut current = remove_javascript_extra(src)?;
    on_step("RemoveComment", &current);

    // inline simple anonymous IIFEs so classic rules can see direct statements
    {
        let tree = build_javascript_tree_for_storage::<EmptyStorage>(&current)?;
        let mut inline_iife = InlineIife::default();
        tree.apply(&mut inline_iife)?;
        current = inline_iife.clear()?;
    }
    on_step("InlineIife", &current);

    // rewrite augmented assignments to plain assignments for easier follow-up processing
    {
        let tree = build_javascript_tree_for_storage::<EmptyStorage>(&current)?;
        let mut rewrite_augmented = ExpandAugmentedAssignment::default();
        tree.apply(&mut rewrite_augmented)?;
        current = rewrite_augmented.clear()?;
    }
    on_step("ExpandAugmentedAssignment", &current);

    // reduce safe comma sequences to their last expression
    {
        let tree = build_javascript_tree_for_storage::<EmptyStorage>(&current)?;
        let mut reduce_sequence = ReduceSequenceExpression::default();
        tree.apply(&mut reduce_sequence)?;
        current = reduce_sequence.clear()?;
    }
    on_step("ReduceSequenceExpression", &current);

    #[cfg(debug_assertions)]
    {
        let debug_dir = std::path::Path::new("./debug");
        if !debug_dir.exists() {
            std::fs::create_dir_all(debug_dir).expect("Failed to create debug directory");
        }
    }

    // sanitize var names
    {
        let tree = build_javascript_tree_for_storage::<EmptyStorage>(&current)?;
        let mut sanitize = SanitizeVarNames::default();
        tree.apply(&mut sanitize)?;
        current = sanitize.clear()?;
    }
    on_step("SanitizeVarNames", &current);

    // remove obvious dead code
    if !keep_dead_code {
        let mut i = 0;
        loop {
            i += 1;
            trace!(
                "Pre-clean pass iteration {}: current code length = {}",
                i,
                current.len()
            );

            #[cfg(debug_assertions)]
            {
                let debug_dir = std::path::Path::new("./debug");
                let debug_file = debug_dir.join(format!("debug_pre_pass_{}.js", i));
                std::fs::write(debug_file, &current).expect("Failed to write debug file");
            }

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
            on_step("RemoveUnused", &current);
        }
    }

    // simplify bracket calls to member expressions to help some rules
    {
        let tree = build_javascript_tree_for_storage::<EmptyStorage>(&current)?;
        let mut bracket_to_member = BracketCallToMember::default();
        tree.apply(&mut bracket_to_member)?;
        current = bracket_to_member.clear()?;
    }
    on_step("BracketCallToMember", &current);

    Ok(current)
}

/// Shared implementation of `clean_tree`, calling back `on_step` the same
/// way as `remove_extra_impl`.
fn clean_impl(
    mut current: String,
    keep_dead_code: bool,
    on_step: &mut dyn FnMut(&str, &str),
) -> MinusOneResult<String> {
    // remove remaining dead code
    if !keep_dead_code {
        let mut i = 0;
        loop {
            i += 1;
            trace!(
                "Post-lean pass iteration {}: current code length = {}",
                i,
                current.len()
            );

            #[cfg(debug_assertions)]
            {
                let debug_dir = std::path::Path::new("./debug");
                let debug_file = debug_dir.join(format!("debug_post_pass_{}.js", i));
                std::fs::write(debug_file, &current).expect("Failed to write debug file");
            }

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
            on_step("RemoveUnused", &current);
        }
    }

    // simplify bracket calls to member expressions to make it more "human readable"
    let tree = build_javascript_tree_for_storage::<EmptyStorage>(&current)?;
    let mut bracket_to_member = BracketCallToMember::default();
    tree.apply(&mut bracket_to_member)?;
    current = bracket_to_member.clear()?;
    on_step("BracketCallToMember", &current);

    let tree = build_javascript_tree_for_storage::<EmptyStorage>(&current)?;
    let mut global_this_simplifier = GlobalThisSimplifier::default();
    tree.apply(&mut global_this_simplifier)?;
    current = global_this_simplifier.clear()?;
    on_step("GlobalThisSimplifier", &current);

    // simplify some for loops to while loops
    let tree = build_javascript_tree_for_storage::<EmptyStorage>(&current)?;
    let mut for_to_while = ForToWhile::default();
    tree.apply(&mut for_to_while)?;
    current = for_to_while.clear()?;
    on_step("ForToWhile", &current);

    Ok(current)
}

/// Shared implementation of `lint_tree`, calling back `on_step` the same
/// way as `remove_extra_impl`.
fn lint_impl(
    root: &Tree<HashMapStorage<JavaScript>>,
    keep_dead_code: bool,
    on_step: &mut dyn FnMut(&str, &str),
) -> MinusOneResult<String> {
    let mut linter = crate::js::linter::Linter::default();
    root.apply(&mut linter)?;
    on_step("Linter", &linter.output);

    // fallback to returning the linted output without cleaning if the clean pass fails
    match clean_impl(linter.output.clone(), keep_dead_code, on_step) {
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

impl DeobfuscationBackend for JavaScriptBackend {
    type Language = JavaScript;

    fn remove_extra(src: &str, keep_dead_code: bool) -> MinusOneResult<String> {
        remove_extra_impl(src, keep_dead_code, &mut |_, _| {})
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
        keep_dead_code: bool,
    ) -> MinusOneResult<String> {
        lint_impl(root, keep_dead_code, &mut |_, _| {})
    }

    fn language_rules<'a>() -> Vec<&'a str> {
        JavaScriptRuleSet::new(RuleSetBuilderType::WithoutRules(vec![])).names()
    }
}

impl CleanBackend for JavaScriptBackend {
    fn build_clean_tree<'a>(src: &'a str) -> MinusOneResult<Tree<'a, EmptyStorage>> {
        build_javascript_tree_for_storage(src)
    }

    fn clean_tree(root: &Tree<EmptyStorage>, keep_dead_code: bool) -> MinusOneResult<String> {
        let current = root.root()?.text()?.to_string();
        clean_impl(current, keep_dead_code, &mut |_, _| {})
    }
}

impl<'a> DeobfuscateEngine<'a, JavaScriptBackend> {
    pub fn from_javascript(src: &'a str) -> MinusOneResult<Self> {
        Self::from_source(src)
    }
    pub fn deobfuscate_traced(&mut self) -> MinusOneResult<Vec<crate::js::trace::Step>> {
        let mut tracer = crate::js::trace::TracingRuleSet::new(JavaScriptRuleSet::new(
            RuleSetBuilderType::WithoutRules(vec![]),
        ));
        self.root_mut()
            .apply_mut_with_strategy(&mut tracer, JavaScriptStrategy)?;
        Ok(tracer.steps)
    }
}

impl<'a> CleanEngine<'a, JavaScriptBackend> {
    pub fn from_javascript(src: &'a str) -> MinusOneResult<Self> {
        Self::from_source(src)
    }
}
