//! Intermediate representations (IR).
//!
//! These types are generated after parsing a Rust source file containing Riko attributes. They
//! contain the information sufficient for generating target code.

use crate::parse::Fun;
use crate::parse::MarshalingRule;
use crate::ErrorSource;
use quote::ToTokens;
use std::path::Path;
use std::path::PathBuf;
use syn::FnArg;
use syn::Item;
use syn::ItemFn;
use syn::ItemMod;
use syn::Lit;
use syn::Meta;
use syn::MetaNameValue;
use syn::Type;

fn resolve_module_path(file_path_parent: PathBuf, module_child: &ItemMod) -> syn::Result<PathBuf> {
    if let Some(attr) = module_child
        .attrs
        .iter()
        .find(|attr| attr.path.to_token_stream().to_string() == "path")
    {
        if let Meta::NameValue(nv) = attr.parse_meta()? {
            if let Lit::Str(lit) = nv.lit {
                let file_path_child: PathBuf = lit.value().into();
                let mut result = file_path_parent;
                result.pop();
                result.extend(&file_path_child);
                Ok(result)
            } else {
                Err(syn::Error::new_spanned(
                    nv.lit,
                    "Expect a file path literal",
                ))
            }
        } else {
            Err(syn::Error::new_spanned(attr, "Expect a name-value pair"))
        }
    } else {
        let mut result = file_path_parent;
        result.set_file_name(format!("{}.rs", module_child.ident.to_string()));
        if !result.is_file() {
            result.set_file_name(module_child.ident.to_string());
            result.push("mod.rs");
        }
        Ok(result)
    }
}

/// Crate.
///
/// This is the root of a tree of IR.
#[derive(Debug, PartialEq)]
pub struct Crate {
    pub modules: Vec<Module>,
    pub name: String,
}

impl Crate {
    pub fn parse(src: &Path, name: String) -> Result<Self, crate::Error> {
        log::info!("Reading `{}`", src.display());
        let raw = std::fs::read_to_string(src).map_err(|err| crate::Error {
            file: src.to_owned(),
            source: ErrorSource::ReadSource(err),
        })?;
        let file = syn::parse_file(&raw).map_err(|err| crate::Error {
            file: src.to_owned(),
            source: ErrorSource::Parse(err),
        })?;
        Ok(Self {
            modules: Module::parse_items(&file.items, &[], src)?,
            name,
        })
    }
}

/// Module.
#[derive(Debug, PartialEq)]
pub struct Module {
    pub functions: Vec<Function>,

    /// Full path of this [Module]. An empty path indicates the root module.
    pub path: Vec<String>,
}

impl Module {
    /// Parses a block of Rust source file into a set of [Module]s.
    ///
    /// # Parameters
    ///
    /// * `path`: The path of the module being parsed. It serves as the prefix of the paths of all
    ///   of its child modules.
    ///
    /// # Returns
    ///
    /// Contains the module being parsed and all its child modules.
    fn parse_items(
        items: &[Item],
        module_path: &[String],
        file_path: &Path,
    ) -> Result<Vec<Self>, crate::Error> {
        let mut result = Vec::<Self>::new();
        let mut functions = Vec::<Function>::new();

        for item in items.iter() {
            match item {
                Item::Fn(inner) => {
                    if inner
                        .attrs
                        .iter()
                        .any(|x| x.path.to_token_stream().to_string() == "riko :: fun")
                    {
                        let f = Function::parse(inner).map_err(|err| crate::Error {
                            file: file_path.to_owned(),
                            source: ErrorSource::Riko(err),
                        })?;
                        functions.push(f);
                    }
                }
                Item::Mod(inner) => {
                    result.extend(Self::parse_module(inner, module_path, file_path)?);
                }
                _ => {}
            }
        }
        result.push(Self {
            functions,
            path: module_path.into(),
        });
        Ok(result)
    }

