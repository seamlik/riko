use crate::parse::Fun;
use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::quote;
use syn::FnArg;
use syn::Ident;
use syn::ItemUse;
use syn::ReturnType;
use syn::Signature;
use syn::Type;

/// Generates Rust code wrapping a `Heaped`.
pub fn heaped(name: &Ident) -> TokenStream {
    let pool_name = quote::format_ident!("__RIKO_POOL_{}", name);
    let result = quote! {
        impl ::riko_runtime::heap::Heaped for #name {
            fn into_handle(self) -> ::riko_runtime::returned::Returned<::riko_runtime::heap::Handle> {
                ::riko_runtime::heap::Pool::store(&*#pool_name, self).into()
            }
        }

        #[allow(non_upper_case_globals)]
        static #pool_name: ::once_cell::sync::Lazy<::riko_runtime::heap::SimplePool<#name>> = ::once_cell::sync::Lazy::new(
            || ::std::default::Default::default()
        );
    };
    result
}

/// Generates Rust code wrapping a function.
///
/// # Parameters
///
/// * `args`: Must be a complete version with all optional fields filled in.
pub fn fun(sig: &Signature, args: &Fun) -> TokenStream {
    // Global defs
    let has_iterators = args.sig.has_iterators();

    // Name of the generated function
    let original_name = &sig.ident;
    let result_name = mangle_function(&args.name, args.module.iter());

    // `use` statements
    let mut result_uses = Vec::<ItemUse>::new();
    if has_iterators {
        result_uses.push(syn::parse_quote! { use ::riko_runtime::iterator::IntoReturned; })
    }

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
            syn::Error::new_spanned(arg_original, "Does not support this kind of parameter.")
                .to_compile_error()
        };
        result_args_invoked.push(arg_invoked);
    }

    // Block that calls the original function
    let result_invocation_conversion = if has_iterators {
        quote! { .__riko_into_returned() }
    } else {
        quote! { .into() }
    };
    let result_block_invocation = match &args.sig.output {
        Some(output) => {
            let output_type = output.to_rust_return_type();
            quote! {
                let result: ::riko_runtime::returned::Returned< #output_type > = #original_name(
                    #(#result_args_invoked),*
                )
                #result_invocation_conversion ;

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
            #(#result_uses)*
            #result_block_invocation
        }
    };
    result
}

/// Transform a function's original name to the one used by JNI.
fn mangle_function<'a>(name: &str, module: impl Iterator<Item = &'a String>) -> Ident {
    let mut module_mangled = module.map(|it| it.replace("_", "_1")).join("_");
    if !module_mangled.is_empty() {
        module_mangled.push_str("_");
    }

    quote::format_ident!(
        "Java_{}_1_1Riko_1Module__1_1riko_1{}",
        module_mangled,
        name.replace("_", "_1")
    )
}

mod tests {
    use super::*;

    #[test]
    fn mangle_function() {
        assert_eq!(
            "Java_org_example__1_1Riko_1Module__1_1riko_1run",
            super::mangle_function("run", vec!["org".into(), "example".into()].iter()).to_string()
        );
        assert_eq!(
            "Java__1_1Riko_1Module__1_1riko_1run",
            super::mangle_function("run", std::iter::empty()).to_string()
        );
    }

    #[test]
    fn fun_nothing() {
        let function: syn::ItemFn = syn::parse_quote! {
            fn function() {}
        };
        let args: Fun = syn::parse_quote! {
            module = "samples",
            name = "function",
        };

        let actual = fun(&function.sig, &args).to_string();
        let expected = quote! {
            #[no_mangle]
            pub extern "C" fn Java_samples__1_1Riko_1Module__1_1riko_1function(
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
            module = "samples",
            name = "function",
            sig = "(String, String) -> String",
        };

        let actual = fun(&function.sig, &args).to_string();
        let expected = quote! {
            #[no_mangle]
            pub extern "C" fn Java_samples__1_1Riko_1Module__1_1riko_1function(
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

    #[test]
    fn fun_iterator() {
        let function: syn::ItemFn = syn::parse_quote! {
            fn function(a: String, b: String) -> Box<dyn Iterator<Item = String> + Send + 'static> {
                unimplemented!()
            }
        };
        let args: Fun = syn::parse_quote! {
            module = "samples",
            name = "function",
            sig = "(String, String) -> Iterator<String>",
        };

        let actual = fun(&function.sig, &args).to_string();
        let expected = quote! {
            #[no_mangle]
            pub extern "C" fn Java_samples__1_1Riko_1Module__1_1riko_1function(
                _env: ::jni::JNIEnv,
                _class: ::jni::objects::JClass,
                arg_0_jni: ::jni::sys::jbyteArray,
                arg_1_jni: ::jni::sys::jbyteArray
            ) -> ::jni::sys::jbyteArray {
                use ::riko_runtime::iterator::IntoReturned;

                let result: ::riko_runtime::returned::Returned<_> = function(
                    ::riko_runtime::Marshaled::from_jni(&_env, arg_0_jni),
                    ::riko_runtime::Marshaled::from_jni(&_env, arg_1_jni)
                )
                .__riko_into_returned();

                ::riko_runtime::Marshaled::to_jni(&result, &_env)
            }
        }
        .to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn heap() {
        let protagonist: syn::ItemStruct = syn::parse_quote! {
            struct NuclearReactor;
        };
        let actual = heaped(&protagonist.ident).to_string();

        let expected = quote ! {
            impl ::riko_runtime::heap::Heaped for NuclearReactor {
                fn into_handle(self) -> ::riko_runtime::returned::Returned<::riko_runtime::heap::Handle> {
                    ::riko_runtime::heap::Pool::store(&*__RIKO_POOL_NuclearReactor, self).into()
                }
            }

            #[allow(non_upper_case_globals)]
            static __RIKO_POOL_NuclearReactor: ::once_cell::sync::Lazy<::riko_runtime::heap::SimplePool<NuclearReactor>> = ::once_cell::sync::Lazy::new(
                || ::std::default::Default::default()
            );
        }.to_string();

        assert_eq!(expected, actual);
    }
}
