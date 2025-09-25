extern crate minusone;
use minusone::engine::DeobfuscateEngine;

wit_bindgen::generate!({
    world: "minusone",
});

struct Minusone;
impl Guest for Minusone {
    fn deobfuscate(source: String) -> String {
        match DeobfuscateEngine::remove_extra(&source) {
            Ok(remove_comment) => {
                let mut engine = DeobfuscateEngine::from_powershell(&remove_comment).unwrap();
                engine.deobfuscate().unwrap();
                engine.lint().unwrap()
            }
            Err(e) => "ERROR: Cannot clean the source\n--> {:?}".to_string(),
        }
    }
}

export!(Minusone);
