//! Sub-optimal language binding generator.
//!
//! # Config
//!
//! In order to enable code generation, at least 1 target must be specified in the package metadata.

extern crate proc_macro;

mod jni;

use jni::JniExpander;
use proc_macro::TokenStream;
use quote::ToTokens;
use riko_core::parse::Fun;
use syn::ItemFn;
use syn::ItemStruct;

/// Specifies marshaling rule for a function parameter.
///
/// To specify a rule for the return type, use the `marshal` parameter of [fun].
///
/// This is a helper attribute for [fun]. In order to avoid name collision, the fully-qualified
/// attribute name (`#[riko::marshal]`) must be used.
#[proc_macro_attribute]
pub fn marshal(_: TokenStream, _: TokenStream) -> TokenStream {
    unimplemented!("Must not be used on its own")
}

/// Generates language bindings for a function.
///
/// This attribute only applies on a
/// [free-standing function](https://doc.rust-lang.org/reference/items/functions.html).
///
/// See [Fun] for details on the attributes.
#[proc_macro_attribute]
pub fn fun(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut subject = syn::parse_macro_input!(item as ItemFn);
    let mut args: Fun = syn::parse_macro_input!(attr as Fun);
    if let Err(err) = args.expand_all_fields(&subject.sig) {
        return err.to_compile_error().into();
    }

    let mut result: TokenStream = JniExpander.fun(&mut subject, &args).into();
    result.extend::<TokenStream>(subject.into_token_stream().into());
    result
}

/// Generates language bindings for a Rust type allocated on the heap.
///
/// Deriving this trait allows code on the target side to construct an object and put it on the
/// heap. This is achieved by creating a global object pool dedicated to the type deriving the
/// trait.
#[proc_macro_derive(Heaped)]
pub fn derive_heap(item: TokenStream) -> TokenStream {
    let item_struct = syn::parse_macro_input!(item as ItemStruct);
    JniExpander.heaped(&item_struct).into()
}

/// Language binding generator.
///
/// All macro arguments must be fully expanded.
trait MacroExpander {
    /// Generates Rust code wrapping a `Heaped`.
    fn heaped(&self, item: &ItemStruct) -> proc_macro2::TokenStream;

    /// Generates Rust code wrapping a function.
    ///
    /// After expansion, all `#[riko::marshal]` attributes must be removed from `item`.
    fn fun(&self, item: &mut ItemFn, args: &Fun) -> proc_macro2::TokenStream;
}
