use proc_macro2::TokenStream;
use quote::quote;
use quote::ToTokens;
use syn::Attribute;
use syn::FnArg;
use syn::ItemFn;
use syn::ItemStruct;

pub fn object(item: &ItemStruct) -> TokenStream {
    let struct_name = &item.ident;
    let result = quote! {
        impl ::riko_runtime::object::Object for #struct_name {
            fn into_handle(self) -> ::riko_runtime::returned::Returned<::riko_runtime::object::Handle> {
                ::riko_runtime::object::Pool::store(&*::riko_runtime::object::POOL, self).into()
            }
        }
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
    fn object() {
        let protagonist: ItemStruct = syn::parse_quote! {
            struct NuclearReactor;
        };
        let actual = super::object(&protagonist).to_string();

        let expected = quote ! {
            impl ::riko_runtime::object::Object for NuclearReactor {
                fn into_handle(self) -> ::riko_runtime::returned::Returned<::riko_runtime::object::Handle> {
                    ::riko_runtime::object::Pool::store(&*::riko_runtime::object::POOL, self).into()
                }
            }
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
