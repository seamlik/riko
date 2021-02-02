//! Marker attributes of [Riko](https://github.com/seamlik/riko)
//!
//! TODO: Document the attribute parameters

#![feature(drain_filter)]

mod expand;

use proc_macro::TokenStream;
use quote::ToTokens;
use syn::ItemFn;

/// Specifies marshaling rule for a function parameter.
///
/// To specify a rule for the return type, use the `marshal` parameter of [fun](macro@fun).
///
/// This is a helper attribute for [fun](macro@fun). In order to avoid name collision, the fully-qualified
/// attribute name (`#[riko::marshal]`) must be used.
///
/// # See
///
/// * [MarshalingRule](riko_core::ir::MarshalingRule)
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

/// Ignores the marked item.
///
/// Riko's source code parser will ignore any item marked by this attribute, as well as any child-items.
///
/// Usually used to mark the module containing generated code or bridge code. It not only speeds up the
/// parsing, but also avoids file-not-found errors.
///
/// # Example
///
/// ```ignore
/// #[path = "../../target/riko/riko_sample.rs"]
/// #[riko::ignore]
/// mod bridge;
/// ```
#[proc_macro_attribute]
pub fn ignore(_: TokenStream, item: TokenStream) -> TokenStream {
    item
}
