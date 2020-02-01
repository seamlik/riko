//! Syntax tree parsing.

use quote::ToTokens;
use std::convert::TryFrom;
use std::fmt::Debug;
use std::fmt::Formatter;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;
use syn::AttributeArgs;
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

/// Attributes for `#[fun]`.
#[derive(Default, Debug, PartialEq)]
pub struct Fun {
    /// Symbol name used when exporting the item, convenient for avoiding name clashes.
    pub name: String,

    /// Marshaling rule for the return type.
    ///
    /// To specify the rule for a parameter, use `#[riko::marshal]`.
    pub marshal: Option<MarshalingRule>,
}

impl Fun {
    /// Fills in all optional fields by consulting a function signature.
    pub fn expand_all_fields(&mut self, sig: &Signature) -> syn::Result<()> {
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
                    _ => return Err(syn::Error::new_spanned(pair.path, "Unrecognized argument")),
                },
                _ => return Err(syn::Error::new_spanned(arg, "Not a key-value")),
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

/// Specifies how to marshal the arguments and the returned value of a function across the FFI.
///
/// For now, the rules are a bit limiting (no unsigned integers, for example). This is
/// because we only want to make sure they work with all target languages (Java does not have
/// unsigned integers, for example).
///
/// # Rule Inference
///
/// When no rule is specified, the macros will try to guess the rule.
///
/// Since procedural macros can only analyse a syntax tree and have no access to any type
/// information, it is impossible to always acurrately infer the rule. If the inference causes
/// compiler errors or a type alias is used, specify the rule explicitly.
///
/// If no other rules match the inference, `Struct` will be chosen by default.
///
/// # Errors and Nullness
///
/// Unless specified, most of the rules work with their corresponding Rust types being wrapped
/// inside an [Option]. In the return position, wrapping the type in a [Result]
/// is also supported.
///
/// # References and borrowed Types
///
/// For function parameters, references are also supported. Unfortunately, the borrowed version of
/// a specific type is not supported (e.g. `&str` instead of `&String`), as that will prevent us
/// from benefiting from the compiler's type inference and will lose the support of
/// [Result] and [Option], which is of higher priority.
///
/// For returned types, only owned types are supported.
pub enum MarshalingRule {
    /// Marshals a boolean value.
    Bool,

    /// Marshals specifically a byte array instead of a collection of [u8].
    ///
    /// Only `ByteBuf` from [serde_bytes](https://crates.io/crates/serde_bytes) is supported for
    /// this rule.
    Bytes,

    /// Marshals an [i8].
    I8,

    /// Marshals an [i32].
    I32,

    /// Marshals an [i64].
    I64,

    /// Marshals custom types that support serialzation through [Serde](https://serde.rs).
    ///
    /// User must specify the marshaling rule in the form of `Struct<fully-qualified type path>`.
    /// Alternatively, one may obmit the rule and use the fully-qualified type path in the function
    /// signature.
    Struct(Path),

    /// Marshals a [String].
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
            "Struct" => Ok(Self::Struct(Path {
                leading_colon: None,
                segments: Punctuated::new(),
            })),
            "String" => Ok(Self::String),
            _ => Err(syn::Error::new_spanned(src, "Invalid marshaling rule")),
        }
    }

    pub fn to_rust_return_type(&self) -> Type {
        match self {
            Self::Bool => syn::parse_quote! { bool },
            Self::Bytes => syn::parse_quote! { ::serde_bytes::ByteBuf },
            Self::I8 => syn::parse_quote! { i8 },
            Self::I32 => syn::parse_quote! { i32 },
            Self::I64 => syn::parse_quote! { i64 },
            Self::Struct(inner) => Type::Path(TypePath {
                qself: None,
                path: inner.clone(),
            }),
            Self::String => syn::parse_quote! { ::std::string::String },
        }
    }

    pub fn infer(t: &Type) -> syn::Result<Self> {
        enum Candidate {
            Primitive(&'static str),
            /// To match the leading colons in `::std::string::String`, put an empty string at the
            /// first location.
            Struct(&'static [&'static str]),
        }
        impl Candidate {
            fn matches(&self, raw: &str) -> bool {
                let matches = |candidate: &str| {
                    raw == candidate
                        || raw == format!("Option < {} >", candidate)
                        || raw == format!("Result < Option < {} > >", candidate)
                };
                match self {
                    Self::Primitive(name) => matches(name),
                    Self::Struct(path) => {
                        matches(path.last().unwrap())
                            || matches(&path[1..].join(" :: "))
                            || matches(&path.join(" :: ").trim())
                    }
                }
            }
        }

        let type_path = match t {
            Type::Reference(reference) => assert_type_is_path(&reference.elem),
            _ => assert_type_is_path(t),
        }?;
        let type_path_str = type_path.segments.to_token_stream().to_string();

        if Candidate::Primitive("bool").matches(&type_path_str) {
            Ok(Self::Bool)
        } else if Candidate::Primitive("i32").matches(&type_path_str) {
            Ok(Self::I32)
        } else if Candidate::Primitive("i64").matches(&type_path_str) {
            Ok(Self::I64)
        } else if Candidate::Primitive("i8").matches(&type_path_str) {
            Ok(Self::I8)
        } else if Candidate::Struct(&["", "std", "string", "String"]).matches(&type_path_str) {
            Ok(Self::String)
        } else if Candidate::Struct(&["", "serde_bytes", "ByteBuf"]).matches(&type_path_str) {
            Ok(Self::Bytes)
        } else {
            Ok(Self::Struct(type_path.clone()))
        }
    }
}

