use crate::MacroExpander;
use proc_macro2::TokenStream;
use quote::quote;
use riko_core::parse::Fun;
use riko_core::parse::MarshalingRule;
use syn::FnArg;
use syn::Ident;
use syn::ItemFn;
use syn::ItemStruct;
use syn::ReturnType;
use syn::Type;

pub struct JniExpander;

impl MacroExpander for JniExpander {
    fn fun(&self, item: &mut ItemFn, args: &Fun) -> TokenStream {
        // Name of the generated function
        let original_name = &item.sig.ident;
        let result_name = mangle_function_name(item);

        // Remove all `#[riko::marshal]`
        if let Err(err) = MarshalingRule::parse(item.sig.inputs.iter_mut()) {
            return err.to_compile_error();
        }

        // Parameters of the generated function
        let mut result_params = Vec::<TokenStream>::new();
        result_params.push(quote! { _env: ::jni::JNIEnv });
        result_params.push(quote! { _class: ::jni::objects::JClass });

        // Function arguments placed at the invocation of the original function
        let mut result_args_invoked = Vec::<TokenStream>::new();

        for index in 0..item.sig.inputs.len() {
            // Parameters
            let param_name = quote::format_ident!("arg_{}_jni", index);
            result_params.push(quote! { #param_name : ::jni::sys::jbyteArray });

            // Marshal JNI data as Rust data
            let arg_original = &item.sig.inputs[index];
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
                syn::Error::new_spanned(arg_original, "Does not support this kind of parameter")
                    .to_compile_error()
            };
            result_args_invoked.push(arg_invoked);
        }

        // Block that calls the original function
        let result_block_invocation = match &args.marshal {
            Some(output) => {
                let output_type = output.to_rust_return_type();
                let into_returned = if let Some(MarshalingRule::Iterator(_)) = &args.marshal {
                    quote! {
                        let returned = ::riko_runtime::iterator::IntoReturned::into(result);
                    }
                } else {
                    quote! {
                        let returned: ::riko_runtime::returned::Returned<#output_type> = std::convert::Into::into(result);
                    }
                };
                quote! {
                    let result = #original_name(
                        #(#result_args_invoked),*
                    );
                    #into_returned
                    ::riko_runtime::Marshaled::to_jni(&returned, &_env)
                }
            }
            None => quote! { #original_name(#(#result_args_invoked),*) },
        };

        // Return type of the generated function
        let result_output = if let ReturnType::Default = item.sig.output {
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

    fn heaped(&self, item: &ItemStruct) -> TokenStream {
        let pool_name = quote::format_ident!("__riko_POOL_{}", item.ident);
        let struct_name = &item.ident;
        let result = quote! {
            impl ::riko_runtime::heap::Heaped for #struct_name {
                fn into_handle(self) -> ::riko_runtime::returned::Returned<::riko_runtime::heap::Handle> {
                    ::riko_runtime::heap::Pool::store(&*#pool_name, self).into()
                }
            }

            #[allow(non_upper_case_globals)]
            static #pool_name: ::once_cell::sync::Lazy<::riko_runtime::heap::SimplePool<#struct_name>> = ::once_cell::sync::Lazy::new(
                ::std::default::Default::default
            );
        };
        result
    }
}

fn mangle_function_name(function: &ItemFn) -> Ident {
    let raw = riko_core::parse::mangle_function_name(function);
    quote::format_ident!("Java_{}", raw.to_string().replace("_", "_1"))
}

mod tests {
    use super::*;

    #[test]
    fn fun_nothing() {
        let mut function: syn::ItemFn = syn::parse_quote! {
            fn function() {}
        };
        let args: Fun = syn::parse_quote! {
            name = "function",
        };
        let mangled_name = mangle_function_name(&function);

        let actual = JniExpander.fun(&mut function, &args).to_string();
        let expected = quote! {
            #[no_mangle]
            pub extern "C" fn #mangled_name(
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
        let mut function: syn::ItemFn = syn::parse_quote! {
            fn function(
                a: &String,
                #[riko::marshal(String)] b: Option<String>,
            ) -> Result<Option<String>> {
                unimplemented!()
            }
        };
        let args: Fun = syn::parse_quote! {
            name = "function",
            marshal = "String",
        };
        let mangled_name = mangle_function_name(&function);

        let actual = JniExpander.fun(&mut function, &args).to_string();
        let expected = quote! {
            #[no_mangle]
            pub extern "C" fn #mangled_name(
                _env: ::jni::JNIEnv,
                _class: ::jni::objects::JClass,
                arg_0_jni: ::jni::sys::jbyteArray,
                arg_1_jni: ::jni::sys::jbyteArray
            ) -> ::jni::sys::jbyteArray {
                let result = function(
                    &(::riko_runtime::Marshaled::from_jni(&_env, arg_0_jni)),
                    ::riko_runtime::Marshaled::from_jni(&_env, arg_1_jni)
                );
                let returned: ::riko_runtime::returned::Returned<::std::string::String> = std::convert::Into::into(result);
                ::riko_runtime::Marshaled::to_jni(&returned, &_env)
            }
        }
        .to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn fun_iterator() {
        let mut function: syn::ItemFn = syn::parse_quote! {
            fn function(
                #[riko::marshal(String)] a: String,
                #[riko::marshal(String)] b: String,
            ) -> Box<dyn Iterator<Item = String> + Send + 'static> {
                unimplemented!()
            }
        };
        let args: Fun = syn::parse_quote! {
            name = "function",
            marshal = "Iterator<String>",
        };
        let mangled_name = mangle_function_name(&function);

        let actual = JniExpander.fun(&mut function, &args).to_string();
        let expected = quote! {
            #[no_mangle]
            pub extern "C" fn #mangled_name(
                _env: ::jni::JNIEnv,
                _class: ::jni::objects::JClass,
                arg_0_jni: ::jni::sys::jbyteArray,
                arg_1_jni: ::jni::sys::jbyteArray
            ) -> ::jni::sys::jbyteArray {
                let result = function(
                    ::riko_runtime::Marshaled::from_jni(&_env, arg_0_jni),
                    ::riko_runtime::Marshaled::from_jni(&_env, arg_1_jni)
                );
                let returned = ::riko_runtime::iterator::IntoReturned::into(result);
                ::riko_runtime::Marshaled::to_jni(&returned, &_env)
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
        let actual = JniExpander.heaped(&protagonist).to_string();

        let expected = quote ! {
            impl ::riko_runtime::heap::Heaped for NuclearReactor {
                fn into_handle(self) -> ::riko_runtime::returned::Returned<::riko_runtime::heap::Handle> {
                    ::riko_runtime::heap::Pool::store(&*__riko_POOL_NuclearReactor, self).into()
                }
            }

            #[allow(non_upper_case_globals)]
            static __riko_POOL_NuclearReactor: ::once_cell::sync::Lazy<::riko_runtime::heap::SimplePool<NuclearReactor>> = ::once_cell::sync::Lazy::new(
                ::std::default::Default::default
            );
        }.to_string();

        assert_eq!(expected, actual);
    }
}
