//! Core components of Riko
//!
//! [bindgen] is the entry point.

#![feature(iter_order_by)]

pub mod ir;
mod jni;
pub mod parse;
mod util;

use async_std::fs::File;
use futures_util::io::AsyncWriteExt;
use futures_util::TryFutureExt;
use ir::Crate;
use ir::Function;
use ir::Module;
use proc_macro2::TokenStream;
use quote::ToTokens;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt::Display;
use std::fmt::Formatter;
use std::path::Path;
use std::path::PathBuf;
use syn::ItemFn;
use thiserror::Error;

/// Target code generation.
trait TargetCodeWriter {
    /// Generates target code for the entire crate and writes to a tree of files.
    fn write_target_all(&self, ir: &Crate) -> HashMap<PathBuf, String>;

    /// Generates all bridge code.
    fn write_bridge_all(&self, root: &Crate) -> Result<TokenStream, Error> {
        let mut result = TokenStream::default();
        for module in root.modules.iter() {
            for function in module.functions.iter() {
                result.extend(
                    self.write_bridge_function(function, module, root)
                        .into_token_stream(),
                );
            }
        }
        Ok(result)
    }

    /// Generates Rust bridge code for a function.
    fn write_bridge_function(&self, function: &Function, module: &Module, root: &Crate) -> ItemFn;

    /// Generates target code for a function.
    fn write_target_function(&self, function: &Function, module: &Module, root: &Crate) -> String;

    /// Generates target code for a module.
    fn write_target_module(&self, module: &Module, root: &Crate) -> String;
}

/// Creates a source file on the filesystem, overwrites any existing content, handles logging.
///
/// Used by [TargetCodeWriter] implementations.
async fn open_file(path: &Path) -> std::io::Result<File> {
    log::info!("Writing to `{}`", path.display());
    async_std::fs::create_dir_all(path.parent().unwrap()).await?;
    File::create(path).await
}

/// Writes a source file.
///
/// The file will be created first, and all existing content will be erased.
async fn write_file(path: &Path, content: &str) -> std::io::Result<()> {
    log::info!("Writing to `{}`", path.display());

    async_std::fs::create_dir_all(path.parent().unwrap()).await?;

    let mut file = File::create(path).await?;
    file.write_all(content.as_bytes()).await
}

/// Generates language bindings and writes to an output directory.
pub async fn bindgen<'a>(
    crate_name: &str,
    crate_entry: &Path,
    output_directory: &Path,
    targets: impl Iterator<Item = &'a String>,
) -> Result<(), Error> {
    let mut bridge_path = output_directory.to_owned();
    bridge_path.push(format!("{}.rs", crate_name));
    // Delete the bridge code first because it interferes with the IR scanning
    log::info!("Deleting bridge code: {}", bridge_path.display());
    let _ = async_std::fs::remove_file(&bridge_path).await;

    let ir = Crate::parse(crate_entry, crate_name.into()).await?;
    let mut bridge = TokenStream::default();

    for (target, writer) in create_target_code_writers(targets).into_iter() {
        bridge.extend(writer.write_bridge_all(&ir)?);

        let mut target_output_directory = output_directory.to_owned();
        target_output_directory.push(target);

        let mut target_output_directory_root = target_output_directory.clone();
        target_output_directory_root.push(crate_name);
        // Clear the target code first because the bindgen is append-only
        log::info!(
            "Deleting target code: {}",
            target_output_directory_root.display()
        );
        let _ = async_std::fs::remove_dir_all(&target_output_directory_root).await;

        // TODO: Parallelize this
        for (path, code) in writer.write_target_all(&ir).iter() {
            let mut dst = target_output_directory.clone();
            dst.extend(path.iter());
            write_file(&dst, code).await.map_err(|err| Error {
                file: dst,
                source: ErrorSource::Write(err),
            })?;
        }
    }

    let mut bridge_file = open_file(&bridge_path)
        .map_err(|err| Error {
            file: bridge_path.to_owned(),
            source: ErrorSource::Write(err),
        })
        .await?;
    bridge_file
        .write_all(bridge.to_string().as_bytes())
        .await
        .map_err(|err| Error {
            file: bridge_path.to_owned(),
            source: ErrorSource::Write(err),
        })?;

    Ok(())
}

/// This is where [TargetCodeWriter] implementations are registered.
fn create_target_code_writers<'a>(
    targets: impl Iterator<Item = &'a String>,
) -> BTreeMap<String, Box<dyn TargetCodeWriter>> {
    let mut map = BTreeMap::<String, Box<dyn TargetCodeWriter>>::new();
    for target in targets {
        match target.as_str() {
            "jni" => {
                map.insert(target.into(), Box::new(jni::JniWriter));
            }
            _ => log::warn!("Unsupported target `{}`", target),
        }
    }
    map
}

/// Errors when parsing Rust code or writing target code.
#[derive(Error, Debug)]
pub struct Error {
    /// The Rust source file which causes the error.
    pub file: PathBuf,
    pub source: ErrorSource,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{}", self.file.display())?;
        if let Some(err) = self
            .source()
            .and_then(|x| x.source())
            .and_then(|x| x.downcast_ref::<syn::Error>())
        {
            let location = err.span().start();
            write!(f, ":{}:{}", location.line, location.column)?;
        }
        Ok(())
    }
}

/// Cause of [Error](struct@Error).
///
/// See the source code for the meaning of the variants.
#[derive(Error, Debug)]
pub enum ErrorSource {
    #[error("Failed to read a source file")]
    ReadSource(#[source] std::io::Error),

    #[error("Failed to read an external module")]
    ReadExternalModule(#[source] Box<Error>),

    #[error("Failed to write target code")]
    Write(#[source] std::io::Error),

    #[error("Failed to parse Rust code")]
    Parse(#[source] syn::Error),

    #[error("Illegal Riko attribute usage")]
    Riko(#[source] syn::Error),
}

#[cfg(test)]
fn normalize_source_code(code: &str) -> String {
    let regex = r"\s+".parse::<regex::Regex>().unwrap();
    code.replace(&regex, " ")
}
