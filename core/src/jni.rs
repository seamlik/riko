//! Java support using JNI.

use crate::ir::Crate;
use crate::ir::Function;
use crate::ir::MarshalingRule;
use crate::ir::Module;
use crate::ErrorSource;
use crate::TargetCodeWriter;
use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::quote;
use quote::ToTokens;
use std::path::Path;
use syn::Ident;
use syn::ItemFn;

const CLASS_FOR_MODULE: &str = "Module";
const PREFIX_FOR_NATIVE: &str = "__riko_";

/// Writes JNI bindings.
pub struct JniWriter;

impl TargetCodeWriter for JniWriter {
    fn write_target_all(&self, root: &Crate, output_directory: &Path) -> Result<(), crate::Error> {
        for module in root.modules.iter() {
            let mut file_path = output_directory.to_owned();
            file_path.push(&root.name);
            file_path.extend(module.path.iter());
            file_path.push(format!("{}.java", CLASS_FOR_MODULE));

            crate::write_file(&file_path, &self.write_target_module(module, root)).map_err(
                |err| crate::Error {
                    file: file_path,
                    source: ErrorSource::Write(err),
                },
            )?;
        }
        Ok(())
    }

    fn write_target_function(&self, function: &Function, _: &Module, crate_: &Crate) -> String {
        let return_type_public = target_type_public(
            function.output.rule,
            &function.output.unwrapped_type.0,
            &crate_.name,
        );

        let return_block = match function.output.rule {
            MarshalingRule::Unit => "",
            MarshalingRule::Any => "return new riko.Any(result);",
            _ => "return result;",
        };

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
            return_type_local = target_type_local(
                function.output.rule,
                &function.output.unwrapped_type.0,
                &crate_.name
            ),
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
        let shelve = if function.output.rule == MarshalingRule::Any {
            let method = match (function.output.result, function.output.option) {
                (false, false) => quote::format_ident!("shelve_self"),
                (false, true) => quote::format_ident!("shelve_option"),
                (true, false) => quote::format_ident!("shelve_result"),
                (true, true) => quote::format_ident!("shelve_result_option"),
            };
            quote! {
                let result = ::riko_runtime::object::Object::#method(result);
            }
        } else {
            Default::default()
        };

        // Inherited `#[cfg]`
        let cfg = function.collect_cfg(module, root);

        let result: ItemFn = syn::parse_quote! {
            #(#cfg)*
            #[no_mangle]
            #[allow(clippy::identity_conversion)]
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

fn target_type_public(
    rule: MarshalingRule,
    unwrapped_type: &syn::Path,
    crate_name: &str,
) -> String {
    match rule {
        MarshalingRule::Any => "riko.Any".into(),
        MarshalingRule::Bool => "java.lang.Boolean".into(),
        MarshalingRule::Bytes => "byte[]".into(),
        MarshalingRule::I8 => "java.lang.Byte".into(),
        MarshalingRule::I32 => "java.lang.Integer".into(),
        MarshalingRule::I64 => "java.lang.Long".into(),
        MarshalingRule::Struct => {
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
            result.join(".")
        }
        MarshalingRule::String => "java.lang.String".into(),
        MarshalingRule::Unit => "void".into(),
    }
}

/// To use in `riko.Returned<#target_type_local>`.
fn target_type_local(rule: MarshalingRule, unwrapped_type: &syn::Path, crate_name: &str) -> String {
    match rule {
        MarshalingRule::Any => "java.lang.Integer".into(),
        MarshalingRule::Unit => "java.lang.Object".into(),
        _ => target_type_public(rule, unwrapped_type, crate_name),
    }
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

mod tests {
    use super::*;
    use crate::ir::*;
    use crate::parse::*;

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
    fn module_nothing() {
        let expected = r#"
            package riko_sample.example;

            public final class Module {
                private Module() {}
            }
        "#;
        let ir = Crate {
            name: "riko_sample".into(),
            modules: vec![Module {
                functions: vec![],
                path: vec!["example".into()],
                cfg: Default::default(),
            }],
        };
        let actual = JniWriter.write_target_module(&ir.modules[0], &ir);
        assert_eq!(
            crate::normalize_source_code(expected),
            crate::normalize_source_code(&actual),
        );
    }

    #[test]
    fn function_rename_target() {
        let expected = r#"
            private static native byte[] __riko_function( );
            public static void function( ) {
              final byte[] returned = __riko_function( );
              final java.lang.Object result = riko
                .Marshaler
                .decode(returned)
                .unwrap(java.lang.Object.class);
            }
        "#;
        let ir = Crate {
            name: "riko_sample".into(),
            modules: vec![Module {
                functions: vec![Function {
                    name: "function_ffi".into(),
                    inputs: vec![],
                    output: Default::default(),
                    pubname: "function".into(),
                    cfg: Default::default(),
                }],
                path: vec!["example".into()],
                cfg: Default::default(),
            }],
        };
        let actual =
            JniWriter.write_target_function(&ir.modules[0].functions[0], &ir.modules[0], &ir);

        assert_eq!(
            crate::normalize_source_code(expected),
            crate::normalize_source_code(&actual),
        );
    }

    #[test]
    fn function_rename_bridge() {
        let ir = Crate {
            name: "riko_sample".into(),
            modules: vec![Module {
                functions: vec![Function {
                    name: "function_ffi".into(),
                    inputs: vec![],
                    output: Default::default(),
                    pubname: "function".into(),
                    cfg: Default::default(),
                }],
                path: vec!["util".into()],
                cfg: Default::default(),
            }],
        };
        let actual = JniWriter
            .write_bridge_function(&ir.modules[0].functions[0], &ir.modules[0], &ir)
            .into_token_stream()
            .to_string();
        let expected = quote! {
            #[no_mangle]
            #[allow(clippy::identity_conversion)]
            #[allow(clippy::let_unit_value)]
            #[allow(clippy::unit_arg)]
            pub extern "C" fn Java_riko_1sample_util_Module__1_1riko_1function(
                _env: ::jni::JNIEnv,
                _class: ::jni::objects::JClass
            ) -> ::jni::sys::jbyteArray {
                let result = crate::util::function_ffi();
                let result: ::riko_runtime::returned::Returned<()> = result.into();
                ::riko_runtime::Marshal::to_jni(&result, &_env)
            }
        }
        .to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn function_simple_target() {
        let expected = r#"
            private static native byte[] __riko_function(
                byte[] arg_0,
                byte[] arg_1
            );
            public static java.lang.String function(
                final java.lang.Integer arg_0,
                final java.lang.Long arg_1
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
        let ir = Crate {
            name: "riko_sample".into(),
            modules: vec![Module {
                functions: vec![Function {
                    name: "function".into(),
                    pubname: "function".into(),
                    inputs: vec![
                        Input {
                            rule: MarshalingRule::I32,
                            borrow: true,
                            unwrapped_type: Assertable(syn::parse_quote! { i32 }),
                        },
                        Input {
                            rule: MarshalingRule::I64,
                            borrow: false,
                            unwrapped_type: Assertable(syn::parse_quote! { i64 }),
                        },
                    ],
                    output: Output {
                        rule: MarshalingRule::String,
                        result: false,
                        option: false,
                        unwrapped_type: Assertable(syn::parse_quote! { String }),
                    },
                    cfg: Default::default(),
                }],
                path: vec!["example".into()],
                cfg: Default::default(),
            }],
        };
        let actual =
            JniWriter.write_target_function(&ir.modules[0].functions[0], &ir.modules[0], &ir);

        assert_eq!(
            crate::normalize_source_code(expected),
            crate::normalize_source_code(&actual),
        );
    }

    #[test]
    fn function_simple_bridge() {
        let ir = Crate {
            name: "riko_sample".into(),
            modules: vec![Module {
                functions: vec![Function {
                    name: "function".into(),
                    pubname: "function".into(),
                    inputs: vec![
                        Input {
                            rule: MarshalingRule::I32,
                            borrow: true,
                            unwrapped_type: Assertable(syn::parse_quote! { i32 }),
                        },
                        Input {
                            rule: MarshalingRule::I64,
                            borrow: false,
                            unwrapped_type: Assertable(syn::parse_quote! { i64 }),
                        },
                    ],
                    output: Output {
                        rule: MarshalingRule::String,
                        result: false,
                        option: false,
                        unwrapped_type: Assertable(syn::parse_quote! { String }),
                    },
                    cfg: Default::default(),
                }],
                path: vec!["util".into()],
                cfg: Default::default(),
            }],
        };
        let actual = JniWriter
            .write_bridge_function(&ir.modules[0].functions[0], &ir.modules[0], &ir)
            .into_token_stream()
            .to_string();
        let expected = quote! {
            #[no_mangle]
            #[allow(clippy::identity_conversion)]
            #[allow(clippy::let_unit_value)]
            #[allow(clippy::unit_arg)]
            pub extern "C" fn Java_riko_1sample_util_Module__1_1riko_1function(
                _env: ::jni::JNIEnv,
                _class: ::jni::objects::JClass,
                arg_0_jni: ::jni::sys::jbyteArray,
                arg_1_jni: ::jni::sys::jbyteArray
            ) -> ::jni::sys::jbyteArray {
                let result = crate::util::function(
                    &(::riko_runtime::Marshal::from_jni(&_env, arg_0_jni)),
                    ::riko_runtime::Marshal::from_jni(&_env, arg_1_jni)
                );
                let result: ::riko_runtime::returned::Returned<::std::string::String> = result.into();
                ::riko_runtime::Marshal::to_jni(&result, &_env)
            }
        }
        .to_string();

        assert_eq!(expected, actual);
    }
}
