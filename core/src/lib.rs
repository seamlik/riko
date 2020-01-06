//! Core components of Riko

#![feature(drain_filter)]

pub mod ir;
pub mod jni;
pub mod parse;

use ir::Crate;
use ir::Function;
use ir::Module;
use regex::Regex;
use std::error::Error as StdError;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::path::PathBuf;
use thiserror::Error;

/// Target code generation.
pub trait TargetCodeWriter {
    /// Generates target code for the entire crate and writes to a tree of files.
    fn write_all(&self, ir: &Crate) -> Result<(), Error>;

    /// Generates target code for a function.
    fn write_function(&self, function: &Function, module: &Module, root: &Crate) -> String;

    /// Generates target code for a module.
    fn write_module(&self, module: &Module, root: &Crate) -> String;

    fn write_target_file(&self, path: &Path, content: &str) -> std::io::Result<()> {
        let mut path_full = self.output_directory().to_owned();
        path_full.push(path);
        log::info!("Writing to `{}`", path_full.display());

        std::fs::create_dir_all(path_full.parent().unwrap())?;

        let mut file = File::create(path_full)?;
        file.write_all(content.as_bytes())?;

        Ok(())
    }

    /// The directory where the target code is written to.
    fn output_directory(&self) -> &Path;
}

/// Errors when parsing Rust code or writing target code.
#[derive(Error, Debug)]
pub struct Error {
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

/// Cause of [Error].
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

fn normalize_source_code(code: &str) -> String {
    let regex = r"\s+".parse::<Regex>().unwrap();
    code.replace(&regex, " ")
}
