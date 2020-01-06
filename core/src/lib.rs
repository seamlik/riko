//! Core components of Riko

#![feature(drain_filter)]

pub mod ir;
pub mod jni;
pub mod parse;

use ir::Crate;
use ir::Function;
use ir::Module;
use regex::Regex;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

/// Target code generation.
pub trait TargetCodeWriter {
    /// Generates target code for the entire crate and writes to a tree of files.
    fn write_all(&self, ir: &Crate) -> anyhow::Result<()>;

    /// Generates target code for a function.
    fn write_function(
        &self,
        function: &Function,
        module: &Module,
        root: &Crate,
    ) -> syn::Result<String>;

    /// Generates target code for a module.
    fn write_module(&self, module: &Module, root: &Crate) -> syn::Result<String>;

    fn write_target_file(&self, path: &Path, content: &str) -> std::io::Result<()> {
        let mut path_full = self.output_directory().to_owned();
        path_full.push(path);

        std::fs::create_dir_all(path_full.parent().unwrap())?;

        let mut file = File::create(path_full)?;
        file.write_all(content.as_bytes())?;

        Ok(())
    }

    /// The directory where the target code is written to.
    fn output_directory(&self) -> &Path;
}

}

fn normalize_source_code(code: &str) -> String {
    let regex = r"\s+".parse::<Regex>().unwrap();
    code.replace(&regex, " ")
}
