#[path = "../../target/riko/jni/bridge.rs"]
mod bridge;

mod heaped;
//mod serde;

use serde_bytes::ByteBuf;

#[riko::fun]
fn nothing() {}

#[riko::fun]
fn _i32(a: i32, b: i32) -> i32 {
    a + b
}

#[riko::fun(name = "rename")]
fn rename_ffi() {}

#[riko::fun]
fn result_option(a: Option<i32>, b: Option<i32>) -> Result<Option<i32>, std::fmt::Error> {
    match (a, b) {
        (Some(a_value), Some(b_value)) => Ok(Some(a_value + b_value)),
        (None, None) => Err(std::fmt::Error),
        _ => Ok(None),
    }
}

type NewType = i32;
#[riko::fun(marshal = "I32")]
fn marshal(#[riko::marshal = "I32"] a: NewType) -> NewType {
    -a
}

#[riko::fun]
fn string(a: String, b: String) -> String {
    a + &b
}

#[riko::fun]
fn bytes(mut x: ByteBuf, y: ByteBuf) -> ByteBuf {
    x.extend(y);
    x
}

#[riko::fun]
fn _bool(x: bool, y: bool) -> bool {
    x && y
}

pub fn ignored() {
    panic!("This function is not exported")
}
