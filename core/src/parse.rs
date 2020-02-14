//! Syntax tree parsing.

use proc_macro2::TokenStream;
use quote::ToTokens;
use std::convert::TryFrom;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::marker::PhantomData;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;
use syn::Attribute;
use syn::AttributeArgs;
use syn::Ident;
use syn::Lit;
use syn::LitStr;
use syn::Meta;
use syn::MetaNameValue;
use syn::NestedMeta;
use syn::Path;
use syn::ReturnType;
use syn::Signature;
use syn::Token;
use syn::Type;
use syn::TypePath;

/// Arguments of a `#[riko::...]`.
pub trait Args: Sized {
    /// Full name of the attribute.
    const NAME: &'static [&'static str];

    /// Finds the first attribute that corresponds to this type and parses it.
    fn take_from<'a>(mut attrs: impl Iterator<Item = &'a Attribute>) -> syn::Result<Option<Self>> {
        let eq = |attr: &Attribute| {
            attr.path
                .segments
                .iter()
                .map(|t| t.to_token_stream().to_string())
                .eq_by(Self::NAME, |a, b| &a == b)
        };
        if let Some(attr) = attrs.find(|attr| eq(attr)) {
            Ok(Some(Self::parse(attr)?))
        } else {
            Ok(None)
        }
    }

    /// Parses the tokens of an [Attribute].
    ///
    /// Only parse the tokens after the attribute name. [take_from](Args::take_from) is responsible for checking if
    /// it is the right attribute.
    fn parse(attr: &Attribute) -> syn::Result<Self>;
}

/// State of an [Args].
///
/// An [Args] may contain optional fields that are filled during IR parsing. This type indicates if
/// an [Args] is fully expanded.
pub trait ArgsState {}

/// All fields are expanded.
pub struct Expanded;

impl ArgsState for Expanded {}

/// Freshly parsed from source code.
#[derive(Debug, PartialEq)]
pub struct Raw;

impl ArgsState for Raw {}

/// `#[riko::marshal]`
pub struct Marshal {
    /// Marshaling rule for a function parameter.
    ///
    /// To specify the rule for the return type, provide the `marshal` argument in
    /// `#[riko::fun]`.
    pub value: MarshalingRule,
}

impl Args for Marshal {
    const NAME: &'static [&'static str] = &["riko", "marshal"];

    fn parse(attr: &Attribute) -> syn::Result<Self> {
        let rule = if let Meta::NameValue(MetaNameValue {
            lit: Lit::Str(value),
            ..
        }) = attr.parse_meta()?
        {
            value.parse()?
        } else {
            return Err(syn::Error::new_spanned(
                attr.tokens.clone(),
                "Expect a marshaling rule",
            ));
        };
        Ok(Self { value: rule })
    }
}

/// `#[riko::fun]`.
///
/// All parameters are optional.
#[derive(Debug, PartialEq)]
pub struct Fun<S: ArgsState> {
    phantom: PhantomData<S>,

    /// Symbol name used when exporting the item, convenient for avoiding name clashes.
    pub name: String,

    /// Marshaling rule for the return type.
    ///
    /// To specify the rule for a parameter, use `#[riko::marshal]` on the parameter.
    pub marshal: Option<MarshalingRule>,
}

impl Fun<Raw> {
    /// Fills in all optional fields by consulting a function signature.
    pub fn expand_all_fields(self, sig: &Signature) -> syn::Result<Fun<Expanded>> {
        let marshal = if let ReturnType::Type(_, ty) = &sig.output {
            if self.marshal == None {
                Some(MarshalingRule::infer(ty)?)
            } else {
                self.marshal
            }
        } else {
            None
        };

        Ok(Fun::<Expanded> {
            phantom: Default::default(),
            marshal,
            name: self.name,
        })
    }
}

impl Default for Fun<Raw> {
    fn default() -> Self {
        Self {
            phantom: Default::default(),
            marshal: Default::default(),
            name: Default::default(),
        }
    }
}

impl Args for Fun<Raw> {
    const NAME: &'static [&'static str] = &["riko", "fun"];

    fn parse(attr: &Attribute) -> syn::Result<Self> {
        if attr.tokens.is_empty() {
            Ok(Default::default())
        } else {
            attr.parse_args()
        }
    }
}

impl TryFrom<AttributeArgs> for Fun<Raw> {
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

impl Parse for Fun<Raw> {
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
/// # Errors and Nullness
///
/// Unless specified, most of the rules work with their corresponding Rust types being wrapped
/// inside an [Option]. In the return position, wrapping the type in a [Result]
/// is also supported.
///
/// # References and Borrowed Types
///
/// Because the data is copied between FFI boundary and thus is always owned, support for references
/// and borrwoed types are limited.
///
/// References are supported For function parameters. However, the borrowed version of an owned type
/// is not supported (e.g. `&String` works but `&str` doesn't).
///
/// For returned types, only owned types are supported.
#[derive(PartialEq, Debug)]
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
    Struct(Assertable<Path>),

    /// Marshals a [String].
    String,

