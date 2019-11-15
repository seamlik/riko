//! Syntax tree parsing.

use crate::config::Config;
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
use syn::FnArg;
use syn::Ident;
use syn::Lit;
use syn::LitStr;
use syn::Meta;
use syn::NestedMeta;
use syn::Path;
use syn::ReturnType;
use syn::Signature;
use syn::Token;
use syn::Type;
use syn::TypePath;

/// Represents a `#[fun]`.
#[derive(Default, Debug, PartialEq)]
pub struct Fun {
    pub module: Vec<String>,
    pub name: String,
    pub marshal: Option<MarshalingRule>,
}

impl Fun {
    /// Fills in all optional fields by consulting a function signature.
    pub fn expand_all_fields(&mut self, sig: &Signature, config: &Config) -> syn::Result<()> {
        if self.name.is_empty() {
            self.name = sig.ident.to_string();
        }
        if self.module.is_empty() {
            self.module = config.guess_module_by_span(sig.span())?;
        }
        if let ReturnType::Type(_, ty) = &sig.output {
            if self.marshal == None {
                self.marshal = Some(MarshalingRule::infer(ty)?);
            }
        }

        Ok(())
    }
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
                    Some(name) if name == "marshal" => {
                        result.marshal = Some(assert_lit_is_litstr(&pair.lit)?.parse()?);
                    }
                    Some(name) if name == "module" => {
                        result.module = assert_lit_is_litstr(&pair.lit)?
                            .value()
                            .split("::")
                            .map(Into::into)
                            .collect();
                    }
                    _ => return Err(syn::Error::new_spanned(pair.path, "Unrecognized argument.")),
                },
                _ => return Err(syn::Error::new_spanned(arg, "Not a key-value.")),
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

impl MarshalingRule {
    fn from_name(src: &Ident) -> syn::Result<Self> {
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
            _ => Err(syn::Error::new_spanned(src, "Invalid marshaling rule.")),
        }
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

    fn infer(t: &Type) -> syn::Result<Self> {
        let type_path = match t {
            Type::Reference(reference) => assert_type_is_path(&reference.elem),
            _ => assert_type_is_path(t),
        }?;
        let type_path_no_leading_colons = type_path.segments.to_token_stream().to_string();

        match type_path_no_leading_colons.as_str() {
            "bool" => Ok(Self::Bool),
            "i32" => Ok(Self::I32),
            "i64" => Ok(Self::I64),
            "i8" => Ok(Self::I8),
            "std :: string :: String" | "String" => Ok(Self::String),
            "std :: vec :: Vec < u8 >" | "Vec < u8 >" => Ok(Self::Bytes),
            _ => Ok(Self::Serde(type_path.clone())),
            // TODO: Result & Option
            // TODO: Iterator
        }
    }

    /// Generates a list of [MarshalingRule]s based on the helper attributes `#[riko::marshal]`
    /// applied on `params` and removes those attributes during the process.
    ///
    /// Only processes the first matching attribute.
    pub fn parse<'a>(params: impl Iterator<Item = &'a mut FnArg>) -> syn::Result<Vec<Self>> {
        let mut result = Vec::<Self>::new();
        for it in params {
            if let FnArg::Typed(ref mut inner) = it {
                let marshal_attr = inner
                    .attrs
                    .drain_filter(|attr| {
                        attr.path.to_token_stream().to_string() == "riko :: marshal"
                    })
                    .nth(0);
                if let Some(attr) = marshal_attr {
                    result.push(syn::parse2::<MarshalAttrArgs>(attr.tokens)?.rule);
                } else {
                    result.push(MarshalingRule::infer(&inner.ty)?);
                }
            } else {
                todo!("`#[fun]` on a method not implemented");
            }
        }
        Ok(result)
    }
}

impl Parse for MarshalingRule {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        let mut result = Self::from_name(&name)?;
        match result {
            Self::Serde(ref mut inner) => {
                input.parse::<Token![<]>()?;
                *inner = input.parse()?;
                input.parse::<Token![>]>()?;
            }
            Self::Iterator(ref mut inner) => {
                input.parse::<Token![<]>()?;
                *inner = input.parse::<Ident>()?.to_string();
                input.parse::<Token![>]>()?;
            }
            _ => (),
        }
        Ok(result)
    }
}

