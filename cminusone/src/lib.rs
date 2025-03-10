use minusone::engine::DeobfuscateEngine;

#[cxx::bridge(namespace = "cminusone")]
mod bridge {
    extern "Rust" {
        fn deobfuscate_powershell(src: String) -> String;
    }
}

fn deobfuscate_powershell(src: String) -> String {
    let remove_comment = DeobfuscateEngine::remove_extra(&src).unwrap();
    let mut engine = DeobfuscateEngine::from_powershell(&remove_comment).unwrap();
    engine.deobfuscate().unwrap();

    engine.lint().unwrap()
}