    /// `()`.
    Unit,
}

impl MarshalingRule {
    fn from_name(src: &Ident) -> syn::Result<Self> {
        match src.to_string().as_str() {
            "Bool" => Ok(Self::Bool),
            "Bytes" => Ok(Self::Bytes),
            "I8" => Ok(Self::I8),
            "I32" => Ok(Self::I32),
            "I64" => Ok(Self::I64),
            "Struct" => Ok(Self::Struct(Assertable(Path {
                leading_colon: None,
                segments: Punctuated::new(),
            }))),
            "String" => Ok(Self::String),
            "Unit" => Ok(Self::Unit),
            _ => Err(syn::Error::new_spanned(src, "Invalid marshaling rule")),
        }
    }

    /// The Rust type corresponding to the rule.
    pub fn rust_type(&self) -> Type {
        match self {
            Self::Bool => syn::parse_quote! { bool },
            Self::Bytes => syn::parse_quote! { ::serde_bytes::ByteBuf },
            Self::I8 => syn::parse_quote! { i8 },
            Self::I32 => syn::parse_quote! { i32 },
            Self::I64 => syn::parse_quote! { i64 },
            Self::Struct(Assertable(inner)) => Type::Path(TypePath {
                qself: None,
                path: inner.clone(),
            }),
            Self::String => syn::parse_quote! { ::std::string::String },
            Self::Unit => syn::parse_quote! { () },
        }
    }

    /// Infers the rule from the Rust source code when a rule is not specified.
    ///
    /// Since procedural macros can only analyse a syntax tree and have no access to any type
    /// information, it is impossible to always acurrately infer the rule. If the inference causes
    /// compiler errors or a type alias is used, specify the rule explicitly.
    ///
    /// If no other rules match the inference, `Struct` will be chosen by default.
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
                        || raw.starts_with(&format!("Result < {} , ", candidate))
                        || raw.starts_with(&format!("Result < Option < {} > , ", candidate))
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
        } else if Candidate::Primitive("( )").matches(&type_path_str) {
            Ok(Self::Unit)
        } else if Candidate::Struct(&["", "std", "string", "String"]).matches(&type_path_str) {
            Ok(Self::String)
        } else if Candidate::Struct(&["", "serde_bytes", "ByteBuf"]).matches(&type_path_str) {
            Ok(Self::Bytes)
        } else {
            Ok(Self::Struct(Assertable(type_path.clone())))
        }
    }
}

impl Parse for MarshalingRule {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        let mut result = Self::from_name(&name)?;
        if let Self::Struct(Assertable(ref mut inner)) = result {
            input.parse::<Token![<]>()?;
            *inner = input.parse()?;
            input.parse::<Token![>]>()?;
        }
        Ok(result)
    }
}

/// Wraps a [syn] type for unit tests.
///
/// Most [syn] types don't implement [Debug] or [PartialEq] which makes them unable to be used in
/// [assert_eq]. This type fixes the problem.
pub struct Assertable<T>(pub T);

impl<T> AsRef<T> for Assertable<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T: ToTokens> Debug for Assertable<T> {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self.0.to_token_stream().to_string())
    }
}

impl<T: ToTokens> PartialEq for Assertable<T> {
    fn eq(&self, other: &Self) -> bool {
        fn to_string<T: ToTokens>(a: &T) -> String {
            a.to_token_stream().to_string()
        }
        to_string(&self.0) == to_string(&other.0)
    }
}

impl<T: ToTokens> ToTokens for Assertable<T> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens)
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
        Err(syn::Error::new_spanned(src, "Expect a string literal"))
    }
}

#[allow(non_snake_case)]
mod tests {
    use super::*;

    #[test]
    fn Fun_parse() {
        let expected = Fun::<Raw> {
            phantom: Default::default(),
            name: "function2".to_owned(),
            marshal: Some(MarshalingRule::String),
        };
        let actual: Fun<Raw> = syn::parse_quote! {
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
            r#"Struct("org :: example :: Love")"#
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
            MarshalingRule::infer(&syn::parse_quote! { Result<ByteBuf, std::io::Error> }).unwrap(),
            MarshalingRule::Bytes
        );
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { Result<Option<ByteBuf>, std::io::Error> })
                .unwrap(),
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
            MarshalingRule::infer(&syn::parse_quote! {
                Result<serde_bytes::ByteBuf, std::io::Error>
            })
            .unwrap(),
            MarshalingRule::Bytes
        );
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! {
                Result<Option<serde_bytes::ByteBuf>, std::io::Error>
            })
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
            MarshalingRule::infer(
                &syn::parse_quote! { Result<Option<::serde_bytes::ByteBuf>, std::io::Error> }
            )
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
            MarshalingRule::infer(&syn::parse_quote! { Result<bool, std::io::Error> }).unwrap(),
            MarshalingRule::Bool
        );
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { Result<Option<bool>, std::io::Error> })
                .unwrap(),
            MarshalingRule::Bool
        );

        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { Result<(), Error> }).unwrap(),
            MarshalingRule::Unit
        );

        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { org::example::Love }).unwrap(),
            MarshalingRule::Struct(Assertable::<Path>(syn::parse_quote! { org::example::Love }))
        );
    }
}
