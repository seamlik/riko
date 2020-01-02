//! Sub-optimal language binding generator.
//!
//! # Config
//!
//! In order to enable code generation, a config file called `Riko.toml` must be present alongside
//! `Cargo.toml`. This is to prevent Riko attributes in external crates from generating target code.
//! Furthermore, environment variable `RIKO_ENABLED` must set to `true` in order to reduce I/O
//! during IDE analysis.
//!
//! ## Root Section
//!
//! * `enabled`: Enable code generation. Defaults to `false`.
//! * `output`: Directory to place the generated target code. Relative to the config file. Defaults
//!   to `target/riko`.
//!
//! ## `[jni]` Section
//!
//! * `enabled`: Enable JNI bindings generation. Defaults to false.

#![feature(drain_filter)]
#![feature(proc_macro_span)]

extern crate proc_macro;

mod codegen;
mod config;
mod jni;
mod parse;

use crate::config::Config;
use parse::Fun;
use proc_macro::TokenStream;
use quote::ToTokens;
use syn::ItemFn;
use syn::ItemStruct;

/// Specifies marshaling rule for a function parameter.
///
/// To specify a rule for the return type, use the `marshal` parameter of [fun].
///
/// This is a helper attribute for [fun]. In order to avoid name collision, the fully-qualified
/// attribute name (`#[riko::marshal]`) must be used.
///
/// Marshaling rules specify how to marshal the arguments and the returned type of a function across
/// the FFI boundary. The following rules are supported:
///
/// * `Bool`
/// * `Bytes`
/// * `I8`
/// * `I32`
/// * `I64`
/// * `Iterator`
/// * `Serde`
/// * `String`
///
/// Most of them are self-explanatory. For example, `I32` means the data will be serialized as a
/// signed 32-bit integer, and will be deserialized as an `int` on the Java side.
///
/// For now, the rules are a bit limiting (no unsigned integers, for example). This is
/// because we only want to make sure they work with all target languages (Java does not have
/// unsigned integers, for example).
///
/// # Rule Inference
///
/// When no marshaling rule is specified, the macros will try to guess the rule.
///
/// Since procedural macros can only analyse a syntax tree and have no access to any type
/// information, it is impossible to always acurrately infer the rule. When the inference causes
/// compiler errors or a type alias is used, specify the rule explicitly.
///
/// When no other rules match the inference, `Serde` will be chosen by default.
///
/// # `Bytes`
///
/// For marshaling a byte array, namely `Vec<u8>`. This rule exists because it is commonly used.
///
/// # `Serde`
///
/// This rule is for custom types that support serialzation and deserialization through
/// [Serde](https://serde.rs).
///
/// User must specify the marshaling rule in the form of `Serde<fully-qualified type path>`.
/// Alternatively, one may obmit the rule and use the fully-qualified type path in the function
/// signature.
///
/// # `Iterator`
///
/// This rule is for marshaling an [Iterator]. It exists because it is a performance issue to
/// marshal a very large byte array across the FFI. Another reason is that some libraries provides
/// event subscriptions in the form of [Iterator]s instead of `Stream`s.
///
/// For now, only returning of an [Iterator] is supported.
///
/// User must specify the item type in the rule in the form of `Iterator<X>`.
///
/// Due to technical difficulties, this rule only supports marshaling an [Iterator] wrapped in a
/// [Box] or a [Result]. See `riko_runtime::iterators::IntoReturned` for explanation.
///
/// # Errors and Nullness
///
/// Unless specified, most of the rules work with their corresponding Rust types being wrapped
/// inside an [Option]. In the return position, wrapping the type in a [Result]
/// is also supported.
///
/// # References and borrowed Types
///
/// For function parameters, references are also supported. Unfortunately, the borrowed version of
/// a specific type is not supported (e.g. `&str` instead of `&String`), as that will prevent us
/// from benefiting from the compiler's type inference and will lose the support of
/// [Result] and [Option], which is of higher priority.
///
/// For returned types, only owned types are supported.
#[proc_macro_attribute]
pub fn marshal(_: TokenStream, _: TokenStream) -> TokenStream {
    unimplemented!("Must not be used on its own.")
}

/// Generates language bindings for a function.
///
/// This attribute only applies on a
/// [free-standing function](https://doc.rust-lang.org/reference/items/functions.html).
///
/// # Parameters
///
/// All parameters are optional.
///
/// * `marshal`: Marshaling rule for the return type. Use [marshal] for specifying a rule for
///   function parameters.
/// * `module`: Module path of the function.
/// * `name`: Symbol name used when exporting the item, convenient for avoiding name clashes.
///
/// # Module Path
///
/// The module path is used to determind the namespace (or a similar concept) of the function on
/// which the attribute applies. For example, a function in a crate `samples` with a module path
/// `samples::utils` will generate a Java static method in the package `samples.utils`.
///
/// Keyword `crate` at the head of the module path is supported.
///
/// When obmitted, the crate name will be read from `Cargo.toml` and the module path will be
/// guessed from the file path to the source code. In case of a functions in a sub-module inside a
/// source file, this parameter must be set explicitly.
#[proc_macro_attribute]
pub fn fun(attr: TokenStream, item: TokenStream) -> TokenStream {
    let config = config::current();
    if !config.enabled {
        return item;
    }

    let mut subject = syn::parse_macro_input!(item as ItemFn);
    let mut args: Fun = syn::parse_macro_input!(attr as Fun);
    args.expand_all_fields(&subject.sig, &config).unwrap();

    let mut generated = TokenStream::new();
    if config.jni.enabled {
        generated.extend(jni::Bindgen::new(&config).fun(&mut subject, &args));
    }

    let mut result: TokenStream = subject.into_token_stream().into();
    result.extend(generated);
    result
}

/// Generates language bindings for a Rust type allocated on the heap.
///
/// Deriving this trait allows code on the target side to construct an object and put it on the
/// heap. This is achieved by creating a global object pool dedicated to the type deriving the
/// trait.
#[proc_macro_derive(Heaped)]
pub fn derive_heap(item: TokenStream) -> TokenStream {
    let config = config::current();
    if !config.enabled {
        return TokenStream::new();
    }

    let item_struct = syn::parse_macro_input!(item as ItemStruct);
    jni::Bindgen::new(&config).heaped(&item_struct)
}

/// Language binding generator.
trait Bindgen<'cfg> {
    fn new(config: &'cfg Config) -> Self;
    fn config(&self) -> &'cfg Config;
    fn heaped(&self, item: &ItemStruct) -> TokenStream;
    fn fun(&self, item: &mut ItemFn, args: &Fun) -> TokenStream;
}
