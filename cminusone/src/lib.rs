use minusone::engine::DeobfuscateEngine;
use widestring::U16String;

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

#[no_mangle]
pub extern "C" fn rust_function(buffer: *const u16, strlen: usize) {
    unsafe {
        let name = U16String::from_ptr(buffer, strlen as usize);
        println!("Hello: {}", name.to_string().unwrap());
    };
}
