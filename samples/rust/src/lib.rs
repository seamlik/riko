mod heap;
mod serde;

use std::result::Result;

#[riko::fun]
fn nothing() {
    println!("A no-in no-out function.");
}

#[riko::fun(sig = "(I32, I32) -> I32")]
fn _i32(a: i32, b: i32) -> i32 {
    a + b
}

#[riko::fun(name = "rename")]
fn rename_ffi() {
    println!("A function to be renamed.");
}

#[riko::fun(sig = "(I32, I32) -> I32")]
fn result_option(a: Option<i32>, b: Option<i32>) -> Result<Option<i32>, std::fmt::Error> {
    match (a, b) {
        (Some(a_value), Some(b_value)) => Ok(Some(a_value + b_value)),
        (None, None) => Err(std::fmt::Error),
        _ => Ok(None),
    }
}
