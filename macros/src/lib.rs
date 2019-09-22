//! Sub-optimal language binding generator.

#![feature(proc_macro_span)]

extern crate proc_macro;

mod config;
mod jni;
mod parse;

use crate::config::Config;
use parse::Fun;
use proc_macro::TokenStream;
use std::convert::TryInto;
use syn::AttributeArgs;
use syn::ItemFn;
use syn::ItemStruct;

/// Generates language bindings for a function.
///
/// This attribute only applies on a
/// [free-standing function](https://doc.rust-lang.org/reference/items/functions.html).
///
/// # Parameters
///
/// * `sig`: Marshaling signature. Defaults to an empty signature, i.e. no input or output. See
///   "Marshaling Rules" below.
/// * `name`: Symbol name used when exporting the item, convenient for avoiding name clashes.
///
/// # Marshaling Rules
///
/// Since procedural macros can only analyse a syntax tree and have no access to any type
/// information, it is impossible to acurrately determine what marshaling rules to use for each
/// parameters and return type of a function. Therefore, user must specify the marshaling rules
/// manually.
///
/// The syntax of a marshaling signature is the same as the signature of a closure, e.g.
/// `(A, B) -> C`. Each component in the signature must be one of the "marshaling rules".
///
/// The following marshaling rules are supported:
///
/// * Bytes
/// * I8
/// * I32
/// * I64
/// * Iterator
/// * Serde
/// * String
///
/// ## Primitives
///
/// All the rules except `Iterator` and `Serde` are for marshaling primitive types or built-in
/// standard types.
///
/// These rules specify how the data is copied and sent between the FFI boundary. For example, `I32`
/// means the data will be serialized as a 32-bit integer, and will be deserialized as an `int` on
/// the Java side.
///
/// For now, the primitive rules are a bit limiting (no unsigned integers, for example). This is
/// because we only want to make sure they work with all target languages (Java does not have
/// unsigned integers, for example).
///
/// ## `Serde`
///
/// This rule is for custom types that support serialzation and deserialization through
/// [Serde](https://serde.rs).
///
/// In the return position, user must specify the data type in the form of `Serde<X>`. The type can
/// be anything that is resolvable in the current scope. For example:
///
/// ```rust
/// struct Love;
/// type Home = Love;
///
/// #[riko::fun(sig = "(Serde) -> Serde<Love>")]
/// fn search(love: Love) -> Home {
///     love
/// }
/// ```
///
/// ## `Iterator`
///
/// This rule is for marshaling an iterator. It exists because it is a performance issue to marshal
/// a very large byte array across the FFI. Another reason is that some libraries provides event
/// APIs in the form of iterators instead of `Stream`s.
///
/// User must specify the item type in the rule in the form of `Iterator<X>`.
///
/// Due to technical difficulties, this rule only supports marshaling a `MarshalingIterator` and
/// only in the return posigion.
///
/// ## Errors and Nullness
///
/// Unless specified, most of the rules work with their corresponding Rust types being wrapped
/// inside an [Option]. In the return position, wrapping the type in a [Result](std::result::Result)
/// is also supported. For example:
///
/// ```rust
/// use std::io::Result;
///
/// #[riko::fun(sig = "(String, String) -> String")]
/// fn fun(a: String, b: Option<String>) -> Result<Option<String>> {
///     Ok(Some("Love".to_string()))
/// }
/// ```
#[proc_macro_attribute]
pub fn fun(attr: TokenStream, mut item: TokenStream) -> TokenStream {
    let config = config::current();
    if !config.enabled {
        return item;
    }

    let args: Fun = syn::parse_macro_input!(attr as AttributeArgs)
        .try_into()
        .expect("Failed to parse attribute arguments.");

    let subject = if let Ok(item_fn) = syn::parse::<ItemFn>(item.clone()) {
        FunSubject::Function(item_fn)
    } else {
        panic!("Applied to an unsupported language item.")
    };

    let mut generated = Vec::<TokenStream>::new();
    if config.jni.enabled {
        generated.push(jni::Bindgen::new(&config).gen_fun(&subject, &args));
    }

    item.extend(generated);
    item
}

/// Generates language bindings for a Rust type allocated in the heap.
///
/// Deriving this trait allows code on the target side to construct an object and put it on the
/// heap. This is achieved by creating a global object pool dedicated to the type deriving the
/// trait.
#[proc_macro_derive(Heap)]
pub fn derive_heap(item: TokenStream) -> TokenStream {
    let config = config::current();
    if !config.enabled {
        return TokenStream::new();
    }

    let item_struct = syn::parse_macro_input!(item as ItemStruct);
    jni::Bindgen::new(&config).gen_heap(&item_struct).into()
}

/// Language binding generator.
trait Bindgen<'cfg> {
    fn new(config: &'cfg Config) -> Self;
    fn config(&self) -> &'cfg Config;
    fn gen_heap(&self, item: &ItemStruct) -> TokenStream;
    fn gen_fun(&self, item: &FunSubject, args: &Fun) -> TokenStream;
}

/// Item on which a `#[fun]` can be applied.
enum FunSubject {
    Function(ItemFn),
}
