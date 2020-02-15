use syn::GenericArgument;
use syn::Lit;
use syn::LitStr;
use syn::Path;
use syn::PathArguments;
use syn::Type;

/// Checks if a type is a [Result], if it's an [Option], and returns the inner type.
pub fn unwrap_result_option(ty: &syn::Path) -> syn::Result<(bool, bool, Path)> {
    if let Some(first_segment) = ty.segments.last() {
        let first_segment_name = first_segment.ident.to_string();
        let first_segment_arg = first_argument(&first_segment.arguments);
        if first_segment_name == "Option" {
            Ok((false, true, first_segment_arg?))
        } else if first_segment_name == "Result" {
            let first_segment_arg = first_segment_arg?;
            if let Some(second_segment) = first_segment_arg.segments.last() {
                let second_segment_name = second_segment.ident.to_string();
                let second_segment_arg = first_argument(&second_segment.arguments);
                if second_segment_name == "Option" {
                    Ok((true, true, second_segment_arg?))
                } else {
                    Ok((true, false, first_segment_arg))
                }
            } else {
                Err(syn::Error::new_spanned(
                    ty,
                    "Expect the type for a `Result`",
                ))
            }
        } else {
            Ok((false, false, ty.clone()))
        }
    } else {
        let empty_type = syn::Path {
            leading_colon: None,
            segments: Default::default(),
        };
        Ok((false, false, empty_type))
    }
}

/// Gets the first generic argument.
///
/// E.g. `int` in `Option<int>`.
fn first_argument(arg: &PathArguments) -> syn::Result<Path> {
    if let PathArguments::AngleBracketed(inner) = arg {
        if let Some(GenericArgument::Type(ty)) = inner.args.first() {
            assert_type_is_path(&ty)
        } else {
            Err(syn::Error::new_spanned(inner, "Expect type argument"))
        }
    } else {
        Err(syn::Error::new_spanned(
            arg,
            "Expect a angle bracket surrounded token",
        ))
    }
}

pub fn assert_type_is_path(src: &Type) -> syn::Result<Path> {
    let msg = "Expect a type path or a unit";
    match src {
        Type::Path(path) => Ok(path.path.clone()),
        Type::Tuple(tuple) => {
            if tuple.elems.is_empty() {
                Ok(syn::Path {
                    leading_colon: None,
                    segments: Default::default(),
                })
            } else {
                Err(syn::Error::new_spanned(tuple, msg))
            }
        }
        _ => Err(syn::Error::new_spanned(src, msg)),
    }
}

pub fn assert_lit_is_litstr(src: &Lit) -> syn::Result<&LitStr> {
    if let Lit::Str(litstr) = src {
        Ok(litstr)
    } else {
        Err(syn::Error::new_spanned(src, "Expect a string literal"))
    }
}

mod test {
    use super::*;
    use quote::ToTokens;

    #[test]
    fn first_argument() {
        let simple: Path = syn::parse_quote! { Option<bool> };
        assert_eq!(
            "bool",
            super::first_argument(&simple.segments.first().unwrap().arguments)
                .unwrap()
                .to_token_stream()
                .to_string()
        );

        let empty: Path = syn::parse_quote! { bool };
        assert!(super::first_argument(&empty.segments.first().unwrap().arguments).is_err());

        let iter: Path = syn::parse_quote! { Iterator<Item = bool> };
        assert!(super::first_argument(&iter.segments.first().unwrap().arguments).is_err());
    }

    #[test]
    fn unwrap_result_option() {
        let item: Path = syn::parse_quote! { bool };
        let (result, option, unwrapped) = super::unwrap_result_option(&item).unwrap();
        assert_eq!(
            (false, false, "bool".into()),
            (result, option, unwrapped.to_token_stream().to_string())
        );

        let item: Path = syn::parse_quote! { Option<bool> };
        let (result, option, unwrapped) = super::unwrap_result_option(&item).unwrap();
        assert_eq!(
            (false, true, "bool".into()),
            (result, option, unwrapped.to_token_stream().to_string())
        );

        let item: Path = syn::parse_quote! { Result<bool, Error> };
        let (result, option, unwrapped) = super::unwrap_result_option(&item).unwrap();
        assert_eq!(
            (true, false, "bool".into()),
            (result, option, unwrapped.to_token_stream().to_string())
        );

        let item: Path = syn::parse_quote! { Result<Option<bool>, Error> };
        let (result, option, unwrapped) = super::unwrap_result_option(&item).unwrap();
        assert_eq!(
            (true, true, "bool".into()),
            (result, option, unwrapped.to_token_stream().to_string())
        );
    }
}
