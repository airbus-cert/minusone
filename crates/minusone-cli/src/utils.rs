use crate::cli::{Cli, DebugLevel, Language};
use crate::trace_view;
use log::{info, warn};
use minusone::debug::DebugView;
use minusone::engine::{DeobfuscateEngine, DeobfuscationBackend};
use minusone::error::MinusOneResult;
use minusone::js::backend::JavaScriptBackend;
use minusone::ps::backend::PowershellBackend;
use std::fmt::Debug;

pub(crate) fn run_deobf<B: DeobfuscationBackend>(
    source: &str,
    cli: Cli,
    rule_set: Option<Vec<String>>,
    skip_rule_set: Option<Vec<String>>,
    keep_dead_code: bool,
) -> MinusOneResult<()>
where
    <B as DeobfuscationBackend>::Language: Debug,
{
    let cleaned = DeobfuscateEngine::<B>::remove_extra(source, keep_dead_code)?;

    let mut engine = DeobfuscateEngine::<B>::from_source(&cleaned)?;

    if let Some(rules) = rule_set {
        engine.deobfuscate_with_custom_ruleset(rules.iter().map(AsRef::as_ref).collect())?;
    } else if let Some(skip_rules) = skip_rule_set {
        engine
            .deobfuscate_without_custom_ruleset(skip_rules.iter().map(AsRef::as_ref).collect())?;
    } else {
        engine.deobfuscate()?;
    }

    if cli.debug_level == DebugLevel::Debug || cli.debug_level == DebugLevel::Trace {
        let debug_view = DebugView::new(
            cli.debug_indent,
            !cli.debug_no_text,
            !cli.debug_no_count,
            !cli.debug_no_colors,
        );
        engine.debug(Some(debug_view));

        println!("\n\n");
    }

    println!("{}", engine.lint(keep_dead_code)?);
    Ok(())
}

pub(crate) fn run_deobf_js_traced(
    source: &str,
    cli: Cli,
    rule_set: Option<Vec<String>>,
    skip_rule_set: Option<Vec<String>>,
    keep_dead_code: bool,
) -> MinusOneResult<()> {
    if rule_set.is_some() || skip_rule_set.is_some() {
        warn!("Custom rule selection is not supported in trace mode; running the full ruleset");
    }

    let (cleaned, mut steps) =
        JavaScriptBackend::remove_extra_traced(source, keep_dead_code)?;
    let mut engine = DeobfuscateEngine::<JavaScriptBackend>::from_source(&cleaned)?;

    steps.extend(engine.deobfuscate_traced()?);

    if cli.debug_level == DebugLevel::Debug || cli.debug_level == DebugLevel::Trace {
        let debug_view = DebugView::new(
            cli.debug_indent,
            !cli.debug_no_text,
            !cli.debug_no_count,
            !cli.debug_no_colors,
        );
        engine.debug(Some(debug_view));

        println!("\n\n");
    }

    let (final_output, post_steps) =
        JavaScriptBackend::lint_traced(engine.root_mut(), keep_dead_code)?;
    steps.extend(post_steps);

    println!("{}", final_output);

    let html = trace_view::render(source, &steps);
    let out_path = "steps.html";
    match std::fs::write(out_path, html) {
        Ok(()) => info!(
            "Recorded {} reduction step(s), trace written to {}",
            steps.len(),
            out_path
        ),
        Err(e) => log::error!("Failed to write trace file {}: {}", out_path, e),
    }

    Ok(())
}

pub(crate) fn get_available_rules(language: Language) -> Vec<String> {
    let rules = match language {
        Language::Powershell => DeobfuscateEngine::<PowershellBackend>::language_rules(),
        Language::Javascript => DeobfuscateEngine::<JavaScriptBackend>::language_rules(),
    };

    rules.into_iter().map(String::from).collect()
}
