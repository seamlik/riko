use proc_macro2::TokenStream;
use quote::quote;
use quote::ToTokens;
use syn::Attribute;
use syn::FnArg;
use syn::ItemFn;
use syn::ItemStruct;

pub fn heaped(item: &ItemStruct) -> TokenStream {
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

pub fn remove_marshal_attrs(function: &mut ItemFn) {
    fn remove(attrs: &mut Vec<Attribute>) {
        attrs
            .drain_filter(|attr| attr.path.to_token_stream().to_string() == "riko :: marshal")
            .for_each(drop);
    }
    for param in function.sig.inputs.iter_mut() {
        match param {
            FnArg::Receiver(inner) => remove(&mut inner.attrs),
            FnArg::Typed(inner) => remove(&mut inner.attrs),
        }
    }
}

mod tests {
    use super::*;

    #[test]
    fn heaped() {
        let protagonist: ItemStruct = syn::parse_quote! {
            struct NuclearReactor;
        };
        let actual = super::heaped(&protagonist).to_string();

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

    #[test]
    fn remove_marshal_attrs() {
        let mut actual: ItemFn = syn::parse_quote! {
            pub fn function(
                src: usize,
                #[riko::marshal(I64)] dst: Option<usize>
            ) -> String {
                unimplemented!()
            }
        };
        let expected: ItemFn = syn::parse_quote! {
            pub fn function(src: usize, dst: Option<usize>) -> String {
                unimplemented!()
            }
        };
        super::remove_marshal_attrs(&mut actual);
        assert_eq!(
            expected.into_token_stream().to_string(),
            actual.into_token_stream().to_string()
        )
    }
}
