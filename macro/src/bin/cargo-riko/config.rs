//! Configuration.

use anyhow::Context;
use cargo_metadata::Metadata;
use cargo_metadata::MetadataCommand;
use cargo_metadata::Package;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::collections::HashSet;
use std::path::PathBuf;

/// Configuration.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Config {
    /// What target code should be generated.
    pub targets: BTreeSet<String>,

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
            let mut config: Config = match &pkg.metadata {
                Value::Object(value) => {
                    if let Some(raw) = value.get("riko") {
                        serde_json::from_value::<Config>(raw.clone())?
                    } else {
                        Default::default()
                    }
                }
                _ => Default::default(),
            };
            config.expand_all_fields(pkg, &metadata)?;
            configs.push(config);
        }
        Ok(configs)
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
