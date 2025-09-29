extern crate minusone;
use minusone::engine::DeobfuscateEngine;

wit_bindgen::generate!({
    world: "minusone",
});

const LANGUAGES: [&str; 1] = ["Powershell"];

struct Minusone;
impl Guest for Minusone {
    fn get_languages() -> Vec<String> {
        LANGUAGES.iter().map(|s| s.to_string()).collect()
    }

    fn deobfuscate(source: String, language: String) -> Result<String, String> {
        match language.as_str() {
            "Powershell" => {
                let without_comments =
                    DeobfuscateEngine::remove_extra(&source).map_err(|e| format!("{e:?}"))?;

                let mut engine = DeobfuscateEngine::from_powershell(&without_comments)
                    .map_err(|e| format!("{e:?}"))?;
                engine.deobfuscate().map_err(|e| format!("{e:?}"))?;

                engine.lint().map_err(|e| format!("{e:?}"))
            }
            _ => Err(format!("Error: Language {} not implemented", language)),
        }
    }
}

export!(Minusone);
