use once_cell::sync::Lazy;
use proc_macro2::Span;
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

static CONFIGS: Lazy<Mutex<HashMap<PathBuf, Arc<Config>>>> =
    Lazy::new(|| Mutex::new(std::iter::once(Default::default()).collect()));

const CONFIG_FILENAME: &str = "Riko.toml";

/// Gets the config according the call site.
pub fn current() -> Result<Arc<Config>, Box<dyn Error>> {
    let config_path = locate(proc_macro::Span::call_site())?;
    let mut configs = CONFIGS.lock()?;
    if let Some(config) = configs.get(&config_path) {
        Ok(config.clone())
    } else {
        let config = Arc::new(read(&config_path)?);
        configs.insert(config_path, config.clone());
        Ok(config)
    }
}

/// Reads config from filesystem.
fn read(config_path: &Path) -> Result<Config, Box<dyn Error>> {
    let mut config: Config = toml::from_slice(&std::fs::read(config_path)?)?;
    config.expand_all_fields(&config_path)?;
    Ok(config)
}

/// Locates a config upwards from the source file being expanded.
///
/// First look for `Riko.toml` in the directory containing the source file, then continue searching
/// its parent directory until the root is reached.
fn locate(span: proc_macro::Span) -> std::io::Result<PathBuf> {
    let source_file = root_span(span).source_file();
    let mut config_path = if source_file.is_real() {
        source_file.path().canonicalize()?
    } else {
        eprintln!("Source file in an external crate, skipping code geeration.");
        return Ok(Default::default());
    };
    if !config_path.is_dir() {
        config_path.pop();
    }
    loop {
        config_path.push(CONFIG_FILENAME);
        if config_path.is_file() {
            break Ok(config_path);
        } else if config_path.parent().unwrap().parent().is_none() {
            eprintln!("No Riko configuration found, skipping code generation.");
            break Ok(Default::default()); // Reached root
        } else {
            config_path.pop();
            config_path.pop();
        }
    }
}

/// Recursively finds the root [Span](proc_macro::Span).
fn root_span(span: proc_macro::Span) -> proc_macro::Span {
    match span.parent() {
        Some(parent) => root_span(parent),
        None => span,
    }
}

/// `Riko.toml`.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub jni: JniConfig,
    pub output: PathBuf,

    /* Below are cached data, not fields in `Riko.toml`. */
    #[serde(skip)]
    pub crate_name: String,

    #[serde(skip)]
    pub entry: PathBuf,
}

impl Config {
    /// Fills in all cached data by reading additional information from elsewhere.
    fn expand_all_fields(&mut self, config_path: &Path) -> Result<(), Box<dyn Error>> {
        let cargo_path = config_path.with_file_name("Cargo.toml");
        let cargo: CargoConfig = toml::from_slice(&std::fs::read(&cargo_path)?)?;
        self.crate_name = cargo.package.name;
        self.entry = if cargo.lib.path.is_absolute() {
            cargo.lib.path
        } else {
            let mut entry: PathBuf = cargo_path;
            entry.pop();
            entry.extend(cargo.lib.path.iter());
            entry
        };

        Ok(())
    }

    /// Gets the module path of a [Span] according to its source file path.
    ///
    /// The result will not be correct if the actual module is a sub-module inside a source file.
    pub fn module_by_span(&self, span: Span) -> syn::Result<Vec<String>> {
        let source_file = root_span(span.unwrap()).source_file();
        if self.crate_name.is_empty() {
            Err(syn::Error::new(span, "Unknown crate name."))
        } else if !source_file.is_real() {
            Err(syn::Error::new(span, "Source file is not real."))
        } else if let Some(crate_path) = self.entry.parent() {
            let source_file_path = source_file.path().canonicalize().map_err(|_| {
                syn::Error::new(span, format!("Invalid source file path: {:?}", source_file))
            })?;

            // Crate root
            if source_file_path == self.entry {
                return Ok(vec![self.crate_name.clone()]);
            }

            let mut result: Vec<String> = source_file_path
                .strip_prefix(crate_path)
                .map_err(|_| {
                    syn::Error::new(
                        span,
                        format!("Source file outside of current crate: {:?}", source_file),
                    )
                })?
                .iter()
                .map(|it| it.to_str().unwrap().into())
                .collect();
            result.insert(0, self.crate_name.clone());
            let mut last = result.pop().unwrap();
            if last != "mod.rs" && last.ends_with(".rs") {
                last = last.trim_end_matches(".rs").into();
                result.push(last);
            }
            Ok(result)
        } else {
            Err(syn::Error::new(span, "Unknown crate entry."))
        }
    }
}

#[derive(Deserialize, Default)]
pub struct JniConfig {
    pub enabled: bool,
}

/// Minified version of a `Cargo.toml`.
#[derive(Deserialize)]
struct CargoConfig {
    lib: CargoConfigLib,
    package: CargoConfigPackage,
}

#[derive(Deserialize)]
struct CargoConfigPackage {
    name: String,
}

#[derive(Deserialize)]
#[serde(default)]
struct CargoConfigLib {
    path: PathBuf,
}

impl Default for CargoConfigLib {
    fn default() -> Self {
        Self {
            path: vec!["src", "lib.rs"].into_iter().collect(),
        }
    }
}
