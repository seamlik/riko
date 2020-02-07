//! Intermediate representations (IR).
//!
//! These types are generated after parsing a Rust source file containing Riko attributes. They
//! contain the information sufficient for generating target code.

use crate::parse::Args;
use crate::parse::Assertable;
use crate::parse::Expanded;
use crate::parse::Fun;
use crate::parse::Marshal;
use crate::parse::MarshalingRule;
use crate::ErrorSource;
use quote::ToTokens;
use std::path::Path;
use std::path::PathBuf;
use syn::AttrStyle;
use syn::Attribute;
use syn::FnArg;
use syn::Item;
use syn::ItemFn;
use syn::ItemMod;
use syn::Lit;
use syn::Meta;
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

fn extract_cfg(src: impl Iterator<Item = Attribute>) -> Vec<Assertable<Attribute>> {
    src.filter(|attr| attr.path.to_token_stream().to_string() == "cfg")
        .map(|mut attr| {
            if let AttrStyle::Inner(_) = attr.style {
                attr.style = AttrStyle::Outer;
            }
            Assertable(attr)
        })
        .collect()
}

/// Crate.
///
/// This is the root of an IR tree.
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
            modules: Module::parse_items(file.items.into_iter(), &[], src, file.attrs.into_iter())?,
            name,
        })
    }

    /// List of [Module]s in a module path, in the order of appearance in the path.
    ///
    /// Used to get a list of all parent [Module]s.
    fn modules_by_path<'a>(&'a self, path: &[String]) -> Vec<&'a Module> {
        path.iter()
            .enumerate()
            .map(|(idx, _)| {
                let subpath = &path[..(idx + 1)];
                self.modules
                    .iter()
                    .find(|m| m.path == subpath)
                    .expect("No such module path")
            })
            .collect()
    }
}

/// Module.
#[derive(Debug, PartialEq)]
pub struct Module {
    pub functions: Vec<Function>,

    /// Full path of this [Module]. An empty path indicates the root module.
    pub path: Vec<String>,

    pub cfg: Vec<Assertable<Attribute>>,
}