    /// Parses a Rust module into a set of [Module]s.
    ///
    /// The result will contain the module being parsed and all its child modules.
    ///
    /// # Parameters
    ///
    /// * `module_path_parent`: Path of the parent module.
    /// * `file_path_parent`: File path of the parent moduel.
    ///
    /// # See Also
    ///
    /// * [parse_items]
    fn parse_module(
        module: &ItemMod,
        module_path_parent: &[String],
        file_path_parent: &Path,
    ) -> Result<Vec<Self>, crate::Error> {
        let module_name_child = module.ident.to_string();

        let mut module_path_child: Vec<String> = module_path_parent.into();
        module_path_child.push(module_name_child.clone());

        let file_path_child =
            resolve_module_path(file_path_parent.to_owned(), module).map_err(|err| {
                crate::Error {
                    file: file_path_parent.to_owned(),
                    source: ErrorSource::Parse(err),
                }
            })?;

        if let Some((_, items)) = &module.content {
            Self::parse_items(items, &module_path_child, &file_path_parent)
        } else {
            log::info!("Reading `{}`", file_path_child.display());
            let raw = std::fs::read_to_string(&file_path_child).map_err(|err| crate::Error {
                file: file_path_parent.to_owned(),
                source: ErrorSource::ReadExternalModule(Box::new(crate::Error {
                    file: file_path_child.to_owned(),
                    source: ErrorSource::ReadSource(err),
                })),
            })?;
            let ast = syn::parse_file(&raw).map_err(|err| crate::Error {
                file: file_path_child.to_owned(),
                source: ErrorSource::Parse(err),
            })?;
            Self::parse_items(&ast.items, &module_path_child, &file_path_child)
        }
    }
}

/// Free-standing function.
#[derive(Debug, PartialEq)]
pub struct Function {
    pub inputs: Vec<Input>,
    pub name: String,
    pub output: Option<MarshalingRule>,

    /// Public name exported to the target side.
    pub pubname: String,
}

impl Function {
    /// Parses an [ItemFn]. The item must be marked by a `#[riko::fun]`.
    fn parse(item: &ItemFn) -> syn::Result<Self> {
        let attr = item
            .attrs
            .iter()
            .find(|x| x.path.to_token_stream().to_string() == "riko :: fun")
            .unwrap();
        let name = item.sig.ident.to_string();

        let mut args: Fun = if attr.tokens.is_empty() {
            Default::default()
        } else {
            attr.parse_args()?
        };
        args.expand_all_fields(&item.sig)?;

        Ok(Self {
            inputs: Input::parse(item.sig.inputs.iter())?,
            pubname: if args.name.is_empty() {
                name.clone()
            } else {
                args.name
            },
            name,
            output: args.marshal,
        })
    }
}

/// Function parameter.
#[derive(Debug, PartialEq)]
pub struct Input {
    pub rule: MarshalingRule,
    /// If the parameter accepts a reference.
    pub borrow: bool,
}

impl Input {
    pub fn parse<'a>(params: impl Iterator<Item = &'a FnArg>) -> syn::Result<Vec<Self>> {
        params
            .map(|p| {
                if let FnArg::Typed(ref inner) = p {
                    let marshal_attr = inner
                        .attrs
                        .iter()
                        .find(|attr| attr.path.to_token_stream().to_string() == "riko :: marshal");
                    let rule = if let Some(attr) = marshal_attr {
                        if let Meta::NameValue(MetaNameValue {
                            lit: Lit::Str(value),
                            ..
                        }) = attr.parse_meta()?
                        {
                            value.parse()?
                        } else {
                            return Err(syn::Error::new_spanned(
                                attr,
                                "Invalid `#[riko::marshal]` arguments",
                            ));
                        }
                    } else {
                        MarshalingRule::infer(&inner.ty)?
                    };
                    let borrow = if let Type::Reference(_) = *inner.ty {
                        true
                    } else {
                        false
                    };
                    Ok(Self { rule, borrow })
                } else {
                    todo!("`#[fun]` on a method not implemented");
                }
            })
            .collect()
    }
}

mod test {
    use super::*;

    #[test]
    fn function() {
        let mut function: syn::ItemFn = syn::parse_quote! {
            #[riko::fun(marshal = "String", name = "function2")]
            fn function(
                a: &String,
                #[riko::marshal = "String"] b: Option<String>,
            ) -> Result<Option<String>> {
                unimplemented!()
            }
        };

        let expected = Function {
            name: "function".into(),
            inputs: vec![
                Input {
                    rule: MarshalingRule::String,
                    borrow: true,
                },
                Input {
                    rule: MarshalingRule::String,
                    borrow: false,
                },
            ],
            output: Some(MarshalingRule::String),
            pubname: "function2".into(),
        };
        let actual = Function::parse(&mut function).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn input() {
        let function: ItemFn = syn::parse_quote! {
            pub fn function(
                a: &String,
                #[riko::marshal = "String"] b: Option<String>,
            ) {
                unimplemented!()
            }
        };
        let actual = vec![
            Input {
                rule: MarshalingRule::String,
                borrow: true,
            },
            Input {
                rule: MarshalingRule::String,
                borrow: false,
            },
        ];
        assert_eq!(Input::parse(function.sig.inputs.iter()).unwrap(), actual)
    }
}
