Riko (WiP)
==========

> Sub-optimal language binding generator

This project aims to ease (or eliminate) the task of writing tedious wrapper code for a Rust crate in order for it to be used by other languages. All you have to do is mark the Rust code with the macros in this crate, and `cargo riko` will generate the wrapper code on both the Rust side and the target side.

For all supported language targets, consult the modules in [riko_core](https://docs.rs/riko_core).

Contrary to the general atmosphere of Rust's ecosystem, the generated code is not zero-cost abstraction (hence the name "sub-optimal"). Overhead is imposed at various situations:

* Marshaling data across FFI boundary involves encoding and decoding [CBOR](https://cbor.io).
* Using an object (as in object-oriented programming) requires allocating memory in a global pool protected by locks.
* Data is always copied between FFI boundary because pointers or references are not supported.

Usage
-----

Consult the `sample-*` projects on the details.

1. Mark the items to be exported with `riko::*` attributes.
2. Specify language targets in the package metadata. At least 1 target must be specified, otherwise `cargo riko` won't do anything.
3. Add dependencies necessary to the generated code.
4. Add a `cdylib` crate type.
5. Run `cargo riko`.

The generated code will be placed at directory `target/riko`.

Install
-------

To use Riko's attributes, add `riko` to package's dependencies.

To install the tools generating wrapper code, run `cargo install riko`. The installed binaries are:

* `cargo-riko`: For use inside a Cargo project
* `riko` (WiP): For use without the Cargo infrasturcture

Dependencies for Generated Code
-------------------------------

Some third party crates are used by the generated code.

The mandatory ones are:

* Runtime support: [riko_runtime](https://crates.io/crates/riko_runtime)

The optional ones are:

* Interfacing with JNI: [jni](https://crates.io/crates/jni)
* Marshaling byte arrays: [serde_bytes](https://crates.io/crates/serde_bytes)
