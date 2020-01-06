//! Configuration.

use anyhow::Context;
use cargo_metadata::Metadata;
use cargo_metadata::MetadataCommand;
use cargo_metadata::Package;
use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

/// Filename of a Riko config.
const FILENAME: &str = "Riko.toml";

/// `Riko.toml`.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Config {
    // TODO: Use `Cargo.toml` package metadata
    pub jni: JniConfig,

    #[serde(skip)]
    pub cached: ConfigCachedFields,
}

impl Config {
    /// Reads the top-level `Cargo.toml` and reads the `Riko.toml` of all crates in the workspace.
    pub fn read_all_configs() -> anyhow::Result<Vec<Config>> {
        let metadata = MetadataCommand::new().exec()?;
        let workspace_members_ids = metadata.workspace_members.iter().collect::<HashSet<_>>();
        let mut configs = Vec::new();
        for pkg in metadata
            .packages
            .iter()
            .filter(|x| workspace_members_ids.contains(&x.id))
        {
            let config_path = pkg.manifest_path.with_file_name(FILENAME);
            let mut config = Config::read(&config_path)
                .with_context(|| format!("Failed to read config {}", config_path.display()))?;
            config.expand_all_fields(pkg, &metadata)?;
            configs.push(config);
        }
        Ok(configs)
    }

    /// Reads config from filesystem.
    ///
    /// Returns a default config if the file is not found.
    pub fn read(path: &Path) -> anyhow::Result<Self> {
        if !path.is_file() {
            return Ok(Default::default());
        }

        let config: Config = toml::from_slice(&std::fs::read(path)?)?;
        Ok(config)
    }

    /// Fills in all optional fields, expands all relative filesystem paths, etc..
    fn expand_all_fields(&mut self, package: &Package, metadata: &Metadata) -> anyhow::Result<()> {
        let cargo_config_raw = std::fs::read(&package.manifest_path)
            .with_context(|| package.manifest_path.display().to_string())?;
        let cargo_config: CargoConfig = toml::from_slice(&cargo_config_raw)
            .with_context(|| package.manifest_path.display().to_string())?;

        self.cached.crate_name = package.name.clone();

        self.cached.output_directory = metadata.target_directory.clone();
        self.cached.output_directory.push("riko");

        self.cached.entry = if cargo_config.lib.path.is_absolute() {
            cargo_config.lib.path
        } else {
            let mut entry: PathBuf = package.manifest_path.to_owned();
            entry.pop();
            entry.extend(cargo_config.lib.path.iter());
            entry
        };

        Ok(())
    }
}

/// Fields not in a Cargo config but cached for further uses.
pub struct ConfigCachedFields {
    /// Where Cargo places all its generated target code.
    pub output_directory: PathBuf,

    /// Crate name.
    pub crate_name: String,

    /// Entry source file of this crate.
    pub entry: PathBuf,
}

impl Default for ConfigCachedFields {
    fn default() -> Self {
        Self {
            crate_name: Default::default(),
            entry: Default::default(),
            output_directory: ["target", "riko"].iter().collect(),
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct JniConfig {
    pub enabled: bool, // TODO: Use `targets = ["jni", "pinvoke"]`
}

/// Minified version of a `Cargo.toml`.
#[derive(Deserialize, Default)]
#[serde(default)]
struct CargoConfig {
    lib: CargoConfigLib,
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