impl Parse for MarshalingRule {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        let mut result = Self::from_name(&name)?;
        if let Self::Struct(ref mut inner) = result {
            input.parse::<Token![<]>()?;
            *inner = input.parse()?;
            input.parse::<Token![>]>()?;
        }
        Ok(result)
    }
}

impl PartialEq for MarshalingRule {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Struct(left), Self::Struct(right)) => {
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
            Self::Struct(inner) => write!(f, "Struct {{ {} }}", inner.to_token_stream()),
            Self::String => write!(f, "String"),
        }
    }
}

fn assert_type_is_path(src: &Type) -> syn::Result<&Path> {
    if let Type::Path(type_path) = src {
        Ok(&type_path.path)
    } else {
        Err(syn::Error::new_spanned(src, "Expected a type path"))
    }
}

fn assert_lit_is_litstr(src: &Lit) -> syn::Result<&LitStr> {
    if let Lit::Str(litstr) = src {
        Ok(litstr)
    } else {
        Err(syn::Error::new_spanned(src, "Invalid value"))
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
        };
        let actual: Fun = syn::parse_quote! {
            name = "function2",
            marshal = "String",
        };
        assert_eq!(expected, actual);
    }

    #[test]
    fn MarshalingRule_parse_rule() {
        assert_eq!(
            format!(
                "{:?}",
                syn::parse_str::<MarshalingRule>("Struct<org::example::Love>").unwrap()
            ),
            "Struct { org :: example :: Love }"
        );
        assert_eq!(
            format!("{:?}", syn::parse_str::<MarshalingRule>("String").unwrap()),
            "String"
        );
        syn::parse_str::<MarshalingRule>("Struct").unwrap_err();
    }

    #[test]
    fn MarshalingRule_infer() {
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { ByteBuf }).unwrap(),
            MarshalingRule::Bytes
        );
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { Option<ByteBuf> }).unwrap(),
            MarshalingRule::Bytes
        );
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { Result<Option<ByteBuf>> }).unwrap(),
            MarshalingRule::Bytes
        );
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { serde_bytes::ByteBuf }).unwrap(),
            MarshalingRule::Bytes
        );
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { Option<serde_bytes::ByteBuf> }).unwrap(),
            MarshalingRule::Bytes
        );
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { Result<Option<serde_bytes::ByteBuf>> })
                .unwrap(),
            MarshalingRule::Bytes
        );
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { ::serde_bytes::ByteBuf }).unwrap(),
            MarshalingRule::Bytes
        );
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { Option<::serde_bytes::ByteBuf> }).unwrap(),
            MarshalingRule::Bytes
        );
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { Result<Option<::serde_bytes::ByteBuf>> })
                .unwrap(),
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
            MarshalingRule::infer(&syn::parse_quote! { Option<bool> }).unwrap(),
            MarshalingRule::Bool
        );
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { Result<Option<bool>> }).unwrap(),
            MarshalingRule::Bool
        );
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { org::example::Love }).unwrap(),
            MarshalingRule::Struct(syn::parse_quote! { org::example::Love })
        );
    }
}
