use syn::GenericArgument;
use syn::Lit;
use syn::LitStr;
use syn::Path;
use syn::PathArguments;
use syn::Type;

struct TypeLayerIter {
    cursor: Option<Path>,
}

impl TypeLayerIter {
    fn new(ty: Path) -> Self {
        Self { cursor: Some(ty) }
    }
}

impl Iterator for TypeLayerIter {
    type Item = Path;

    fn next(&mut self) -> Option<Self::Item> {
        match self.cursor.clone() {
            None => None,
            Some(cursor) => match cursor.segments.last() {
                None => {
                    self.cursor = None;
                    Some(Path {
                        leading_colon: None,
                        segments: Default::default(),
                    })
                }
                Some(current) => {
                    self.cursor = match &current.arguments {
                        PathArguments::AngleBracketed(arg) => match arg.args.first() {
                            None => None,
                            Some(arg) => {
                                if let GenericArgument::Type(ty) = arg {
                                    if let Ok(ty) = assert_type_is_path(&ty) {
                                        Some(ty)
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            }
                        },
                        _ => None,
                    };
                    Some(cursor.clone())
                }
            },
        }
    }
}

static WRAPPERS: &[&str] = &["Arc", "Option", "Result"];

pub fn unwrap_type(ty: syn::Path) -> Path {
    let default = Path {
        leading_colon: None,
        segments: Default::default(),
    };
    for layer in TypeLayerIter::new(ty) {
        if let Some(last_segment) = layer.segments.last() {
            if !WRAPPERS.contains(&last_segment.ident.to_string().as_str()) {
                return layer;
            }
        }
    }
    default
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

#[cfg(test)]
mod test {
    use super::*;
    use quote::ToTokens;

    #[test]
    fn type_layers() {
        fn run(path: Path) -> Vec<String> {
            TypeLayerIter::new(path)
                .map(|t| {
                    t.segments
                        .last()
                        .map(|segment| segment.ident.to_string())
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
        }

        let expected = vec!["Option", "bool"];
        let actual = run(syn::parse_quote! { Option<bool> });
        assert_eq!(expected, actual);

        let expected = vec!["Result", "bool"];
        let actual = run(syn::parse_quote! { Result<bool, Error> });
        assert_eq!(expected, actual);

        let expected = vec!["Result", "Option", "bool"];
        let actual = run(syn::parse_quote! { Result<Option<bool>, Error> });
        assert_eq!(expected, actual);

        let expected = vec!["Arc", "Result", "Option", "Love"];
        let actual = run(syn::parse_quote! { std::sync::Arc<Result<Option<Love>>, anyhow::Error> });
        assert_eq!(expected, actual);

        let expected = vec!["Result", ""];
        let actual = run(syn::parse_quote! { Result<(), Error> });
        assert_eq!(expected, actual);
    }

    #[test]
    fn unwrap_type() {
        fn run(path: Path) -> String {
            super::unwrap_type(path).into_token_stream().to_string()
        }

        let expected = "bool";
        let actual = run(syn::parse_quote! { Option<bool> });
        assert_eq!(expected, actual);

        let expected = "bool";
        let actual = run(syn::parse_quote! { Result<bool, Error> });
        assert_eq!(expected, actual);

        let expected = "bool";
        let actual = run(syn::parse_quote! { Result<Option<bool>, Error> });
        assert_eq!(expected, actual);

        let expected = "Love";
        let actual = run(syn::parse_quote! { std::sync::Arc<Result<Option<Love>>, anyhow::Error> });
        assert_eq!(expected, actual);

        let expected = "";
        let actual = run(syn::parse_quote! { Result<(), Error> });
        assert_eq!(expected, actual);

        let expected = "Vec < u8 >";
        let actual = run(syn::parse_quote! { Vec<u8> });
        assert_eq!(expected, actual);

        let expected = "Vec < u8 >";
        let actual = run(syn::parse_quote! { Option<Vec<u8>> });
        assert_eq!(expected, actual);
    }
}
