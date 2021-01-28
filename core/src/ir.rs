//! Intermediate representations (IR).
//!
//! These types are generated after parsing a Rust source file containing Riko attributes. They
//! contain the information sufficient for generating target code.

#[cfg(test)]
pub mod sample;

use crate::parse::Args;
use crate::parse::Fun;
use crate::parse::Marshal;
use crate::ErrorSource;
use futures_util::future::LocalBoxFuture;
use futures_util::FutureExt;
use proc_macro2::TokenStream;
use quote::ToTokens;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::path::Path;
use std::path::PathBuf;
use strum_macros::*;
use syn::AttrStyle;
use syn::Attribute;
use syn::FnArg;
use syn::Item;
use syn::ItemFn;
use syn::ItemMod;
use syn::Lit;
use syn::Meta;
use syn::ReturnType;
use syn::Type;
use syn::TypePath;

/// Resolve the file path to a chile module.
///
/// # Parameters
///
/// * `module_name_parent`: The name of the parent module. If none, it means the crate root.
/// * `file_path_parent`: The file path to the parent module.
fn resolve_module_path(
    module_name_parent: Option<&String>,
    file_path_parent: PathBuf,
    module_child: &ItemMod,
) -> syn::Result<PathBuf> {
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
    } else if let Some(parent) = module_name_parent {
        // Not crate root
        let mut result = file_path_parent;
        let child = module_child.ident.to_string();
        result.set_file_name(parent);
        result.push(format!("{}.rs", &child));
        if result.is_file() {
            Ok(result)
        } else {
            result.set_file_name(&child);
            result.push("mod.rs");
            Ok(result)
        }
    } else {
        // Crate root
        let mut result = file_path_parent;
        result.set_file_name(format!("{}.rs", module_child.ident.to_string()));
        if result.is_file() {
            Ok(result)
        } else {
            result.set_file_name(module_child.ident.to_string());
            result.push("mod.rs");
            Ok(result)
        }
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

fn find_ignore_attribute<'a>(mut src: impl Iterator<Item = &'a Attribute>) -> bool {
    src.any(|attr| attr.path.to_token_stream().to_string() == "riko :: ignore")
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
///
/// # Inference
///
/// Since procedural macros can only analyse a syntax tree and have no access to any type
/// information, it is impossible to always acurrately infer the rule. If the inference causes
/// compiler errors or a type alias is used, specify the rule explicitly.
///
/// If no other rules match the inference, [Struct](MarshalingRule::Struct) will be chosen by default.
///
/// # Result and Option
///
/// All rules support the underlying type wrapped inside either a [Result], an [Option] or a
/// `Result<Option<T>, E>`. No other combinations are supported.
#[derive(PartialEq, Debug, EnumString, Clone, Copy)]
pub enum MarshalingRule {
    /// [bool].
    Bool,

    /// Marshals specifically a byte array instead of a collection of [u8].
    ///
    /// Only `ByteBuf` from [serde_bytes](https://crates.io/crates/serde_bytes) is supported for
    /// this rule.
    Bytes,

    /// [i8].
    I8,

    /// [i32].
    I32,

    /// [i64].
    I64,

    /// Heap-allocated data.
    Object,

    /// Custom types that support serialzation through [Serde](https://serde.rs).
    ///
    /// This rule requires the type in the function signature be fully qualified.
    Struct,

    /// [String].
    String,

    /// `()`.
    Unit,
}

impl MarshalingRule {
    fn infer(t: &syn::Path) -> Self {
        fn matches(candidate: &'static [&'static str], raw: &str) -> bool {
            raw == *candidate.last().unwrap()
                || raw == candidate[1..].join(" :: ")
                || raw == candidate.join(" :: ").trim()
        }

        let type_path_str = t.segments.to_token_stream().to_string();

        if "bool" == type_path_str {
            Self::Bool
        } else if "i32" == type_path_str {
            Self::I32
        } else if "i64" == type_path_str {
            Self::I64
        } else if "i8" == type_path_str {
            Self::I8
        } else if type_path_str.trim().is_empty() {
            Self::Unit
        } else if matches(&["", "std", "string", "String"], &type_path_str) {
            Self::String
        } else if matches(&["", "serde_bytes", "ByteBuf"], &type_path_str) {
            Self::Bytes
        } else {
            Self::Struct
        }
    }
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
    pub async fn parse(src: &Path, name: String) -> Result<Self, crate::Error> {
        log::info!("Reading `{}`", src.display());
        let raw = async_std::fs::read_to_string(src)
            .await
            .map_err(|err| crate::Error {
                file: src.to_owned(),
                source: ErrorSource::ReadSource(err),
            })?;
        let file = syn::parse_file(&raw).map_err(|err| crate::Error {
            file: src.to_owned(),
            source: ErrorSource::Parse(err),
        })?;
        Ok(Self {
            modules: Module::parse_items(file.items.into_iter(), &[], src, file.attrs.into_iter())
                .await?,
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
    /// * `module_path`: The fully quialified name of the module being parsed. It serves as the prefix of the paths of all
    ///   of its child modules. An empty path means the crate root.
    /// * `file_path`: Path to the file containing the `items`.
    ///
    /// # Returns
    ///
    /// Contains the module being parsed and all its child modules.
    ///
    /// Boxed Future to work around recursive async.
    fn parse_items<'a>(
        items: impl Iterator<Item = Item> + 'static,
        module_path: &'a [String],
        file_path: &'a Path,
        attrs: impl Iterator<Item = Attribute> + 'static,
    ) -> LocalBoxFuture<'a, Result<Vec<Self>, crate::Error>> {
        async move {
            let mut result = Vec::<Self>::new();
            let mut functions = Vec::<Function>::new();

            let conv_err = |err| crate::Error {
                file: file_path.to_owned(),
                source: ErrorSource::Riko(err),
            };

            for item in items {
                match item {
                    Item::Fn(inner) => {
                        if find_ignore_attribute(inner.attrs.iter()) {
                            log::info!("Ignoring function `{}`", inner.sig.ident);
                            continue;
                        }

                        if let Some(args) = Fun::take_from(inner.attrs.iter()).map_err(conv_err)? {
                            functions.push(Function::parse(inner, args).map_err(conv_err)?);
                        }
                    }
                    Item::Mod(inner) => {
                        if find_ignore_attribute(inner.attrs.iter()) {
                            log::info!("Ignoring module `{}`", inner.ident);
                            continue;
                        }

                        let parsed_module =
                            Self::parse_module(inner, module_path, file_path).await?;
                        result.extend(parsed_module);
                    }
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
        .boxed_local()
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
    async fn parse_module(
        module: ItemMod,
        module_path_parent: &[String],
        file_path_parent: &Path,
    ) -> Result<Vec<Self>, crate::Error> {
        let mut module_path_child: Vec<String> = module_path_parent.into();
        module_path_child.push(module.ident.to_string());

        let file_path_child = resolve_module_path(
            module_path_parent.last(),
            file_path_parent.to_owned(),
            &module,
        )
        .map_err(|err| crate::Error {
            file: file_path_parent.to_owned(),
            source: ErrorSource::Parse(err),
        })?;

        if let Some((_, items)) = module.content {
            Self::parse_items(
                items.into_iter(),
                &module_path_child,
                &file_path_parent,
                module.attrs.into_iter(),
            )
            .await
        } else {
            log::info!("Reading `{}`", file_path_child.display());
            let raw = async_std::fs::read_to_string(&file_path_child)
                .await
                .map_err(|err| crate::Error {
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
            .await
        }
    }
}

/// Free-standing function.
#[derive(Debug, PartialEq)]
pub struct Function {
    pub inputs: Vec<Input>,
    pub name: String,
    pub output: Output,
    pub cfg: Vec<Assertable<Attribute>>,

    /// Public name exported to the target side.
    pub pubname: String,
}

impl Function {
    /// Parses an [ItemFn].
    fn parse(item: ItemFn, args: Fun) -> syn::Result<Self> {
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
            output: Output::parse(&item.sig.output, args.marshal)?,
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

    /// The actual type wrapped inside a [Result] or an [Option].
    pub unwrapped_type: Assertable<syn::Path>,
}

impl Input {
    fn parse(item: &FnArg) -> syn::Result<Self> {
        match item {
            FnArg::Typed(typed) => {
                let (borrow, original_type) = if let Type::Reference(inner) = &*typed.ty {
                    (true, crate::util::assert_type_is_path(&inner.elem)?)
                } else {
                    (false, crate::util::assert_type_is_path(&*typed.ty)?)
                };
                let unwrapped_type = crate::util::unwrap_type(original_type);
                let rule = if let Some(args) = Marshal::take_from(typed.attrs.iter())? {
                    args.value
                } else {
                    MarshalingRule::infer(&unwrapped_type)
                };

                Ok(Self {
                    rule,
                    borrow,
                    unwrapped_type: Assertable(unwrapped_type),
                })
            }
            FnArg::Receiver(_) => todo!("`#[fun]` on a method not implemented"),
        }
    }
}

/// Function return type.
#[derive(Debug, PartialEq)]
pub struct Output {
    pub rule: MarshalingRule,

    /// The actual type wrapped inside a [Result] or an [Option].
    pub unwrapped_type: Assertable<syn::Path>,
}

impl Output {
    fn parse(sig: &ReturnType, rule_hint: Option<MarshalingRule>) -> syn::Result<Self> {
        let original_type = match sig {
            ReturnType::Default => syn::Path {
                leading_colon: None,
                segments: Default::default(),
            },
            ReturnType::Type(_, ty) => crate::util::assert_type_is_path(&*ty)?,
        };
        let unwrapped_type = crate::util::unwrap_type(original_type);
        let rule = if let Some(inner) = rule_hint {
            inner
        } else {
            MarshalingRule::infer(&unwrapped_type)
        };

        Ok(Self {
            rule,
            unwrapped_type: Assertable(unwrapped_type),
        })
    }

    /// The type to use in the bridge code as `Returned<#marshaled_type>`.
    pub fn marshaled_type(&self) -> Type {
        match self.rule {
            MarshalingRule::Object => syn::parse_quote! { ::riko_runtime::object::Handle },
            MarshalingRule::Bool => syn::parse_quote! { bool },
            MarshalingRule::Bytes => syn::parse_quote! { ::serde_bytes::ByteBuf },
            MarshalingRule::I8 => syn::parse_quote! { i8 },
            MarshalingRule::I32 => syn::parse_quote! { i32 },
            MarshalingRule::I64 => syn::parse_quote! { i64 },
            MarshalingRule::Struct => Type::Path(TypePath {
                qself: None,
                path: self.unwrapped_type.0.clone(),
            }),
            MarshalingRule::String => syn::parse_quote! { ::std::string::String },
            MarshalingRule::Unit => syn::parse_quote! { () },
        }
    }
}

impl Default for Output {
    fn default() -> Self {
        Self {
            rule: MarshalingRule::Unit,
            unwrapped_type: Assertable(syn::Path {
                leading_colon: None,
                segments: Default::default(),
            }),
        }
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod test {
    use super::*;

    #[test]
    fn MarshalingRule_infer() {
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { ByteBuf }),
            MarshalingRule::Bytes
        );
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { String }),
            MarshalingRule::String
        );
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { std::string::String }),
            MarshalingRule::String
        );
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { ::std::string::String }),
            MarshalingRule::String
        );
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { bool }),
            MarshalingRule::Bool
        );
        assert_eq!(
            MarshalingRule::infer(&syn::parse_quote! { crate::Love }),
            MarshalingRule::Struct
        );
    }

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
        let args = Fun::take_from(function.attrs.iter()).unwrap().unwrap();

        let expected = Function {
            name: "function".into(),
            inputs: vec![
                Input {
                    rule: MarshalingRule::Bool,
                    borrow: false,
                    unwrapped_type: Assertable(syn::parse_quote! { bool }),
                },
                Input {
                    rule: MarshalingRule::Bytes,
                    borrow: true,
                    unwrapped_type: Assertable(syn::parse_quote! { String }),
                },
            ],
            output: Output::parse(&function.sig.output, args.marshal).unwrap(),
            pubname: "function2".into(),
            cfg: Default::default(),
        };
        let actual = Function::parse(function, args).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn input() {
        assert_eq!(
            Input {
                rule: MarshalingRule::String,
                borrow: false,
                unwrapped_type: Assertable(syn::parse_quote! { String }),
            },
            Input::parse(&syn::parse_quote! { a: String }).unwrap(),
        );
        assert_eq!(
            Input {
                rule: MarshalingRule::String,
                borrow: false,
                unwrapped_type: Assertable(syn::parse_quote! { usize }),
            },
            Input::parse(&syn::parse_quote! { #[riko::marshal = "String"] b: usize }).unwrap(),
        );
        assert_eq!(
            Input {
                rule: MarshalingRule::Bytes,
                borrow: true,
                unwrapped_type: Assertable(syn::parse_quote! { ByteBuf }),
            },
            Input::parse(&syn::parse_quote! { c: &ByteBuf }).unwrap(),
        );
        assert_eq!(
            Input {
                rule: MarshalingRule::I32,
                borrow: true,
                unwrapped_type: Assertable(syn::parse_quote! { Vec<u8> }),
            },
            Input::parse(&syn::parse_quote! { #[riko::marshal = "I32"] d: &Vec<u8> }).unwrap(),
        );
        assert_eq!(
            Input {
                rule: MarshalingRule::String,
                borrow: false,
                unwrapped_type: Assertable(syn::parse_quote! { String }),
            },
            Input::parse(&syn::parse_quote! { a: String }).unwrap(),
        );
    }

    #[test]
    fn output() {
        assert_eq!(
            Output {
                rule: MarshalingRule::Bool,
                unwrapped_type: Assertable(syn::parse_quote! { bool }),
            },
            Output::parse(&syn::parse_quote! { -> bool }, None).unwrap(),
        );
        assert_eq!(
            Output {
                rule: MarshalingRule::I32,
                unwrapped_type: Assertable(syn::parse_quote! { bool }),
            },
            Output::parse(&syn::parse_quote! { -> bool }, Some(MarshalingRule::I32)).unwrap(),
        );
        assert_eq!(
            Output {
                rule: MarshalingRule::I32,
                unwrapped_type: Assertable(syn::parse_quote! { bool }),
            },
            Output::parse(
                &syn::parse_quote! { -> Result<Option<bool>, Error> },
                Some(MarshalingRule::I32)
            )
            .unwrap(),
        );
        assert_eq!(
            Output::default(),
            Output::parse(&syn::parse_quote! { -> () }, None).unwrap(),
        );
        assert_eq!(
            Output::default(),
            Output::parse(&syn::parse_quote! {}, None).unwrap(),
        );
    }

    #[async_std::test]
    async fn cfg() {
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
            modules: Module::parse_module(module, &[], &PathBuf::default())
                .await
                .unwrap(),
        };

        let expected = [
            r#"# [cfg (feature = "riko_outer")]"#,
            r#"# [cfg (feature = "riko_inner")]"#,
            r#"# [cfg (feature = "util_outer")]"#,
            r#"# [cfg (feature = "util_inner")]"#,
            r#"# [cfg (feature = "function_outer")]"#,
            r#"# [cfg (feature = "function_inner")]"#,
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
