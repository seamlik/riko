//! Syntax tree parsing.

use quote::ToTokens;
use std::convert::TryFrom;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::result::Result;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::AttributeArgs;
use syn::GenericArgument;
use syn::Ident;
use syn::Lit;
use syn::LitStr;
use syn::Meta;
use syn::NestedMeta;
use syn::ParenthesizedGenericArguments;
use syn::Path;
use syn::PathArguments;
use syn::PathSegment;
use syn::ReturnType;
use syn::Token;
use syn::Type;
use syn::TypePath;

#[derive(Default, Debug, PartialEq)]
pub struct MarshalingSignature {
    pub inputs: Vec<MarshalingRule>,
    pub output: Option<MarshalingRule>,
}

impl MarshalingSignature {
    pub fn has_iterators(&self) -> bool {
        if let Some(MarshalingRule::Iterator(_)) = self.output {
            true
        } else {
            self.inputs.iter().any(|rule| {
                if let MarshalingRule::Iterator(_) = rule {
                    true
                } else {
                    false
                }
            })
        }
    }
}

impl Parse for MarshalingSignature {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let sig_raw = input.parse::<ParenthesizedGenericArguments>()?;

        let mut inputs = Vec::<MarshalingRule>::new();
        for it in sig_raw.inputs {
            inputs.push(MarshalingRule::from_type(&it)?);
        }

        let output = match &sig_raw.output {
            ReturnType::Default => None,
            ReturnType::Type(_, r#type) => {
                let candidate = MarshalingRule::from_type(&r#type)?;
                match &candidate {
                    MarshalingRule::Serde(inner) if inner.segments.is_empty() => {
                        return Err(syn::Error::new(
                            sig_raw.output.span(),
                            "Must specify the type for `Serde` in the return position.",
                        ));
                    }
                    _ => Some(candidate),
                }
            }
        };

        Ok(Self { inputs, output })
    }
}

/// Represents a `#[fun]`.
#[derive(Default, Debug, PartialEq)]
pub struct Fun {
    pub name: String,
    pub sig: MarshalingSignature,
}

impl TryFrom<AttributeArgs> for Fun {
    type Error = syn::Error;

    fn try_from(value: AttributeArgs) -> Result<Self, Self::Error> {
        let mut result = Fun::default();
        for arg in value {
            match arg {
                NestedMeta::Meta(Meta::NameValue(pair)) => match pair.path.get_ident() {
                    Some(name) if name == "name" => {
                        result.name = assert_lit_is_litstr(&pair.lit)?.value();
                    }
                    Some(name) if name == "sig" => {
                        result.sig = assert_lit_is_litstr(&pair.lit)?.parse()?
                    }

                    _ => return Err(syn::Error::new(pair.path.span(), "Unrecognized argument.")),
                },
                _ => return Err(syn::Error::new(arg.span(), "Not a key-value.")),
            }
        }
        Ok(result)
    }
}

impl Parse for Fun {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let args: AttributeArgs = Punctuated::<NestedMeta, Token![,]>::parse_terminated(input)?
            .into_iter()
            .collect();
        Self::try_from(args)
    }
}

pub enum MarshalingRule {
    Bool,
    Bytes,
    I8,
    I32,
    I64,
    Iterator(String),
    Serde(Path),
    String,
}

const ERROR_MARSHALING_RULE_IMPURE: &str = "Must not add any other decoration to the type.";
const ERROR_MARSHALING_RULE_INVALID: &str = "Must be one of the variants of `MarshalingRule`.";
const ERROR_MARSHALING_RULE_BAD_NUMBER_OF_TYPE_ARGS: &str = "Too many type arguments.";

impl MarshalingRule {
    fn from_name_without_inner(src: &Ident) -> syn::Result<Self> {
        match src.to_string().as_str() {
            "Bool" => Ok(Self::Bool),
            "Bytes" => Ok(Self::Bytes),
            "I8" => Ok(Self::I8),
            "I32" => Ok(Self::I32),
            "I64" => Ok(Self::I64),
            "Iterator" => Ok(Self::Iterator(String::default())),
            "Serde" => Ok(Self::Serde(Path {
                leading_colon: None,
                segments: Punctuated::new(),
            })),
            "String" => Ok(Self::String),
            _ => Err(syn::Error::new(
                src.span(),
                format!("Invalid marshaling rule: {}", src.to_token_stream()),
            )),
        }
    }

    fn from_type(src: &Type) -> syn::Result<Self> {
        let segment = assert_type_no_prefix(src)?;
        Self::from_pathsegment(segment)
    }

