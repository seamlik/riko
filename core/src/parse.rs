//! Syntax tree parsing.

use crate::ir::MarshalingRule;
use quote::ToTokens;
use std::convert::TryFrom;
use std::fmt::Debug;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;
use syn::Attribute;
use syn::AttributeArgs;
use syn::Lit;
use syn::Meta;
use syn::MetaNameValue;
use syn::NestedMeta;
use syn::Token;

/// Arguments of a `#[riko::…]`.
pub(crate) trait Args: Sized {
    /// Full name of the attribute.
    const NAME: &'static [&'static str];

    /// Looks for the first attribute that corresponds to this type and parses it.
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
            value
                .value()
                .parse()
                .map_err(|_| syn::Error::new_spanned(value, "No such marshaling rule"))?
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
#[derive(Debug, PartialEq, Default)]
pub struct Fun {
    /// Symbol name used when exporting the item, convenient for avoiding name clashes.
    pub name: String,

    /// Marshaling rule for the return type.
    ///
    /// To specify the rule for a parameter, use `#[riko::marshal]` on the parameter.
    pub marshal: Option<MarshalingRule>,
}

impl Args for Fun {
    const NAME: &'static [&'static str] = &["riko", "fun"];

    fn parse(attr: &Attribute) -> syn::Result<Self> {
        if attr.tokens.is_empty() {
            Ok(Default::default())
        } else {
            attr.parse_args()
        }
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
                        result.name = crate::util::assert_lit_is_litstr(&pair.lit)?.value();
                    }
                    Some(name) if name == "marshal" => {
                        let literal = crate::util::assert_lit_is_litstr(&pair.lit)?;
                        result.marshal = Some(literal.value().parse().map_err(|_| {
                            syn::Error::new_spanned(literal, "No such marshaling rule")
                        })?);
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

#[cfg(test)]
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
}
