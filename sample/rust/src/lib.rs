#![feature(proc_macro_hygiene)]

#[path = "../../../target/riko/riko_sample.rs"]
#[riko::ignore]
mod bridge;

mod object;
mod structs;

use futures_channel::oneshot::Canceled;
use serde_bytes::ByteBuf;
use std::time::Duration;

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

#[riko::fun]
async fn future() -> Result<String, Canceled> {
    let (sender, receiver) = futures_channel::oneshot::channel::<String>();
    std::thread::spawn(move || sender.send("love".into()));
    receiver.await
}

#[riko::fun]
async fn future_slow() {
    futures_timer::Delay::new(Duration::from_secs(10)).await
}
