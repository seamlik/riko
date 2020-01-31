//! Java support using JNI.

use crate::ir::Crate;
use crate::ir::Function;
use crate::ir::Module;
use crate::parse::MarshalingRule;
use crate::ErrorSource;
use crate::TargetCodeWriter;
use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::quote;
use quote::ToTokens;
use std::path::Path;
use syn::Ident;
use syn::ItemFn;

const CLASS_FOR_MODULE: &str = "__riko_Module";

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

    fn write_target_function(&self, function: &Function, _: &Module, _: &Crate) -> String {
        let return_type_result = match &function.output {
            None => "void".into(),
            Some(MarshalingRule::Iterator(_)) => return_type(&MarshalingRule::I32),
            Some(inner) => return_type(inner),
        };
        let return_type_public = match &function.output {
            None => "void".into(),
            Some(inner) => return_type(inner),
        };
        let return_prefix = if function.output.is_none() {
            ""
        } else {
            "final byte[] returned ="
        };
        let return_block = if function.output.is_none() {
            "".into()
        } else {
            format!(
                r#"
                    final riko.Returned<{}> result = riko.Marshaler.fromBytes(returned);
                    return result.unwrap();
                "#,
                return_type_result
            )
        };
        let args = function
            .inputs
            .iter()
            .enumerate()
            .map(|(idx, _)| format!("riko.Marshaler.toBytes(arg_{})", idx))
            .join(", ");
        let params_public = function
            .inputs
            .iter()
            .enumerate()
            .map(|(idx, _)| format!("final {} arg_{}", return_type_public, idx))
            .join(", ");
        let return_type_bridge = if function.output.is_none() {
            "void"
        } else {
            "byte[]"
        };
        let params_bridge = function
            .inputs
            .iter()
            .enumerate()
            .map(|(idx, _)| format!("byte[] arg_{}", idx))
            .join(", ");

        format!(
            r#"
                private static native {return_type_bridge} __riko_{name}( {params_bridge} );
                public static {return_type_public} {name}( {params_public} ) {{
                    {return_prefix} __riko_{name}( {args} );
                    {return_block}
                }}
            "#,
            args = args,
            name = &function.pubname,
            params_bridge = params_bridge,
            params_public = params_public,
            return_block = return_block,
            return_prefix = return_prefix,
            return_type_bridge = return_type_bridge,
            return_type_public = return_type_public,
        )
    }

    fn write_bridge_function(&self, function: &Function, module: &Module, root: &Crate) -> ItemFn {
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
                ::riko_runtime::Marshaled::from_jni(&_env, #param_name)
            };
            let arg = if input.borrow {
                quote! { &(#arg_raw) }
            } else {
                arg_raw
            };
            result_args.push(arg);
        }

        // Block that calls the original function
        let result_block_invocation = match &function.output {
            Some(output) => {
                let output_type = output.to_rust_return_type();
                quote! {
                    let result = #full_public_name(
                        #(#result_args),*
                    );
                    let returned: ::riko_runtime::returned::Returned<#output_type> = ::std::convert::Into::into(result);
                    ::riko_runtime::Marshaled::to_jni(&returned, &_env)
                }
            }
            None => quote! { #full_public_name(#(#result_args),*) },
        };

        // Return type of the generated function
        let result_output = if function.output.is_none() {
            TokenStream::default()
        } else {
            quote! { -> ::jni::sys::jbyteArray }
        };

        let result: ItemFn = syn::parse_quote! {
            #[no_mangle]
            pub extern "C" fn #mangled_name(#(#result_params),*) #result_output {
                #result_block_invocation
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

fn return_type(rule: &MarshalingRule) -> String {
    match rule {
        MarshalingRule::Bool => "java.lang.Boolean".into(),
        MarshalingRule::Bytes => "byte[]".into(),
        MarshalingRule::I8 => "java.lang.Byte".into(),
        MarshalingRule::I32 => "java.lang.Integer".into(),
        MarshalingRule::I64 => "java.lang.Long".into(),
        MarshalingRule::Iterator(inner) => format!("java.util.Iterator<{}>", inner),
        MarshalingRule::Serde(inner) => inner.to_token_stream().to_string().replace("::", "."),
        MarshalingRule::String => "java.lang.String".into(),
    }
}

fn mangle_function_name(name: &str, module: &[String], crate_: &str) -> Ident {
    let prefix = std::iter::once(crate_)
        .chain(module.iter().map(String::as_str))
        .map(|ident| ident.replace("_", "_1"))
        .join("_");
    quote::format_ident!(
        "Java_{}__1_1riko_1Module__1_1riko_1{}",
        &prefix,
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
        assert_eq!(
            "Java_riko_1sample__1_1riko_1Module__1_1riko_1function",
            none
        );
        let some_1 =
            super::mangle_function_name("function", &["util".into()], "riko_sample").to_string();
        assert_eq!(
            "Java_riko_1sample_util__1_1riko_1Module__1_1riko_1function",
            some_1
        );
        let some_2 =
            super::mangle_function_name("function", &["util".into(), "unix".into()], "riko_sample")
                .to_string();
        assert_eq!(
            "Java_riko_1sample_util_unix__1_1riko_1Module__1_1riko_1function",
            some_2
        )
    }

    #[test]
    fn module_nothing() {
        let expected = r#"
            package riko_sample.example;

            public final class __riko_Module {
                private __riko_Module() {}
            }
        "#;
        let ir = Crate {
            name: "riko_sample".into(),
            modules: vec![Module {
                functions: vec![],
                path: vec!["example".into()],
            }],
        };
        let actual = JniWriter.write_target_module(&ir.modules[0], &ir);
        assert_eq!(
            crate::normalize_source_code(expected),
            crate::normalize_source_code(&actual),
        );
    }

    #[test]
    fn function_nothing_target() {
        let expected = r#"
            private static native void __riko_function( );
            public static void function( ) {
                __riko_function( );
            }
        "#;
        let ir = Crate {
            name: "riko_sample".into(),
            modules: vec![Module {
                functions: vec![Function {
                    name: "function".into(),
                    inputs: vec![],
                    output: None,
                    pubname: "function".into(),
                }],
                path: vec!["example".into()],
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
    fn function_nothing_bridge() {
        let ir = Crate {
            name: "riko_sample".into(),
            modules: vec![Module {
                functions: vec![Function {
                    name: "function".into(),
                    inputs: vec![],
                    output: None,
                    pubname: "function".into(),
                }],
                path: vec!["util".into()],
            }],
        };
        let actual = JniWriter
            .write_bridge_function(&ir.modules[0].functions[0], &ir.modules[0], &ir)
            .into_token_stream()
            .to_string();
        let expected = quote! {
            #[no_mangle]
            pub extern "C" fn Java_riko_1sample_util__1_1riko_1Module__1_1riko_1function(
                _env: ::jni::JNIEnv,
                _class: ::jni::objects::JClass
            ) {
                crate::util::function()
            }
        }
        .to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn function_rename_target() {
        let expected = r#"
            private static native void __riko_function( );
            public static void function( ) {
                __riko_function( );
            }
        "#;
        let ir = Crate {
            name: "riko_sample".into(),
            modules: vec![Module {
                functions: vec![Function {
                    name: "function_ffi".into(),
                    inputs: vec![],
                    output: None,
                    pubname: "function".into(),
                }],
                path: vec!["example".into()],
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
                    output: None,
                    pubname: "function".into(),
                }],
                path: vec!["util".into()],
            }],
        };
        let actual = JniWriter
            .write_bridge_function(&ir.modules[0].functions[0], &ir.modules[0], &ir)
            .into_token_stream()
            .to_string();
        let expected = quote! {
            #[no_mangle]
            pub extern "C" fn Java_riko_1sample_util__1_1riko_1Module__1_1riko_1function(
                _env: ::jni::JNIEnv,
                _class: ::jni::objects::JClass
            ) {
                crate::util::function_ffi()
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
                final java.lang.String arg_0,
                final java.lang.String arg_1
            ) {
                final byte[] returned = __riko_function(
                    riko.Marshaler.toBytes(arg_0),
                    riko.Marshaler.toBytes(arg_1)
                );
                final riko.Returned<java.lang.String> result = riko.Marshaler.fromBytes(returned);
                return result.unwrap();
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
                            rule: MarshalingRule::String,
                            borrow: true,
                        },
                        Input {
                            rule: MarshalingRule::String,
                            borrow: false,
                        },
                    ],
                    output: Some(MarshalingRule::String),
                }],
                path: vec!["example".into()],
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
                            rule: MarshalingRule::String,
                            borrow: true,
                        },
                        Input {
                            rule: MarshalingRule::String,
                            borrow: false,
                        },
                    ],
                    output: Some(MarshalingRule::String),
                }],
                path: vec!["util".into()],
            }],
        };
        let actual = JniWriter
            .write_bridge_function(&ir.modules[0].functions[0], &ir.modules[0], &ir)
            .into_token_stream()
            .to_string();
        let expected = quote! {
            #[no_mangle]
            pub extern "C" fn Java_riko_1sample_util__1_1riko_1Module__1_1riko_1function(
                _env: ::jni::JNIEnv,
                _class: ::jni::objects::JClass,
                arg_0_jni: ::jni::sys::jbyteArray,
                arg_1_jni: ::jni::sys::jbyteArray
            ) -> ::jni::sys::jbyteArray {
                let result = crate::util::function(
                    &(::riko_runtime::Marshaled::from_jni(&_env, arg_0_jni)),
                    ::riko_runtime::Marshaled::from_jni(&_env, arg_1_jni)
                );
                let returned: ::riko_runtime::returned::Returned<::std::string::String> = ::std::convert::Into::into(result);
                ::riko_runtime::Marshaled::to_jni(&returned, &_env)
            }
        }
        .to_string();

        assert_eq!(expected, actual);
    }
}
