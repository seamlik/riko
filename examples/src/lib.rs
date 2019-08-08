use serde::Deserialize;
use serde::Serialize;

#[riko::fun]
fn nothing() {
    println!("A no-in no-out function.");
}

#[riko::fun(sig = "(I32, I32) -> I32")]
fn i32(a: i32, b: i32) -> i32 {
    a + b
}

#[derive(Serialize, Deserialize)]
struct Love {
    pub target: String,
}

#[derive(Serialize, Deserialize)]
struct Work {
    pub salary: u64,
}

#[derive(Serialize, Deserialize)]
struct Life {
    pub happy: bool,
}

type Job = Work;

#[riko::fun(sig = "(Serde, Serde<Work>) -> Serde")]
fn serde(love: Love, work: Job) -> Life {
    Life {
        happy: !love.target.is_empty() && work.salary > 0,
    }
}

#[riko::fun(rename = "rename_ffi")]
fn rename() {
    println!("A function that to be renamed.");
}

#[derive(riko::Heap)]
struct NuclearReactor;