impl Module {
    /// Parses a block of Rust source file into a set of [Module]s.
    ///
    /// # Parameters
    ///
    /// * `module_path`: The path of the module being parsed. It serves as the prefix of the paths of all
    ///   of its child modules.
    ///
    /// # Returns
    ///
    /// Contains the module being parsed and all its child modules.
    fn parse_items(
        items: impl Iterator<Item = Item>,
        module_path: &[String],
        file_path: &Path,
        attrs: impl Iterator<Item = Attribute>,
    ) -> Result<Vec<Self>, crate::Error> {
        let mut result = Vec::<Self>::new();
        let mut functions = Vec::<Function>::new();

        let conv_err = |err| crate::Error {
            file: file_path.to_owned(),
            source: ErrorSource::Riko(err),
        };

        for item in items {
            match item {
                Item::Fn(inner) => {
                    if let Some(args) = Fun::take_from(inner.attrs.iter()).map_err(conv_err)? {
                        let args = args.expand_all_fields(&inner.sig).map_err(conv_err)?;
                        functions.push(Function::parse(inner, args).map_err(conv_err)?);
                    }
                }
                Item::Mod(inner) => match Self::parse_module(inner, module_path, file_path) {
                    Ok(parsed) => result.extend(parsed),
                    Err(err) => {
                        if let ErrorSource::ReadExternalModule(err_inner) = err.source {
                            if let ErrorSource::ReadSource(err_io) = err_inner.source {
                                log::error!(
                                    "Failed to read source file `{}`. Ignore this if the module is supposed to be generated by Riko. Inner error: {}",
                                    err_inner.file.display(),
                                    err_io,
                                );
                            }
                        } else {
                            return Err(err);
                        }
                    }
                },
                _ => {}
            }
        }
        result.push(Self {
            functions,
            path: module_path.into(),
            cfg: extract_cfg(attrs),
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
        module: ItemMod,
        module_path_parent: &[String],
        file_path_parent: &Path,
    ) -> Result<Vec<Self>, crate::Error> {
        let mut module_path_child: Vec<String> = module_path_parent.into();
        module_path_child.push(module.ident.to_string());

        let file_path_child =
            resolve_module_path(file_path_parent.to_owned(), &module).map_err(|err| {
                crate::Error {
                    file: file_path_parent.to_owned(),
                    source: ErrorSource::Parse(err),
                }
            })?;

        if let Some((_, items)) = module.content {
            Self::parse_items(
                items.into_iter(),
                &module_path_child,
                &file_path_parent,
                module.attrs.into_iter(),
            )
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
            Self::parse_items(
                ast.items.into_iter(),
                &module_path_child,
                &file_path_child,
                ast.attrs.into_iter(),
            )
        }
    }
}

/// Free-standing function.
#[derive(Debug, PartialEq)]
pub struct Function {
    pub inputs: Vec<Input>,
    pub name: String,
    pub output: Option<MarshalingRule>,
    pub cfg: Vec<Assertable<Attribute>>,

    /// Public name exported to the target side.
    pub pubname: String,
}

impl Function {
    /// Parses an [ItemFn].
    fn parse(item: ItemFn, args: Fun<Expanded>) -> syn::Result<Self> {
        let name = item.sig.ident.to_string();

        Ok(Self {
            inputs: item
                .sig
                .inputs
                .iter()
                .map(Input::parse)
                .collect::<syn::Result<_>>()?,
            pubname: if args.name.is_empty() {
                name.clone()
            } else {
                args.name
            },
            name,
            output: args.marshal,
            cfg: extract_cfg(item.attrs.into_iter()),
        })
    }

    /// Collects all `#[cfg]` in `self` and all its parent [Module].
    pub fn collect_cfg<'a>(
        &'a self,
        module: &'a Module,
        root: &'a Crate,
    ) -> Vec<&Assertable<Attribute>> {
        root.modules_by_path(&module.path)
            .iter()
            .flat_map(|m| m.cfg.iter())
            .chain(self.cfg.iter())
            .collect()
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
    fn parse(item: &FnArg) -> syn::Result<Self> {
        match item {
            FnArg::Typed(typed) => {
                let rule = if let Some(args) = Marshal::take_from(typed.attrs.iter())? {
                    args.value
                } else {
                    MarshalingRule::infer(&typed.ty)?
                };

                let borrow = if let Type::Reference(_) = *typed.ty {
                    true
                } else {
                    false
                };

                Ok(Self { rule, borrow })
            }
            FnArg::Receiver(_) => todo!("`#[fun]` on a method not implemented"),
        }
    }
}

mod test {
    use super::*;

    #[test]
    fn function() {
        let function: syn::ItemFn = syn::parse_quote! {
            #[riko::fun(marshal = "I32", name = "function2")]
            fn function(
                a: bool,
                #[riko::marshal = "Bytes"] b: &String,
            ) -> Vec<u8> {
                unimplemented!()
            }
        };
        let args = Fun::take_from(function.attrs.iter())
            .unwrap()
            .unwrap()
            .expand_all_fields(&function.sig)
            .unwrap();

        let expected = Function {
            name: "function".into(),
            inputs: vec![
                Input {
                    rule: MarshalingRule::Bool,
                    borrow: false,
                },
                Input {
                    rule: MarshalingRule::Bytes,
                    borrow: true,
                },
            ],
            output: Some(MarshalingRule::I32),
            pubname: "function2".into(),
            cfg: Default::default(),
        };
        let actual = Function::parse(function, args).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn input() {
        let function: ItemFn = syn::parse_quote! {
            pub fn function(
                a: String,
                #[riko::marshal = "String"] b: usize,
                c: &ByteBuf,
                #[riko::marshal = "I32"] d: &Vec<u8>,
            ) {
                unimplemented!()
            }
        };
        let expected = vec![
            Input {
                rule: MarshalingRule::String,
                borrow: false,
            },
            Input {
                rule: MarshalingRule::String,
                borrow: false,
            },
            Input {
                rule: MarshalingRule::Bytes,
                borrow: true,
            },
            Input {
                rule: MarshalingRule::I32,
                borrow: true,
            },
        ];
        let actual = function
            .sig
            .inputs
            .iter()
            .map(Input::parse)
            .collect::<syn::Result<Vec<Input>>>()
            .unwrap();
        assert_eq!(expected, actual)
    }

    #[test]
    fn cfg() {
        let module: ItemMod = syn::parse_quote! {
            #[cfg(feature = "riko_outer")]
            mod util {
                #![cfg(feature = "riko_inner")]

                #[cfg(feature = "util_outer")]
                mod linux {
                    #![cfg(feature = "util_inner")]

                    #[cfg(feature = "function_outer")]
                    #[riko::fun]
                    fn function() {
                        #![cfg(feature = "function_inner")]
                        unimplemented!()
                    }
                }
            }
        };
        let ir = Crate {
            name: "riko_sample".into(),
            modules: Module::parse_module(module, &[], &PathBuf::default()).unwrap(),
        };

        let expected = [
            r#"# [ cfg ( feature = "riko_outer" ) ]"#,
            r#"# [ cfg ( feature = "riko_inner" ) ]"#,
            r#"# [ cfg ( feature = "util_outer" ) ]"#,
            r#"# [ cfg ( feature = "util_inner" ) ]"#,
            r#"# [ cfg ( feature = "function_outer" ) ]"#,
            r#"# [ cfg ( feature = "function_inner" ) ]"#,
        ]
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
        let actual = ir.modules[0].functions[0]
            .collect_cfg(&ir.modules[0], &ir)
            .into_iter()
            .map(|a| a.to_token_stream().to_string())
            .collect::<Vec<_>>();

        assert_eq!(expected, actual)
    }
}
