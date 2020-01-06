//! Syntax tree parsing.

use blake2::digest::Input;
use blake2::digest::VariableOutput;
use blake2::VarBlake2b;
use data_encoding::HEXLOWER;
use quote::ToTokens;
use std::convert::TryFrom;
use std::fmt::Debug;
use std::fmt::Formatter;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;
use syn::AttributeArgs;
use syn::FnArg;
use syn::Ident;
use syn::ItemFn;
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

/// Generates a mangled function name to be invoked by target code.
pub fn mangle_function_name(function: &ItemFn) -> Ident {
    let mut function_without_attr = function.clone();
    function_without_attr.attrs.clear();

    let body = function_without_attr.into_token_stream().to_string();
    let mut body_hasher = VarBlake2b::new(16).unwrap();
    body_hasher.input(body.as_bytes());
    let body_hash = HEXLOWER.encode(&body_hasher.vec_result());

    quote::format_ident!("__riko_{}_{}", function.sig.ident.to_string(), body_hash)
}

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
        if self.name.is_empty() {
            self.name = sig.ident.to_string();
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
/// If no other rules match the inference, `Serde` will be chosen by default.
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

    /// Marshals a byte array, namely `Vec<u8>`.
    ///
    /// This rule exists because it is commonly used.
    Bytes,

    /// Marshals an [i8].
    I8,

    /// Marshals an [i32].
    I32,

    /// Marshals an [i64].
    I64,

    /// Marshals an [Iterator].
    ///
    /// It exists because it is a performance issue to marshal a very large byte array across the
    /// FFI. Another reason is that some libraries provides event subscriptions in the form of
    /// [Iterator]s instead of `Stream`s.
    ///
    /// For now, it only supports [Iterator]s with a static lifetime. The practicality of this rule
    /// with doubt, hence temporarily deprecated.
    ///
    /// User must specify the item type in the rule in the form of `Iterator<X>`.
    ///
    /// Due to technical difficulties, this rule only supports marshaling an [Iterator] wrapped in a
    /// [Box] or a [Result]. See `riko_runtime::iterators::IntoReturned` for explanation.
    Iterator(String),

    /// Marshals custom types that support serialzation through [Serde](https://serde.rs).
    ///
    /// User must specify the marshaling rule in the form of `Serde<fully-qualified type path>`.
    /// Alternatively, one may obmit the rule and use the fully-qualified type path in the function
    /// signature.
    Serde(Path),

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
            "Iterator" => Ok(Self::Iterator(String::default())),
            "Serde" => Ok(Self::Serde(Path {
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
        // TODO: Smarter inference with a table

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
