//! Sub-optimal language binding generator.

#![feature(proc_macro_span)]

extern crate proc_macro;

mod config;
mod jni;
mod parse;

use parse::Fun;
use proc_macro::TokenStream;
use quote::ToTokens;
use std::convert::TryInto;
use syn::AttributeArgs;
use syn::ItemFn;

/// Generates language bindings for a free-standing function.
///
/// This attribute only applies on a
/// [function item](https://doc.rust-lang.org/reference/items/functions.html).
///
/// # Parameters
///
/// * `sig`: Marshaling signature. Defaults to an empty signature, i.e. no input or output. See
///   "Marshaling Rules" below.
/// * `name`: Symbol name used when exporting the item, convenient for avoiding name clashes.
///
/// Since procedural macros can only analyse a syntax tree and has no access to any type
/// information, it is not possible to acurrately detect what marshaling rules to use for each
/// parameters and return type of a function. Therefore, user must specify the marshaling rules
/// manually.
///
/// The syntax of a marshaling signature is the same as the signature of a closure trait, e.g.
/// `(A, B) -> C`. Each type must be one of the `MarshalingRule`s.
#[proc_macro_attribute]
pub fn fun(attr: TokenStream, item: TokenStream) -> TokenStream {
    let config = config::current();
    if config.enabled == false {
        return item;
    }

    let function = syn::parse_macro_input!(item as ItemFn);
    let args: Fun = syn::parse_macro_input!(attr as AttributeArgs)
        .try_into()
        .expect("Failed to parse attribute arguments.");

    function.into_token_stream().into()
}

#[proc_macro_derive(Heap)]
#[allow(non_snake_case)]
pub fn derive_Heap(item: TokenStream) -> TokenStream {
    let config = config::current();
    if config.enabled == false {
        return TokenStream::new();
    }

    TokenStream::new()
}
