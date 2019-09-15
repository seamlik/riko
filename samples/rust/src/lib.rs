mod iterators;
mod serde;

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

#[derive(riko::Heap)]
struct NuclearReactor;