impl PartialEq for MarshalingRule {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Iterator(left), Self::Iterator(right)) => left == right,
            (Self::Serde(left), Self::Serde(right)) => {
                left.to_token_stream().to_string() == right.to_token_stream().to_string()
            }
            _ => std::mem::discriminant(self) == std::mem::discriminant(other),
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
            Self::Serde(inner) => write!(f, "Serde {{ {} }}", inner.to_token_stream()),
            Self::String => write!(f, "String"),
        }
    }
}

struct MarshalAttrArgs {
    rule: MarshalingRule,
}

impl Parse for MarshalAttrArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        syn::parenthesized!(content in input);
        Ok(Self {
            rule: <MarshalingRule as Parse>::parse(&content)?,
        })
    }
}

fn assert_type_is_path(src: &Type) -> syn::Result<&Path> {
    if let Type::Path(type_path) = src {
        Ok(&type_path.path)
    } else {
        Err(syn::Error::new_spanned(src, "Expected a type path."))
    }
}

fn assert_lit_is_litstr(src: &Lit) -> syn::Result<&LitStr> {
    if let Lit::Str(litstr) = src {
        Ok(litstr)
    } else {
        Err(syn::Error::new_spanned(src, "Invalid value."))
    }
}

#[allow(non_snake_case)]
mod tests {
    use super::*;

    #[test]
    fn Fun_parse() {
        let expected = Fun {
            name: "function2".to_owned(),
            marshal: Some(MarshalingRule::String),
            module: vec!["org".into(), "example".into()],
        };
        let actual: Fun = syn::parse_quote! {
            name = "function2",
            marshal = "String",
            module = "org::example"
        };
        assert_eq!(expected, actual);
    }

    #[test]
    fn MarshalingRule_parse_rule() {
        assert_eq!(
            format!(
                "{:?}",
                syn::parse_str::<MarshalingRule>("Serde<org::example::Love>").unwrap()
            ),
            "Serde { org :: example :: Love }"
        );
        assert_eq!(
            format!("{:?}", syn::parse_str::<MarshalingRule>("String").unwrap()),
            "String"
        );
        syn::parse_str::<MarshalingRule>("Serde").unwrap_err();
    }

    #[test]
    fn MarshalingRule_infer() {
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { Vec<u8> }).unwrap(),
            MarshalingRule::Bytes
        );
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { std::vec::Vec<u8> }).unwrap(),
            MarshalingRule::Bytes
        );
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { ::std::vec::Vec<u8> }).unwrap(),
            MarshalingRule::Bytes
        );

        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { String }).unwrap(),
            MarshalingRule::String
        );
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { std::string::String }).unwrap(),
            MarshalingRule::String
        );
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { ::std::string::String }).unwrap(),
            MarshalingRule::String
        );

        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { bool }).unwrap(),
            MarshalingRule::Bool
        );
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { org::example::Love }).unwrap(),
            MarshalingRule::Serde(syn::parse_quote! { org::example::Love })
        );
    }

    #[test]
    fn MarshalingRule_parse_params() {
        let mut actual_params: Punctuated<FnArg, Token![,]> = vec![
            syn::parse_str::<FnArg>("a: String").unwrap(),
            syn::parse_quote! { #[riko::marshal(String)] b: Option<String> },
        ]
        .into_iter()
        .collect();

        let expected_params: Punctuated<FnArg, Token![,]> = vec![
            syn::parse_str::<FnArg>("a: String").unwrap(),
            syn::parse_quote! { b: Option<String> },
        ]
        .into_iter()
        .collect();

        let actual_rules = MarshalingRule::parse(actual_params.iter_mut()).unwrap();
        let expected_rules = [MarshalingRule::String, MarshalingRule::String];

        assert_eq!(
            actual_params.to_token_stream().to_string(),
            expected_params.to_token_stream().to_string()
        );
        assert_eq!(actual_rules, expected_rules);
    }
}
