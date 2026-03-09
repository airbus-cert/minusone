use crate::cli::Language;
use minusone::engine::{DeobfuscateEngine, DeobfuscationBackend};
use minusone::error::MinusOneResult;
use minusone::js::backend::JavaScriptBackend;
use minusone::ps::backend::PowershellBackend;
use std::fmt::Debug;

pub(crate) fn run_deobf<B: DeobfuscationBackend>(
    source: &str,
    debug: bool,
    rule_set: Option<Vec<String>>,
    skip_rule_set: Option<Vec<String>>,
) -> MinusOneResult<()>
where
    <B as DeobfuscationBackend>::Language: Debug,
{
    let cleaned = DeobfuscateEngine::<B>::remove_extra(source)?;

    let mut engine = DeobfuscateEngine::<B>::from_source(&cleaned)?;

    if let Some(rules) = rule_set {
        engine.deobfuscate_with_custom_ruleset(rules.iter().map(AsRef::as_ref).collect())?;
    } else if let Some(skip_rules) = skip_rule_set {
        engine
            .deobfuscate_without_custom_ruleset(skip_rules.iter().map(AsRef::as_ref).collect())?;
    } else {
        engine.deobfuscate()?;
    }

    if debug {
        engine.debug();
        println!("\n\n");
    }
    println!("{}", engine.lint()?);

    Ok(())
}

pub(crate) fn get_available_rules(language: Language) -> Vec<String> {
    let rules = match language {
        Language::Powershell => DeobfuscateEngine::<PowershellBackend>::language_rules(),
        Language::Javascript => DeobfuscateEngine::<JavaScriptBackend>::language_rules(),
    };

    rules.into_iter().map(String::from).collect()
}
