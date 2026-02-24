extern crate minusone;
use minusone::{engine::DeobfuscateEngine, error::Error as MinusoneError};

enum MinusonejsError {
    MinusoneError(MinusoneError),
    JsError(String),
}

impl ToString for MinusonejsError {
    fn to_string(&self) -> String {
        match self {
            MinusonejsError::JsError(e) => e.clone(),
            MinusonejsError::MinusoneError(e) => format!("{e:?}"),
        }
    }
}

wit_bindgen::generate!({
    world: "minusone",
});

const LANGUAGES: [&str; 1] = ["Powershell"];

fn return_res(res: String) -> (String, String) {
    (res, String::from(""))
}

fn return_err(err: String) -> (String, String) {
    (String::from(""), format!("{err:?}"))
}

fn deobfuscate(
    source: String,
    language: String,
    ruleset: Vec<String>,
    with: bool,
) -> Result<String, MinusonejsError> {
    match language.to_lowercase().as_str() {
        "powershell" => deobfuscate_powershell(source, ruleset, with)
            .map_err(|e| MinusonejsError::MinusoneError(e)),
        _ => Err(MinusonejsError::JsError(format!(
            "Unsupported language: {}",
            language,
        ))),
    }
}

fn deobfuscate_powershell(
    source: String,
    ruleset: Vec<String>,
    with: bool,
) -> Result<String, MinusoneError> {
    let without_comments = DeobfuscateEngine::remove_extra(&source)?;
    let mut engine = DeobfuscateEngine::from_powershell(&without_comments)?;

    match (ruleset.len(), with) {
        (0, _) => engine.deobfuscate(),
        (_, true) => {
            engine.deobfuscate_with_custom_ruleset(ruleset.iter().map(AsRef::as_ref).collect())
        }
        (_, false) => {
            engine.deobfuscate_without_custom_ruleset(ruleset.iter().map(AsRef::as_ref).collect())
        }
    }?;

    engine.lint()
}

struct Minusone;
impl Guest for Minusone {
    fn get_languages() -> Vec<String> {
        LANGUAGES.iter().map(|s| s.to_string()).collect()
    }

    fn deobfuscate(source: String, language: String) -> (String, String) {
        match deobfuscate(source, language, vec![], false) {
            Ok(r) => return_res(r),
            Err(e) => return_err(e.to_string()),
        }
    }

    fn deobfuscate_with(
        source: String,
        language: String,
        ruleset: Vec<String>,
    ) -> (String, String) {
        match deobfuscate(source, language, ruleset, true) {
            Ok(r) => return_res(r),
            Err(e) => return_err(e.to_string()),
        }
    }

    fn deobfuscate_without(
        source: String,
        language: String,
        ruleset: Vec<String>,
    ) -> (String, String) {
        match deobfuscate(source, language, ruleset, false) {
            Ok(r) => return_res(r),
            Err(e) => return_err(e.to_string()),
        }
    }
}

export!(Minusone);
