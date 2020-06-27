//! Marker attributes of [Riko](https://github.com/seamlik/riko)
//!
//! This crate contains target-neutral marker attributes. For documentation on a specific language
//! target, consult the modules in [riko_core].

#![feature(drain_filter)]

mod expand;

use proc_macro::TokenStream;
use quote::ToTokens;
use syn::ItemFn;
use syn::ItemStruct;

/// Specifies marshaling rule for a function parameter.
///
/// To specify a rule for the return type, use the `marshal` parameter of [fun](macro@fun).
///
/// This is a helper attribute for [fun](macro@fun). In order to avoid name collision, the fully-qualified
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

/// Generates language bindings for a stack-allocated struct.
///
/// The struct being marked by this attribute will be serialized using [serde] in
/// [CBOR](https://cbor.io). A version of this struct in the target code will also be generated.
#[proc_macro_attribute]
pub fn stct(_: TokenStream, item: TokenStream) -> TokenStream {
    syn::parse_macro_input!(item as ItemStruct)
        .into_token_stream()
        .into()
}