    fn from_pathsegment(src: &PathSegment) -> syn::Result<Self> {
        let error = || {
            Err(syn::Error::new(
                src.span(),
                format!(
                    "Must specify the type for this rule: {}",
                    src.to_token_stream()
                ),
            ))
        };
        let mut result = Self::from_name_without_inner(&src.ident)?;
        if let Self::Serde(ref mut inner) = result {
            if let Some(path) = assert_patharguments_is_path(&src.arguments)? {
                *inner = path.clone()
            } else {
                return error();
            }
        } else if let Self::Iterator(ref mut inner) = result {
            if let Some(ident) = assert_patharguments_clean(&src.arguments)? {
                *inner = ident.to_string();
            } else {
                return error();
            }
        }
        Ok(result)
    }

    pub fn to_rust_return_type(&self) -> Type {
        match self {
            Self::Bool => syn::parse_quote! { bool },
            Self::Bytes => syn::parse_quote! { ::std::vec::Vec<u8> },
            Self::I8 => syn::parse_quote! { i8 },
            Self::I32 => syn::parse_quote! { i32 },
            Self::I64 => syn::parse_quote! { i64 },
            Self::Iterator(_) => syn::parse_quote! { _ },
            Self::Serde(inner) => Type::Path(TypePath {
                qself: None,
                path: inner.clone(),
            }),
            Self::String => syn::parse_quote! { ::std::string::String },
        }
    }

impl PartialEq for MarshalingRule {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bool, Self::Bool) => true,
            (Self::Bytes, Self::Bytes) => true,
            (Self::I32, Self::I32) => true,
            (Self::I64, Self::I64) => true,
            (Self::I8, Self::I8) => true,
            (Self::Iterator(left), Self::Iterator(right)) => left == right,
            (Self::String, Self::String) => true,
            (Self::Serde(left), Self::Serde(right)) => {
                left.to_token_stream().to_string() == right.to_token_stream().to_string()
            }
            _ => false,
        }
    }
}

impl Debug for MarshalingRule {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            Self::Bool => write!(f, "Bool"),
            Self::Bytes => write!(f, "Bytes"),
            Self::I8 => write!(f, "I8"),
            Self::I32 => write!(f, "I32"),
            Self::I64 => write!(f, "I64"),
            Self::Iterator(inner) => write!(f, "Iterator {{ {:?} }}", inner),
            Self::Serde(inner) => write!(f, "Serde {{ {:?} }}", inner.to_token_stream()),
            Self::String => write!(f, "String"),
        }
    }
}

/// Asserts a [Type] contains only 1 segment (e.g. `Foo<Bar>` instead of `some::Foo<Bar>`).
fn assert_type_no_prefix(src: &Type) -> syn::Result<&PathSegment> {
    if let Type::Path(path) = src {
        if path.qself.is_some() {
            // Self prefix
            Err(syn::Error::new(src.span(), ERROR_MARSHALING_RULE_IMPURE))
        } else if let Some(colons) = path.path.leading_colon {
            // Leading colons
            Err(syn::Error::new(colons.span(), ERROR_MARSHALING_RULE_IMPURE))
        } else if path.path.segments.len() != 1 {
            // 1 segment only
            Err(syn::Error::new(
                path.path.segments.span(),
                ERROR_MARSHALING_RULE_INVALID,
            ))
        } else {
            Ok(path.path.segments.first().unwrap())
        }
    } else {
        Err(syn::Error::new(src.span(), ERROR_MARSHALING_RULE_IMPURE))
    }
}

/// Asserts a [Type] is in the simplest form (like `Foo`) and returns it as an [Ident].
fn assert_type_clean(src: &Type) -> syn::Result<&Ident> {
    let segment = assert_type_no_prefix(src)?;
    if segment.arguments.is_empty() {
        Ok(&segment.ident)
    } else {
        Err(syn::Error::new(
            segment.span(),
            ERROR_MARSHALING_RULE_IMPURE,
        ))
    }
}

/// Asserts `<XXX>` contains a type of the simplest form (e.g. `Foo`).
fn assert_patharguments_clean(src: &PathArguments) -> syn::Result<Option<&Ident>> {
    match &src {
        PathArguments::None => Ok(None),
        PathArguments::AngleBracketed(ref args) => {
            if let Some(colon) = &args.colon2_token {
                Err(syn::Error::new(colon.span(), ERROR_MARSHALING_RULE_IMPURE))
            } else if args.args.len() != 1 {
                Err(syn::Error::new(
                    args.args.span(),
                    ERROR_MARSHALING_RULE_BAD_NUMBER_OF_TYPE_ARGS,
                ))
            } else {
                let first_arg = args.args.first().unwrap();
                if let GenericArgument::Type(first_arg_type) = first_arg {
                    Ok(Some(assert_type_clean(&first_arg_type)?))
                } else {
                    Err(syn::Error::new(
                        first_arg.span(),
                        ERROR_MARSHALING_RULE_IMPURE,
                    ))
                }
            }
        }
        _ => Err(syn::Error::new(src.span(), ERROR_MARSHALING_RULE_IMPURE)),
    }
}

