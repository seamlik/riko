//! Java support using JNI.
//!
//! # Dependencies for the generated code
//!
//! ## Rust
//!
//! * [jni](https://crates.io/crates/jni)
//!
//! ## Java
//!
//! * `riko-runtime-jni`

use crate::ir::Crate;
use crate::ir::Function;
use crate::ir::MarshalingRule;
use crate::ir::Module;
use crate::TargetCodeWriter;
use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::quote;
use quote::ToTokens;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::path::PathBuf;
use syn::Ident;
use syn::ItemFn;

const CLASS_FOR_MODULE: &str = "Module";
const PREFIX_FOR_NATIVE: &str = "__riko_";
const NULLABLE_ATTRIBUTE: &str = "org.checkerframework.checker.nullness.qual.Nullable";

/// Writes JNI bindings.
pub struct JniWriter;

impl TargetCodeWriter for JniWriter {
    fn write_target_all(&self, root: &Crate) -> HashMap<PathBuf, String> {
        root.modules
            .iter()
            .map(|module| {
                let mut file_path = PathBuf::new();
                file_path.push(&root.name);
                file_path.extend(module.path.iter());
                file_path.push(format!("{}.java", CLASS_FOR_MODULE));

                let target_code = self.write_target_module(module, root);
                (file_path, target_code)
            })
            .collect()
    }

    fn write_target_function(&self, function: &Function, _: &Module, crate_: &Crate) -> String {
        let return_type_public = target_type_public(
            function.output.rule,
            &function.output.unwrapped_type.0,
            &crate_.name,
        );

        let return_type_local = match function.output.rule {
            MarshalingRule::Object => "java.lang.Integer".into(),
            MarshalingRule::Unit => "java.lang.Void".into(),
            MarshalingRule::Struct => {
                target_type_obj(&function.output.unwrapped_type.0, &crate_.name).join(".")
            }
            _ => target_type_primitive(function.output.rule).join("."),
        };

        let return_block = match function.output.rule {
            MarshalingRule::Unit => "",
            MarshalingRule::Object => "return result == null ? null : new riko.Object(result);",
            _ => "return result;",
        }
        .to_string();

        let args = function
            .inputs
            .iter()
            .enumerate()
            .map(|(idx, _)| format!("riko.Marshaler.encode(arg_{})", idx))
            .join(", ");
        let params_public = function
            .inputs
            .iter()
            .enumerate()
            .map(|(idx, input)| {
                format!(
                    "final {} arg_{}",
                    target_type_public(input.rule, &input.unwrapped_type.0, &crate_.name),
                    idx
                )
            })
            .join(", ");
        let params_bridge = function
            .inputs
            .iter()
            .enumerate()
            .map(|(idx, _)| format!("byte[] arg_{}", idx))
            .join(", ");

        format!(
            r#"
              private static native byte[] __riko_{name}( {params_bridge} );
              public static {return_type_public} {name}( {params_public} ) {{
                final byte[] returned = __riko_{name}( {args} );
                final {return_type_local} result = riko
                    .Marshaler
                    .decode(returned)
                    .unwrap({return_type_local}.class);
                {return_block}
              }}
            "#,
            args = args,
            name = &function.pubname,
            params_bridge = params_bridge,
            params_public = params_public,
            return_block = return_block,
            return_type_local = return_type_local,
            return_type_public = return_type_public,
        )
    }

