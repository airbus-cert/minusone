extern crate minusone;
use minusone::{engine::DeobfuscateEngine, error::Error};

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

fn deobfuscate_powershell(source: String) -> Result<String, Error> {
    let without_comments = DeobfuscateEngine::remove_extra(&source)?;
    let mut engine = DeobfuscateEngine::from_powershell(&without_comments)?;
    engine.deobfuscate()?;

    engine.lint()
}

struct Minusone;
impl Guest for Minusone {
    fn get_languages() -> Vec<String> {
        LANGUAGES.iter().map(|s| s.to_string()).collect()
    }

    fn deobfuscate(source: String, language: String) -> (String, String) {
        match language.as_str() {
            "Powershell" => match deobfuscate_powershell(source) {
                Ok(r) => return_res(r),
                Err(e) => return_err(format!("{e:?}")),
            },
            _ => return_err(format!("Error: Language {} not implemented", language)),
        }
    }
}

export!(Minusone);
