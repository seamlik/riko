//! Sub-optimal language binding generator.
//!
//! # Config
//!
//! In order to enable code generation, at least 1 target must be specified in the package metadata.

#![feature(drain_filter)]

extern crate proc_macro;

mod expand;

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
#[proc_macro_attribute]
pub fn marshal(_: TokenStream, _: TokenStream) -> TokenStream {
    unimplemented!("Must not be used on its own")
}

/// Generates language bindings for a function.
///
/// This attribute only applies on a
/// [free-standing function](https://doc.rust-lang.org/reference/items/functions.html).
///
/// See [Fun](riko_core::parse::Fun) for details on the attributes.
#[proc_macro_attribute]
pub fn fun(_: TokenStream, item: TokenStream) -> TokenStream {
    let mut subject = syn::parse_macro_input!(item as ItemFn);
    expand::remove_marshal_attrs(&mut subject);
    subject.into_token_stream().into()
}

/// Generates language bindings for a Rust type allocated on the heap.
///
/// Deriving this trait allows code on the target side to construct an object and put it on the
/// heap. This is achieved by creating a global object pool dedicated to the type deriving the
/// trait.
#[proc_macro_derive(Heaped)]
pub fn derive_heap(item: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item as ItemStruct);
    expand::heaped(&item).into()
}
