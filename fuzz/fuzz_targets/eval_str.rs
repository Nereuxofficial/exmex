#![no_main]
use libfuzzer_sys::fuzz_target;
extern crate exmex;

use exmex::eval_str;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data){
        let _ = eval_str(s);
    }
});
