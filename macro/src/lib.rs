//! Marker attributes of [Riko](https://github.com/seamlik/riko)

#![feature(drain_filter)]

extern crate proc_macro;

mod expand;

use proc_macro::TokenStream;
use quote::ToTokens;
use syn::ItemFn;

/// Specifies marshaling rule for a function parameter.
///
/// To specify a rule for the return type, use the `marshal` parameter of [fun].
///
/// This is a helper attribute for [fun]. In order to avoid name collision, the fully-qualified
/// attribute name (`#[riko::marshal]`) must be used.
///
/// # See
/// * [MarshalingRule](riko_core::parse::MarshalingRule)
#[proc_macro_attribute]
pub fn marshal(_: TokenStream, _: TokenStream) -> TokenStream {
    unimplemented!("Must not be used on its own")
}

/// Generates language bindings for a function.
///
/// This attribute only applies on a
/// [free-standing function](https://doc.rust-lang.org/reference/items/functions.html).
///
/// # See
///
/// * [Fun](riko_core::parse::Fun)
#[proc_macro_attribute]
pub fn fun(_: TokenStream, item: TokenStream) -> TokenStream {
    let mut subject = syn::parse_macro_input!(item as ItemFn);
    expand::remove_marshal_attrs(&mut subject);
    subject.into_token_stream().into()
}
