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
use syn::Item;
use syn::ItemFn;
use syn::ItemMod;
use syn::ReturnType;

fn resolve_module_path(file_path_parent: &Path, module_name_child: &str) -> PathBuf {
    let mut result = file_path_parent.with_file_name(format!("{}.rs", module_name_child));
    if !result.is_file() {
        result.set_file_name(module_name_child);
        result.push("mod.rs");
    }
    result
}

/// Crate.
///
/// This is the root of a tree of IR.
#[derive(Debug, PartialEq)]
pub struct Crate {
    pub bridges: Vec<Bridge>,
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
        let (modules, bridges) = Module::parse_items(&file.items, &[], src)?;
        Ok(Self {
            modules,
            name,
            bridges,
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
    ) -> Result<(Vec<Self>, Vec<Bridge>), crate::Error> {
        let mut result = Vec::<Self>::new();
        let mut bridges = Vec::<Bridge>::new();
        let mut functions = Vec::<Function>::new();

        for item in items.iter() {
            match item {
                Item::Fn(inner) => {
                    if inner
                        .attrs
                        .iter()
                        .any(|x| x.path.to_token_stream().to_string() == "riko :: fun")
                    {
                        let (f, b) = Function::parse(inner).map_err(|err| crate::Error {
                            file: file_path.to_owned(),
                            source: ErrorSource::Riko(err),
                        })?;
                        functions.push(f);
                        bridges.push(b);
                    }
                }
                Item::Mod(inner) => {
                    let (m, b) = Self::parse_module(inner, module_path, file_path)?;
                    result.extend(m);
                    bridges.extend(b);
                }
                _ => {}
            }
        }
        result.push(Self {
            functions,
            path: module_path.into(),
        });
        Ok((result, bridges))
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
    ) -> Result<(Vec<Self>, Vec<Bridge>), crate::Error> {
        let module_name_child = module.ident.to_string();

        let mut module_path_child: Vec<String> = module_path_parent.into();
        module_path_child.push(module_name_child.clone());

        // TODO: Support `#[path]` on external modules
        let file_path_child = resolve_module_path(file_path_parent, &module_name_child);

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
    /// Parameters.
    pub inputs: Vec<MarshalingRule>,

    /// Name of the Rust `extern` function that wraps the origianl one.
    pub name_bridge: String,

    /// Name of the function on the target side.
    pub name_public: String,

    /// Return type.
    pub output: Option<MarshalingRule>,
}

impl Function {
    /// Parses an [ItemFn]. The item must be marked by a `#[riko::fun]`.
    fn parse(item: &ItemFn) -> syn::Result<(Self, Bridge)> {
        let attr = item
            .attrs
            .iter()
            .filter(|x| x.path.to_token_stream().to_string() == "riko :: fun")
            .nth(0)
            .unwrap();
        let mut args: Fun = if attr.tokens.is_empty() {
            Default::default()
        } else {
            attr.parse_args()?
        };
        args.expand_all_fields(&item.sig)?;

        let mut item_stripped = item.clone(); // TODO: Don't clone
        let inputs = MarshalingRule::parse(item_stripped.sig.inputs.iter_mut())?;

        let name_bridge = crate::parse::mangle_function_name(item).to_string();

        let bridge = Bridge {
            input: item.sig.inputs.len(),
            name: name_bridge.clone(),
            output: std::mem::discriminant(&ReturnType::Default)
                != std::mem::discriminant(&item.sig.output),
        };
        let result = Self {
            inputs,
            name_bridge,
            name_public: args.name,
            output: args.marshal,
        };

        Ok((result, bridge))
    }
}

/// Function that bridges the code from both sides of FFI.
///
/// For every functions to be exported on the Rust side, there must be a function exposing a C API
/// so that the code on the target side can call. This special function is called a "bridge".
///
/// Bridges are not specified by the user but generated by Riko.
#[derive(Debug, PartialEq)]
pub struct Bridge {
    pub input: usize,
    pub name: String,
    pub output: bool,
}

mod test {
    use super::*;

    #[test]
    fn function() {
        let mut function: syn::ItemFn = syn::parse_quote! {
            #[riko::fun(marshal = "String", name = "function2")]
            fn function(
                a: &String,
                #[riko::marshal(String)] b: Option<String>,
            ) -> Result<Option<String>> {
                unimplemented!()
            }
        };
        let name_bridge = crate::parse::mangle_function_name(&function).to_string();

        let expected_function = Function {
            inputs: vec![MarshalingRule::String, MarshalingRule::String],
            name_bridge: name_bridge.clone(),
            name_public: "function2".into(),
            output: Some(MarshalingRule::String),
        };
        let expected_bridge = Bridge {
            name: name_bridge,
            input: 2,
            output: true,
        };
        let actual = Function::parse(&mut function).unwrap();
        assert_eq!(&(expected_function, expected_bridge), &actual);
    }
}
