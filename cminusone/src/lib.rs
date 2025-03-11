use std::{ffi::c_int, mem, ptr::null};

use minusone::engine::DeobfuscateEngine;
use widestring::U16String;

#[no_mangle]
pub extern "C" fn minusone_deobfuscate_powershell(buffer: *const u16, strlen: c_int) -> *const u16 {
    unsafe {
        let src = U16String::from_ptr(buffer, strlen as usize)
            .to_string()
            .unwrap();

        if let Ok(remove_comment) = DeobfuscateEngine::remove_extra(&src) {
            if let Ok(mut engine) = DeobfuscateEngine::from_powershell(&remove_comment) {
                if engine.deobfuscate().is_ok() {
                    let out = engine.lint().unwrap();
                    let mut out = U16String::from(out).into_vec();
                    out.push(0); // Add null byte

                    out.shrink_to_fit();
                    let ptr = out.as_ptr();
                    mem::forget(out);

                    return ptr;
                }
            }
        }
    }

    null()
}