    fn write_bridge_function(&self, function: &Function, module: &Module, root: &Crate) -> ItemFn {
        let output_type = function.output.marshaled_type();

        // Name of the generated function
        let full_public_name = full_function_name(&function.name, &module.path);
        let mangled_name = mangle_function_name(&function.pubname, &module.path, &root.name);

        // Parameters of the generated function
        let mut result_params = Vec::<TokenStream>::new();
        result_params.push(quote! { _env: ::jni::JNIEnv });
        result_params.push(quote! { _class: ::jni::objects::JClass });

        // Function arguments placed at the invocation of the original function
        let mut result_args = Vec::<TokenStream>::new();

        for (index, input) in function.inputs.iter().enumerate() {
            // Parameters
            let param_name = quote::format_ident!("arg_{}_jni", index);
            result_params.push(quote! { #param_name : ::jni::sys::jbyteArray });

            // Arguments
            let arg_raw = quote! {
                ::riko_runtime::Marshal::from_jni(&_env, #param_name)
            };
            let arg = if input.borrow {
                quote! { &(#arg_raw) }
            } else {
                arg_raw
            };
            result_args.push(arg);
        }

        // Shelving heap-allocated objects
        let shelve = if function.output.rule == MarshalingRule::Object {
            quote! {
                let result = ::riko_runtime::object::Shelve::shelve(result);
            }
        } else {
            Default::default()
        };

        // Inherited `#[cfg]`
        let cfg = function.collect_cfg(module, root);

        let result: ItemFn = syn::parse_quote! {
            #(#cfg)*
            #[no_mangle]
            #[allow(clippy::useless_conversion)]
            #[allow(clippy::let_unit_value)]
            #[allow(clippy::unit_arg)]
            pub extern "C" fn #mangled_name(#(#result_params),*) -> ::jni::sys::jbyteArray {
                let result = #full_public_name(
                    #(#result_args),*
                );
                #shelve
                let result: ::riko_runtime::returned::Returned<#output_type> = result.into();
                ::riko_runtime::Marshal::to_jni(&result, &_env)
            }
        };
        result
    }

    fn write_target_module(&self, module: &Module, root: &Crate) -> String {
        let body = module
            .functions
            .iter()
            .map(|function| self.write_target_function(function, module, root))
            .join("\n");
        let result_package = std::iter::once(&root.name)
            .chain(module.path.iter())
            .join(".");

        format!(
            r#"
                package {package};

                public final class {class} {{

                    private {class}() {{}}

                    {body}
                }}
            "#,
            body = body,
            class = CLASS_FOR_MODULE,
            package = &result_package,
        )
    }
}

fn target_type_obj(unwrapped_type: &syn::Path, crate_name: &str) -> Vec<String> {
    let mut result = unwrapped_type
        .segments
        .iter()
        .map(|i| i.to_token_stream().to_string())
        .collect::<Vec<_>>();
    if let Some(first) = result.first_mut() {
        if first == "crate" {
            *first = crate_name.into();
        }
    }
    result
}

fn target_type_primitive(rule: MarshalingRule) -> Vec<&'static str> {
    let mut result = vec!["java", "lang"];
    match rule {
        MarshalingRule::Bool => result.push("Boolean"),
        MarshalingRule::Bytes => return vec!["byte[]"],
        MarshalingRule::I8 => result.push("Byte"),
        MarshalingRule::I32 => result.push("Integer"),
        MarshalingRule::I64 => result.push("Long"),
        MarshalingRule::String => result.push("String"),
        MarshalingRule::Object | MarshalingRule::Struct => {
            unimplemented!("Use `target_type_obj()`")
        }
        MarshalingRule::Unit => unimplemented!("`void` must be treated differently"),
    }
    result
}

fn target_type_public(
    rule: MarshalingRule,
    unwrapped_type: &syn::Path,
    crate_name: &str,
) -> String {
    let mut raw_type: VecDeque<_> = match rule {
        MarshalingRule::Bytes => {
            return "byte @ org.checkerframework.checker.nullness.qual.Nullable []".into()
        }
        MarshalingRule::Unit => return "void".into(),
        MarshalingRule::Object => {
            return "riko. @ org.checkerframework.checker.nullness.qual.Nullable Object".into()
        }
        MarshalingRule::Struct => target_type_obj(unwrapped_type, crate_name),
        _ => target_type_primitive(rule).into_iter().map_into().collect(),
    }
    .into();
    let name = raw_type.pop_back().unwrap_or_default();
    let prefix = raw_type;

    let mut result = Vec::<String>::default();
    if !prefix.is_empty() {
        result.push(prefix.iter().map(|n| n.to_string() + ".").join(""));
    }
    result.push("@".into());
    result.push(NULLABLE_ATTRIBUTE.into());
    result.push(name);
    result.join(" ")
}

fn mangle_function_name(name: &str, module: &[String], crate_: &str) -> Ident {
    let prefix = std::iter::once(crate_)
        .chain(module.iter().map(String::as_str))
        .map(|ident| ident.replace("_", "_1"))
        .join("_");
    quote::format_ident!(
        "Java_{}_Module_{}{}",
        &prefix,
        &PREFIX_FOR_NATIVE.replace("_", "_1"),
        &name.replace("_", "_1")
    )
}

fn full_function_name(name: &str, module: &[String]) -> syn::Path {
    let ident = quote::format_ident!("{}", name);
    let prefix = module
        .iter()
        .map(|x| quote::format_ident!("{}", x))
        .chain(std::iter::once(ident))
        .collect::<Vec<_>>();
    syn::parse_quote! {
        crate :: #(#prefix)::*
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn full_function_name() {
        let none = super::full_function_name("function", &[])
            .into_token_stream()
            .to_string();
        assert_eq!("crate :: function", none);
        let some_1 = super::full_function_name("function", &["util".into()])
            .into_token_stream()
            .to_string();
        assert_eq!("crate :: util :: function", some_1);
        let some_2 = super::full_function_name("function", &["util".into(), "unix".into()])
            .into_token_stream()
            .to_string();
        assert_eq!("crate :: util :: unix :: function", some_2)
    }

    #[test]
    fn mangle_function_name() {
        let none = super::mangle_function_name("function", &[], "riko_sample").to_string();
        assert_eq!("Java_riko_1sample_Module__1_1riko_1function", none);
        let some_1 =
            super::mangle_function_name("function", &["util".into()], "riko_sample").to_string();
        assert_eq!("Java_riko_1sample_util_Module__1_1riko_1function", some_1);
        let some_2 =
            super::mangle_function_name("function", &["util".into(), "unix".into()], "riko_sample")
                .to_string();
        assert_eq!(
            "Java_riko_1sample_util_unix_Module__1_1riko_1function",
            some_2
        )
    }

    #[test]
    fn module() {
        let ir = &crate::ir::sample::empty_module();
        let expected = r#"
            package riko_sample.example;

            public final class Module {
                private Module() {}
            }
        "#;
        let actual = JniWriter.write_target_module(&ir.modules[0], &ir);
        assert_eq!(
            crate::normalize_source_code(expected),
            crate::normalize_source_code(&actual),
        );
    }

    #[test]
    fn simple_function() {
        let ir = crate::ir::sample::simple_function();

        let expected = r#"
            private static native byte[] __riko_function(
                byte[] arg_0,
                byte[] arg_1
            );
            public static java.lang. @ org.checkerframework.checker.nullness.qual.Nullable String function(
                final java.lang. @ org.checkerframework.checker.nullness.qual.Nullable Integer arg_0,
                final java.lang. @ org.checkerframework.checker.nullness.qual.Nullable Long arg_1
            ) {
                final byte[] returned = __riko_function(
                    riko.Marshaler.encode(arg_0),
                    riko.Marshaler.encode(arg_1)
                );
                final java.lang.String result = riko
                  .Marshaler
                  .decode(returned)
                  .unwrap(java.lang.String.class);
                return result;
            }
        "#;
        let actual =
            JniWriter.write_target_function(&ir.modules[0].functions[0], &ir.modules[0], &ir);
        assert_eq!(
            crate::normalize_source_code(expected),
            crate::normalize_source_code(&actual),
        );

        let expected = quote! {
            #[no_mangle]
            #[allow(clippy::useless_conversion)]
            #[allow(clippy::let_unit_value)]
            #[allow(clippy::unit_arg)]
            pub extern "C" fn Java_riko_1sample_example_Module__1_1riko_1function(
                _env: ::jni::JNIEnv,
                _class: ::jni::objects::JClass,
                arg_0_jni: ::jni::sys::jbyteArray,
                arg_1_jni: ::jni::sys::jbyteArray
            ) -> ::jni::sys::jbyteArray {
                let result = crate::example::function(
                    &(::riko_runtime::Marshal::from_jni(&_env, arg_0_jni)),
                    ::riko_runtime::Marshal::from_jni(&_env, arg_1_jni)
                );
                let result: ::riko_runtime::returned::Returned<::std::string::String> = result.into();
                ::riko_runtime::Marshal::to_jni(&result, &_env)
            }
        }
        .to_string();
        let actual = JniWriter
            .write_bridge_function(&ir.modules[0].functions[0], &ir.modules[0], &ir)
            .into_token_stream()
            .to_string();
        assert_eq!(expected, actual);
    }

    #[test]
    fn returning_object() {
        let ir = crate::ir::sample::returning_object();

        let expected = r#"
            private static native byte[] __riko_function(
            );
            public static riko. @ org.checkerframework.checker.nullness.qual.Nullable Object function(
            ) {
                final byte[] returned = __riko_function(
                );
                final java.lang.Integer result = riko
                  .Marshaler
                  .decode(returned)
                  .unwrap(java.lang.Integer.class);
                return result == null ? null : new riko.Object(result);
            }
        "#;
        let actual =
            JniWriter.write_target_function(&ir.modules[0].functions[0], &ir.modules[0], &ir);
        assert_eq!(
            crate::normalize_source_code(expected),
            crate::normalize_source_code(&actual),
        );

        let expected = quote! {
            #[no_mangle]
            #[allow(clippy::useless_conversion)]
            #[allow(clippy::let_unit_value)]
            #[allow(clippy::unit_arg)]
            pub extern "C" fn Java_riko_1sample_example_Module__1_1riko_1function(
                _env: ::jni::JNIEnv,
                _class: ::jni::objects::JClass
            ) -> ::jni::sys::jbyteArray {
                let result = crate::example::function();
                let result = ::riko_runtime::object::Shelve::shelve(result);
                let result: ::riko_runtime::returned::Returned<::riko_runtime::object::Handle> = result.into();
                ::riko_runtime::Marshal::to_jni(&result, &_env)
            }
        }
        .to_string();
        let actual = JniWriter
            .write_bridge_function(&ir.modules[0].functions[0], &ir.modules[0], &ir)
            .into_token_stream()
            .to_string();
        assert_eq!(expected, actual);
    }

    #[test]
    fn target_type_public() {
        assert_eq!(
            "java.lang. @ org.checkerframework.checker.nullness.qual.Nullable Integer",
            super::target_type_public(MarshalingRule::I32, &syn::parse_quote! { i32 }, "riko")
        );
        assert_eq!(
            "riko. @ org.checkerframework.checker.nullness.qual.Nullable Love",
            super::target_type_public(
                MarshalingRule::Struct,
                &syn::parse_quote! { crate::Love },
                "riko"
            )
        );
        assert_eq!(
            "riko.sample. @ org.checkerframework.checker.nullness.qual.Nullable Love",
            super::target_type_public(
                MarshalingRule::Struct,
                &syn::parse_quote! { crate::sample::Love },
                "riko"
            )
        );
        assert_eq!(
            "@ org.checkerframework.checker.nullness.qual.Nullable Love",
            super::target_type_public(MarshalingRule::Struct, &syn::parse_quote! { Love }, "riko")
        );
        assert_eq!(
            "byte @ org.checkerframework.checker.nullness.qual.Nullable []",
            super::target_type_public(
                MarshalingRule::Bytes,
                &syn::parse_quote! { Vec<u8> },
                "riko"
            )
        );
    }
}
