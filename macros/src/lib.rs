//! Sub-optimal language binding generator.

#![feature(proc_macro_span)]

extern crate proc_macro;

mod config;
mod jni;
mod parse;

use crate::config::Config;
use parse::Fun;
use proc_macro::TokenStream;
use syn::ItemFn;
use syn::ItemStruct;
use syn::Signature;

const ERROR_CONFIG: &str = "Failed to read the config.";

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
/// # Marshaling Rules and Signatures
///
/// A marshaling signature is necessary in order to specify how to marshal the parameters and the
/// returned data of a function across the FFI boundary. The syntax of a marshaling signature is the
/// same as the signature of a closure, e.g. `(A, B) -> C`. Each of its component is a marshaling
/// rule. The following rules are supported:
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
/// 32-bit integer, and will be deserialized as an `int` on the Java side.
///
/// For now, the rules are a bit limiting (no unsigned integers, for example). This is
/// because we only want to make sure they work with all target languages (Java does not have
/// unsigned integers, for example).
///
/// ## Rule Inference
///
/// Most of time, the macros can guess what rules should be applied on a function if a built-in type
/// is used. When using the `Serde` rule, a fully-qualified type path must be specify in the
/// function so that the macro can generate the correct target code.
///
/// To let the macro infer a rule in a marshaling signature, use `_` just like the type inferece.
/// To infer the whole signature, simply obmit the entire `sig` parameter.
///
/// Since procedural macros can only analyse a syntax tree and have no access to any type
/// information, it is impossible to always acurrately infer the rule. When the inference causes
/// compiler errors or a type alias is used, specify the rule explicitly.
///
/// ## `Bytes`
///
/// For marshaling a byte array, namely `Vec<u8>`. This rule exists because it is commonly used.
///
/// ## `Serde`
///
/// This rule is for custom types that support serialzation and deserialization through
/// [Serde](https://serde.rs).
///
/// User must specify the data type in the form of `Serde<X>`.
///
/// ## `Iterator`
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
/// ## Errors and Nullness
///
/// Unless specified, most of the rules work with their corresponding Rust types being wrapped
/// inside an [Option]. In the return position, wrapping the type in a [Result]
/// is also supported.
///
/// ## References and borrowed Types
///
/// For function parameters, references are also supported. Unfortunately, the borrowed version of
/// a specific type is not supported (e.g. `&str` instead of `&String`), as that will prevent us
/// from benefiting from the compiler's type inference and will lose the support of
/// [Result] and [Option], which is of higher priority.
///
/// For returned types, only owned types are supported.
#[proc_macro_attribute]
pub fn fun(attr: TokenStream, mut item: TokenStream) -> TokenStream {
    let config = config::current().expect(ERROR_CONFIG);
    if !config.enabled {
        return item;
    }

    let subject = if let Ok(item_fn) = syn::parse::<ItemFn>(item.clone()) {
        FunSubject::Function(item_fn)
    } else {
        panic!("Applied to an unsupported language item.")
    };

    let mut args: Fun = syn::parse_macro_input!(attr as Fun);
    args.expand_all_fields(subject.signature(), &config)
        .unwrap();

    let mut generated = Vec::<TokenStream>::new();
    if config.jni.enabled {
        generated.push(jni::Bindgen::new(&config).fun(&subject, &args));
    }

    item.extend(generated);
    item
}

/// Generates language bindings for a Rust type allocated on the heap.
///
/// Deriving this trait allows code on the target side to construct an object and put it on the
/// heap. This is achieved by creating a global object pool dedicated to the type deriving the
/// trait.
#[proc_macro_derive(Heaped)]
pub fn derive_heap(item: TokenStream) -> TokenStream {
    let config = config::current().expect(ERROR_CONFIG);
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
    fn fun(&self, item: &FunSubject, args: &Fun) -> TokenStream;
}

/// Item on which a `#[fun]` can be applied.
enum FunSubject {
    Function(ItemFn),
}

impl FunSubject {
    pub fn signature(&self) -> &Signature {
        match self {
            Self::Function(inner) => &inner.sig,
        }
    }
}
