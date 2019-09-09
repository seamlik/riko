mod serde;

#[riko::fun]
fn nothing() {
    println!("A no-in no-out function.");
}

#[riko::fun(sig = "(I32, I32) -> I32")]
fn i32(a: i32, b: i32) -> i32 {
    a + b
}

#[riko::fun(rename = "rename_ffi")]
fn rename() {
    println!("A function that to be renamed.");
}

#[derive(riko::Heap)]
struct NuclearReactor;
