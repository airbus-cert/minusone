extern crate minusone;

use minusone::engine::DeobfuscationBackend;
use minusone::js::backend::JavaScriptBackend;
use minusone::ps::backend::PowershellBackend;
use minusone::trace::Stepper as CoreStepper;
use minusone::{engine::DeobfuscateEngine, error::Error as MinusoneError};
use std::cell::RefCell;
use std::fmt::{Debug, Display};

enum MinusonejsError {
    MinusoneError(MinusoneError),
    JsError(String),
}

impl Display for MinusonejsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            MinusonejsError::JsError(e) => e.clone(),
            MinusonejsError::MinusoneError(e) => match e {
                MinusoneError::MinusOneError(inner) => inner.message.clone(),
                MinusoneError::Utf8Error(u) => u.to_string(),
            },
        };
        write!(f, "{}", str)
    }
}

wit_bindgen::generate!({
    world: "minusone",
});

const LANGUAGES: [&str; 2] = ["Powershell", "JavaScript"];

fn return_res(res: String) -> (String, String) {
    (res, String::from(""))
}

fn return_err(err: String) -> (String, String) {
    (String::from(""), err)
}

pub(crate) fn run_deobf<B: DeobfuscationBackend>(
    source: &str,
    rule_set: Option<Vec<String>>,
    skip_rule_set: Option<Vec<String>>,
) -> Result<String, MinusonejsError>
where
    <B as DeobfuscationBackend>::Language: Debug,
{
    let cleaned = DeobfuscateEngine::<B>::remove_extra(source, false)
        .map_err(MinusonejsError::MinusoneError)?;
    let mut engine =
        DeobfuscateEngine::<B>::from_source(&cleaned).map_err(MinusonejsError::MinusoneError)?;

    if let Some(rules) = rule_set {
        engine
            .deobfuscate_with_custom_ruleset(rules.iter().map(AsRef::as_ref).collect())
            .map_err(MinusonejsError::MinusoneError)?;
    } else if let Some(skip_rules) = skip_rule_set {
        engine
            .deobfuscate_without_custom_ruleset(skip_rules.iter().map(AsRef::as_ref).collect())
            .map_err(MinusonejsError::MinusoneError)?;
    } else {
        engine
            .deobfuscate()
            .map_err(MinusonejsError::MinusoneError)?;
    }

    engine.lint(false).map_err(MinusonejsError::MinusoneError)
}

struct Minusone;
impl Guest for Minusone {
    fn get_languages() -> Vec<String> {
        LANGUAGES.iter().map(|s| s.to_string()).collect()
    }

    fn deobfuscate(source: String, language: String) -> (String, String) {
        let result = match language.to_lowercase().as_str() {
            "ps" | "ps1" | "powershell" => run_deobf::<PowershellBackend>(&source, None, None),
            "js" | "javascript" => run_deobf::<JavaScriptBackend>(&source, None, None),
            _ => Err(MinusonejsError::JsError(format!(
                "Unsupported language: {}. Supported languages are: {:?}",
                language, LANGUAGES
            ))),
        };

        match result {
            Ok(r) => return_res(r),
            Err(e) => return_err(e.to_string()),
        }
    }

    fn deobfuscate_with(
        source: String,
        language: String,
        ruleset: Vec<String>,
    ) -> (String, String) {
        let result = match language.to_lowercase().as_str() {
            "ps" | "ps1" | "powershell" => {
                run_deobf::<PowershellBackend>(&source, Some(ruleset), None)
            }
            "js" | "javascript" => run_deobf::<JavaScriptBackend>(&source, Some(ruleset), None),
            _ => Err(MinusonejsError::JsError(format!(
                "Unsupported language: {}. Supported languages are: {:?}",
                language, LANGUAGES
            ))),
        };

        match result {
            Ok(r) => return_res(r),
            Err(e) => return_err(e.to_string()),
        }
    }

    fn deobfuscate_without(
        source: String,
        language: String,
        ruleset: Vec<String>,
    ) -> (String, String) {
        let result = match language.to_lowercase().as_str() {
            "ps" | "ps1" | "powershell" => {
                run_deobf::<PowershellBackend>(&source, None, Some(ruleset))
            }
            "js" | "javascript" => run_deobf::<JavaScriptBackend>(&source, None, Some(ruleset)),
            _ => Err(MinusonejsError::JsError(format!(
                "Unsupported language: {}. Supported languages are: {:?}",
                language, LANGUAGES
            ))),
        };

        match result {
            Ok(r) => return_res(r),
            Err(e) => return_err(e.to_string()),
        }
    }
}

fn run_stepper(
    source: &str,
    language: &str,
    record_all: bool,
) -> Result<CoreStepper, MinusonejsError> {
    match language.to_lowercase().as_str() {
        "ps" | "ps1" | "powershell" => PowershellBackend::stepper(source, false, record_all),
        "js" | "javascript" => JavaScriptBackend::stepper(source, false, record_all),
        _ => {
            return Err(MinusonejsError::JsError(format!(
                "Unsupported language: {}. Supported languages are: {:?}",
                language, LANGUAGES
            )));
        }
    }
    .map_err(MinusonejsError::MinusoneError)
}

struct StepperResource(RefCell<CoreStepper>);

impl exports::airbus_cert::minusone::trace::GuestStepper for StepperResource {
    fn next(&self) -> Option<exports::airbus_cert::minusone::trace::TraceStep> {
        Iterator::next(&mut *self.0.borrow_mut()).map(|step| {
            exports::airbus_cert::minusone::trace::TraceStep {
                phase: step.phase.to_string(),
                rule: step.rule,
                kind: step.kind.to_string(),
                start: step.start as u32,
                end: step.end as u32,
                source: step.source,
                old: step.old,
                new: step.new,
            }
        })
    }
}

impl exports::airbus_cert::minusone::trace::Guest for Minusone {
    type Stepper = StepperResource;

    fn new_stepper(
        source: String,
        language: String,
        record_all: bool,
    ) -> (
        Option<exports::airbus_cert::minusone::trace::Stepper>,
        String,
    ) {
        match run_stepper(&source, &language, record_all) {
            Ok(stepper) => (
                Some(exports::airbus_cert::minusone::trace::Stepper::new(
                    StepperResource(RefCell::new(stepper)),
                )),
                String::new(),
            ),
            Err(e) => (None, e.to_string()),
        }
    }
}

export!(Minusone);
