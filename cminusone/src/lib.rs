use std::{ffi::c_int, mem};

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
pub unsafe extern "C" fn rust_function(buffer: *const u16, strlen: c_int) -> *const u16 {
    let src = U16String::from_ptr(buffer, strlen as usize)
        .to_string()
        .unwrap();

    println!("Hello: {}", src);

    let remove_comment = DeobfuscateEngine::remove_extra(&src).unwrap();
    let mut engine = DeobfuscateEngine::from_powershell(&remove_comment).unwrap();
    engine.deobfuscate().unwrap();

    let out = engine.lint().unwrap();
    println!("Bye: {}", out);

    let mut out = U16String::from(out).into_vec();
    out.shrink_to_fit();
    let ptr = out.as_ptr();
    mem::forget(out);

    ptr
}
