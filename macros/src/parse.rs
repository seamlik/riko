//! Syntax tree parsing.

use strum_macros::EnumString;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::GenericArgument;
use syn::Ident;
use syn::LitStr;
use syn::ParenthesizedGenericArguments;
use syn::PathArguments;
use syn::PathSegment;
use syn::ReturnType;
use syn::Token;
use syn::Type;

/// Key-value parameters for a macro.
struct KeyValue {
    // TODO: Use `AttributeArgs`
    pub key: Ident,
    pub value: LitStr,
}

impl Parse for KeyValue {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let key = input.parse::<Ident>()?;
        input.parse::<Token![=]>()?;
        let value = input.parse::<LitStr>()?;
        Ok(KeyValue { key, value })
    }
}

#[derive(Default, Debug, PartialEq)]
pub struct MarshalingSignature {
    pub inputs: Vec<MarshalingRule>,
    pub output: Option<MarshalingRule>,
}

impl Parse for MarshalingSignature {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let sig_raw = input.parse::<ParenthesizedGenericArguments>()?;

        let mut inputs = Vec::<MarshalingRule>::new();
        for it in sig_raw.inputs {
            inputs.push(MarshalingRule::from_type(&it)?);
        }

        let output = match sig_raw.output {
            ReturnType::Default => None,
            ReturnType::Type(_, r#type) => Some(MarshalingRule::from_type(&r#type)?),
        };

        Ok(Self { inputs, output })
    }
}

#[derive(Default, Debug, PartialEq)]
pub struct Fun {
    pub name: String,
    pub sig: MarshalingSignature,
}

impl Parse for Fun {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut result = Fun::default();
        for arg in Punctuated::<KeyValue, Token![,]>::parse_terminated(input)? {
            let key = arg.key.to_string();
            if key == "name" {
                result.name = arg.value.value();
            } else if key == "sig" {
                result.sig = arg.value.parse()?;
            } else {
                return Err(syn::Error::new(arg.key.span(), "Invalid argument."));
            }
        }
        Ok(result)
    }
}

#[derive(EnumString, Debug, PartialEq)]
pub enum MarshalingRule {
    Bytes,
    I8,
    I32,
    I64,
    Serde(String),
    String,
}

const ERROR_MARSHALING_RULE_IMPURE: &str = "Must not add any other decoration to the type.";
const ERROR_MARSHALING_RULE_INVALID: &str = "Must be one of the variants of `MarshalingRule`.";
const ERROR_MARSHALING_RULE_BAD_NUMBER_OF_TYPE_ARGS: &str = "Too many type arguments.";

impl MarshalingRule {
    fn from_type(src: &Type) -> syn::Result<Self> {
        let segment = assert_type_no_prefix(src)?;
        Self::from_pathsegment(segment)
    }

    fn from_pathsegment(src: &PathSegment) -> syn::Result<Self> {
        let mut result = src
            .ident
            .to_string()
            .parse()
            .map_err(|err: strum::ParseError| syn::Error::new(src.span(), err.to_string()))?;
        if let MarshalingRule::Serde(ref mut serde_inner) = result {
            *serde_inner = assert_patharguments_clean(&src.arguments)?.to_string();
        }
        Ok(result)
    }
}

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

fn assert_patharguments_clean(src: &PathArguments) -> syn::Result<String> {
    match &src {
        PathArguments::None => Ok(String::default()),
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
                    Ok(assert_type_clean(&first_arg_type)?.to_string())
                } else {
                    Err(syn::Error::new(
                        first_arg.span(),
                        ERROR_MARSHALING_RULE_IMPURE,
                    ))
                }
            }
        }
        _ => return Err(syn::Error::new(src.span(), ERROR_MARSHALING_RULE_IMPURE)),
    }
}

#[allow(non_snake_case)]
mod tests {
    use super::*;

    #[test]
    fn KeyValue_parse() {
        let actual = syn::parse_str::<KeyValue>(r#"love = "forever""#).unwrap();
        assert_eq!((actual.key.to_string()), "love");
        assert_eq!(actual.value.value(), "forever");
    }

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
        let actual = syn::parse_str::<MarshalingSignature>("(Serde) -> Serde<Info>").unwrap();
        assert_eq!(
            actual.inputs,
            vec![MarshalingRule::Serde(String::default())]
        );
        assert_eq!(
            actual.output,
            Some(MarshalingRule::Serde("Info".to_owned()))
        );
    }

    #[test]
    fn MarshalingSignature_no_args() {
        let actual = syn::parse_str::<MarshalingSignature>("() -> Bytes").unwrap();
        assert!(actual.inputs.is_empty());
        assert_eq!(actual.output, Some(MarshalingRule::Bytes));
    }

    #[test]
    fn Marshaling_Signature_nothing() {
        let actual = syn::parse_str::<MarshalingSignature>("()").unwrap();
        assert!(actual.inputs.is_empty());
        assert_eq!(actual.output, None);
    }

    #[test]
    fn Marshaling_Signature_invalid() {
        syn::parse_str::<MarshalingSignature>("() -> &Bytes").unwrap_err();
        syn::parse_str::<MarshalingSignature>("() -> ::Bytes").unwrap_err();
        syn::parse_str::<MarshalingSignature>("() -> std::Bytes").unwrap_err();
        syn::parse_str::<MarshalingSignature>("() -> ").unwrap_err();
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
        let input = r#"
            sig = "(I32) -> I32",
            name = "function2"
        "#;
        let actual = syn::parse_str::<Fun>(input).unwrap();
        assert_eq!(expected, actual);
    }
}
