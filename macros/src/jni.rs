use crate::parse::Fun;
use proc_macro2::TokenStream;
use quote::quote;
use quote::ToTokens;
use syn::spanned::Spanned;
use syn::FnArg;
use syn::Ident;
use syn::ReturnType;
use syn::Signature;
use syn::Type;

/// Generates Rust code wrapping a function.
pub fn gen_function_rust(sig: &Signature, args: &Fun, module: &str) -> TokenStream {
    // Name of the generated function
    let original_name = &sig.ident;
    let result_name = mangle_function(&original_name.to_string(), &args.name, module);

    // Parameters of the generated function
    let mut result_params = Vec::<TokenStream>::new();
    result_params.push(quote! { _env: ::jni::JNIEnv });
    result_params.push(quote! { _class: ::jni::objects::JClass });

    // Function arguments placed at the invocation of the original function
    let mut result_args_invoked = Vec::<TokenStream>::new();

    for index in 0..(args.sig.inputs.len()) {
        // Parameters
        let param_name = quote::format_ident!("arg_{}_jni", index);
        result_params.push(quote! { #param_name : ::jni::sys::jbyteArray });

        // Using the arguments
        let arg_original = &sig.inputs[index];
        let arg_invoked = if let FnArg::Typed(pattern) = arg_original {
            let candidate = quote! {
                ::riko_runtime::Marshaled::from_jni(&_env, #param_name)
            };
            if let Type::Reference(_) = *pattern.ty {
                quote! { &(#candidate) }
            } else {
                candidate
            }
        } else {
            syn::Error::new(
                arg_original.span(),
                "Does not support this kind of parameter.",
            )
            .to_compile_error()
        };
        result_args_invoked.push(arg_invoked);
    }

    // Block that calls the original function
    let result_block_invocation = match &args.sig.output {
        Some(output) => {
            let output_type = output
                .to_rust_return_type()
                .map(ToTokens::into_token_stream)
                .unwrap_or_else(|err| err.to_compile_error());
            quote! {
                let result: ::riko_runtime::returned::Returned< #output_type > = #original_name(
                    #(#result_args_invoked),*
                ).into();
                ::riko_runtime::Marshaled::to_jni(&result, &_env)
            }
        }
        None => quote! { #original_name(#(#result_args_invoked),*) },
    };

    // Return type of the generated function
    let result_output = if let ReturnType::Default = sig.output {
        TokenStream::default()
    } else {
        quote! { -> ::jni::sys::jbyteArray }
    };

    let result = quote! {
        #[no_mangle]
        pub extern "C" fn #result_name(#(#result_params),*) #result_output {
            #result_block_invocation
        }
    };
    result
}

/// Transform a function's original name to the one used by JNI.
fn mangle_function(name_orig: &str, name_new: &str, module: &str) -> Ident {
    let name_mangled = if name_new.is_empty() {
        name_orig
    } else {
        name_new
    };

    let mut module_mangled = module.replace("_", "_1").replace("::", "_");
    if !module.is_empty() {
        module_mangled.push_str("_");
    }

    quote::format_ident!(
        "Java_{}Module__1_1riko_1{}",
        module_mangled,
        name_mangled.replace("_", "_1")
    )
}

mod tests {
    use super::*;

    #[test]
    fn mangle_function() {
        assert_eq!(
            "Java_org_examples_Module__1_1riko_1run",
            super::mangle_function("run", "", "org::examples").to_string()
        );
        assert_eq!(
            "Java_Module__1_1riko_1run",
            super::mangle_function("run", "", "").to_string()
        );
        assert_eq!(
            "Java_org_examples_Module__1_1riko_1run",
            super::mangle_function("run_ffi", "run", "org::examples").to_string()
        );
    }

    #[test]
    fn fun_nothing() {
        let function: syn::ItemFn = syn::parse_quote! {
            fn function() {}
        };
        let args = Fun::default();
        let actual = gen_function_rust(&function.sig, &args, "").to_string();

        let expected = quote! {
            #[no_mangle]
            pub extern "C" fn Java_Module__1_1riko_1function(
                _env: ::jni::JNIEnv,
                _class: ::jni::objects::JClass
            ) {
                function()
            }
        }
        .to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn fun_simple() {
        let function: syn::ItemFn = syn::parse_quote! {
            fn function(a: &String, b: Option<String>) -> Result<Option<String>> {
                unimplemented!()
            }
        };
        let args: Fun = syn::parse_quote! {
            sig = "(String, String) -> String"
        };
        let actual = gen_function_rust(&function.sig, &args, "").to_string();

        let expected = quote! {
            #[no_mangle]
            pub extern "C" fn Java_Module__1_1riko_1function(
                _env: ::jni::JNIEnv,
                _class: ::jni::objects::JClass,
                arg_0_jni: ::jni::sys::jbyteArray,
                arg_1_jni: ::jni::sys::jbyteArray
            ) -> ::jni::sys::jbyteArray {
                let result: ::riko_runtime::returned::Returned<::std::string::String> = function(
                    &(::riko_runtime::Marshaled::from_jni(&_env, arg_0_jni)),
                    ::riko_runtime::Marshaled::from_jni(&_env, arg_1_jni)
                ).into();
                ::riko_runtime::Marshaled::to_jni(&result, &_env)
            }
        }
        .to_string();

        assert_eq!(expected, actual);
    }
}
