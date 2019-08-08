use crate::parse::MarshalingRule;
use crate::parse::MarshalingSignature;
use proc_macro::TokenStream;
use proc_quote::quote;
use syn::punctuated::Punctuated;
use syn::FnArg;
use syn::Ident;
use syn::Token;

pub fn gen_function_rust(name: &str, module: &str, sig: &MarshalingSignature) -> TokenStream {
    let result_name = mangle_function(name, module);

    let mut args_function = Punctuated::<FnArg, Token![,]>::new();
    args_function.push(syn::parse_str("__riko_env: ::jni::JNIEnv").unwrap());
    args_function.push_value(syn::parse_str("__riko_class: ::jni::objects::JClass").unwrap());
    for i in 0..sig.inputs.len() {
        let tokens = format!(
            "__riko_{}_raw: {}",
            i,
            target_type_in(&sig.inputs.get(i).unwrap())
        );
        args_function.push_value(syn::parse_str(&tokens).unwrap());
    }

    let mut args_invocation = Punctuated::<Ident, Token![,]>::new();
    for i in 0..sig.inputs.len() {
        let tokens = format!("__riko_{}", i);
        args_invocation.push(syn::parse_str(&tokens).unwrap());
    }

    let result = quote! {
        #[no_mangle]
        pub unsafe extern "C" fn #result_name(#args_function) {
            #name(#args_invocation)
        }
    };

    unimplemented!()
}

fn mangle_function(name: &str, module: &str) -> String {
    format!(
        "Java_{}_Module__1_1riko_1{}",
        module.replace("::", "_"),
        name
    )
}

fn target_type_in(rule: &MarshalingRule) -> &'static str {
    match rule {
        _ => "",
    }
}

fn target_type_out(rule: &MarshalingRule) -> &'static str {
    match rule {
        _ => "",
    }
}
