mod heap;
mod serde;

use std::result::Result;

#[riko::fun]
fn nothing() {
    println!("A no-in no-out function.");
}

#[riko::fun]
fn _i32(a: i32, b: i32) -> i32 {
    a + b
}

#[riko::fun(name = "rename")]
fn rename_ffi() {
    println!("A function to be renamed.");
}

#[riko::fun(marshal = "I32")]
fn result_option(
    #[riko::marshal(I32)] a: Option<i32>,
    #[riko::marshal(I32)] b: Option<i32>,
) -> Result<Option<i32>, std::fmt::Error> {
    match (a, b) {
        (Some(a_value), Some(b_value)) => Ok(Some(a_value + b_value)),
        (None, None) => Err(std::fmt::Error),
        _ => Ok(None),
    }
}

#[riko::fun]
fn string(a: String, b: String) -> String {
    a + &b
}

#[riko::fun]
fn bytes(mut x: Vec<u8>, y: Vec<u8>) -> Vec<u8> {
    x.extend(y);
    x
}

fn _bool(x: bool, y: bool) -> bool {
    x && y
}

#[riko::fun(marshal = "Iterator<String>")]
fn iterator(a: String, b: String) -> Box<dyn Iterator<Item = String> + Send + 'static> {
    Box::new(vec![a, b].into_iter())
}

#[riko::fun(marshal = "Iterator<String>")]
fn iterator_fallible(
    item: String,
    fails: bool,
) -> Result<impl Iterator<Item = String> + Send + 'static, std::fmt::Error> {
    if fails {
        Err(std::fmt::Error)
    } else {
        Ok(std::iter::once(item))
    }
}
