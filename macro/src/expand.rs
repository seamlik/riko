use quote::ToTokens;
use syn::Attribute;
use syn::FnArg;
use syn::ItemFn;

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
