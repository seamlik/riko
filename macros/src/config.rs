use proc_macro::Span;
use serde::Deserialize;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;

lazy_static::lazy_static! {
    pub static ref CURRENT: Mutex<Option<Config>> = Mutex::default();
}

const CONFIG_FILENAME: &str = "Riko.toml";

/// Gets the config of the current crate.
pub fn current() -> Config {
    let mut current = CURRENT.lock().expect("Failed to lock the static config!");
    if let Some(config) = &*current {
        config.clone()
    } else {
        let config = read();
        *current = Some(config.clone());
        config
    }
}

/// Reads config from filesystem.
fn read() -> Config {
    let source_file = root_span(Span::call_site()).source_file();
    if source_file.is_real() {
        let config_path = locate(&source_file.path()).expect("Error while locating Riko config!");
        match config_path {
            Some(path) => {
                let raw = std::fs::read(path).expect("Failed to read Riko config!");
                toml::from_slice(&raw).expect("Failed to parse Riko config!")
            }
            None => {
                eprintln!("No Riko configuration found, skipping code generation.");
                Config::default()
            }
        }
    } else {
        Config::default()
    }
}

/// Locates a config upwards from the source file being expanded.
///
/// First look for `Riko.toml` in the directory containing the source file, then continue searching
/// its parent directory until the root is reached.
fn locate(source_path: &Path) -> std::io::Result<Option<PathBuf>> {
    let mut config_path = source_path.canonicalize()?;
    if !config_path.is_dir() {
        config_path.pop();
    }
    loop {
        config_path.push(CONFIG_FILENAME);
        if config_path.is_file() {
            break Ok(Some(config_path));
        } else if config_path.parent().unwrap().parent().is_none() {
            break Ok(None); // Reached root
        } else {
            config_path.pop();
            config_path.pop();
        }
    }
}

/// Recursively finds the root [Span] of a [Span].
fn root_span(span: Span) -> Span {
    match span.parent() {
        Some(parent) => root_span(parent),
        None => span,
    }
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub jni: JniConfig,
    pub module: String,
    pub output: PathBuf,
}

#[derive(Deserialize, Default, Clone)]
pub struct JniConfig {
    pub enabled: bool,
}
