//! Configuration.

use anyhow::Context;
use serde::Deserialize;
use std::path::Path;
use std::path::PathBuf;

/// Filename of a Riko config.
pub const FILENAME: &str = "Riko.toml";

/// `Riko.toml`.
#[derive(Deserialize)]
#[serde(default)]
pub struct Config {
    // TODO: Use `Cargo.toml` package metadata
    pub jni: JniConfig,
    pub output: PathBuf, // TODO: Remove this and output to `target` based on Cargo metadata

    /* Below are cached data, not fields in `Riko.toml`. */
    #[serde(skip)]
    pub crate_name: String,

    /// The entry source file of this crate.
    #[serde(skip)]
    pub entry: PathBuf,
}

impl Config {
    /// Reads config from filesystem.
    ///
    /// Returns a default config if the file is not found.
    pub fn read(path: &Path) -> anyhow::Result<Self> {
        if !path.is_file() {
            return Ok(Default::default());
        }

        let mut config: Config = toml::from_slice(&std::fs::read(path)?)?;
        config.expand_all_fields(&path.with_file_name("Cargo.toml"))?;
        Ok(config)
    }

    /// Fills in all optional fields, expands all relative filesystem paths, etc..
    fn expand_all_fields(&mut self, manifest_path: &Path) -> anyhow::Result<()> {
        let cargo_config_raw =
            std::fs::read(manifest_path).with_context(|| manifest_path.display().to_string())?;
        let cargo_config: CargoConfig = toml::from_slice(&cargo_config_raw)
            .with_context(|| manifest_path.display().to_string())?;

        self.crate_name = cargo_config.package.name;
        if self.output.is_relative() {
            self.output = manifest_path.parent().unwrap().join(&self.output)
        };

        self.entry = if cargo_config.lib.path.is_absolute() {
            cargo_config.lib.path
        } else {
            let mut entry: PathBuf = manifest_path.to_owned();
            entry.pop();
            entry.extend(cargo_config.lib.path.iter());
            entry
        };

        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            crate_name: Default::default(),
            entry: Default::default(),
            jni: Default::default(),
            output: ["target", "riko"].iter().collect(),
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct JniConfig {
    pub enabled: bool, // TODO: Use `targets = ["jni", "pinvoke"]`
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
            path: ["src", "lib.rs"].iter().collect(),
        }
    }
}