// Asserts `<XXX>` contains exactly a [Path].
fn assert_patharguments_is_path(src: &PathArguments) -> syn::Result<Option<&Path>> {
    let error = || {
        Err(syn::Error::new(
            src.span(),
            format!("Invalid path arguments: {}", src.to_token_stream()),
        ))
    };
    match src {
        PathArguments::None => Ok(None),
        PathArguments::AngleBracketed(inner) => {
            if inner.colon2_token.is_some() || inner.args.len() != 1 {
                error()
            } else if let GenericArgument::Type(Type::Path(path)) = inner.args.first().unwrap() {
                if path.qself.is_some() {
                    error()
                } else {
                    Ok(Some(&path.path))
                }
            } else {
                error()
            }
        }
        _ => error(),
    }
}

fn assert_lit_is_litstr(src: &Lit) -> syn::Result<&LitStr> {
    if let Lit::Str(litstr) = src {
        Ok(litstr)
    } else {
        Err(syn::Error::new(src.span(), "Invalid value."))
    }
}

#[allow(non_snake_case)]
mod tests {
    use super::*;

    #[test]
    fn MarshalingSignature_full() {
        let actual: MarshalingSignature = syn::parse_str("(String, String) -> String").unwrap();
        assert_eq!(
            actual.inputs,
            vec![MarshalingRule::String, MarshalingRule::String]
        );
        assert_eq!(actual.output, Some(MarshalingRule::String));
    }

    #[test]
    fn MarshalingSignature_no_return() {
        let actual = syn::parse_str::<MarshalingSignature>("(String, String)").unwrap();
        assert_eq!(
            actual.inputs,
            vec![MarshalingRule::String, MarshalingRule::String]
        );
        assert!(actual.output.is_none());
    }

    #[test]
    fn MarshalingSignature_serde() {
        let actual = syn::parse_str::<MarshalingSignature>(
            "(Serde<org::example::Love>) -> Serde<org::example::Life>",
        )
        .unwrap();
        assert_eq!(
            actual.inputs,
            vec![MarshalingRule::Serde(
                syn::parse_quote! { org::example::Love }
            )]
        );
        assert_eq!(
            actual.output,
            Some(MarshalingRule::Serde(
                syn::parse_quote! { org::example::Life }
            ))
        );
    }

    #[test]
    fn MarshalingSignature_iterator() {
        let actual = syn::parse_str::<MarshalingSignature>("() -> Iterator<String>").unwrap();
        assert!(actual.inputs.is_empty());
        assert_eq!(
            actual.output,
            Some(MarshalingRule::Iterator("String".to_owned()))
        );
    }

    #[test]
    fn MarshalingSignature_no_args() {
        let actual = syn::parse_str::<MarshalingSignature>("() -> Bytes").unwrap();
        assert!(actual.inputs.is_empty());
        assert_eq!(actual.output, Some(MarshalingRule::Bytes));
    }

    #[test]
    fn MarshalingSignature_nothing() {
        let actual = syn::parse_str::<MarshalingSignature>("()").unwrap();
        assert!(actual.inputs.is_empty());
        assert_eq!(actual.output, None);
    }

    #[test]
    fn MarshalingSignature_invalid() {
        syn::parse_str::<MarshalingSignature>("() -> &Bytes").unwrap_err();
        syn::parse_str::<MarshalingSignature>("() -> ::Bytes").unwrap_err();
        syn::parse_str::<MarshalingSignature>("() -> std::Bytes").unwrap_err();
        syn::parse_str::<MarshalingSignature>("() -> ").unwrap_err();
    }

    #[test]
    fn MarshalingSignature_has_iterators() {
        assert!(
            syn::parse_str::<MarshalingSignature>("() -> Iterator<String>")
                .unwrap()
                .has_iterators()
        );
        assert!(!syn::parse_str::<MarshalingSignature>("() -> String")
            .unwrap()
            .has_iterators());
    }

    #[test]
    fn Fun_parse() {
        let expected = Fun {
            name: "function2".to_owned(),
            sig: MarshalingSignature {
                inputs: vec![MarshalingRule::I32],
                output: Some(MarshalingRule::I32),
            },
        };
        let actual: Fun = syn::parse_quote! {
            sig = "(I32) -> I32",
            name = "function2"
        };
        assert_eq!(expected, actual);
    }
}
