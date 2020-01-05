use anyhow::Context;
use cargo_metadata::MetadataCommand;
use riko_core::config::Config;
use riko_core::ir::Crate;
use riko_core::jni::JniWriter;
use riko_core::TargetCodeWriter;
use std::collections::HashSet;

pub fn main() {
    if std::env::args().len() > 1 {
        panic!("No arguments are allowed.")
    }
    if let Err(err) = run() {
        handle_error(err);
    }
}

fn run() -> anyhow::Result<()> {
    let configs = read_all_configs().unwrap();
    for config in configs.iter() {
        if config.jni.enabled {
            let ir = Crate::parse(&config.entry, config.crate_name.clone())?;
            let writer = JniWriter::new(config);
            writer.write_all(&ir)?;
        }
    }
    Ok(())
}

/// Reads the top-level `Cargo.toml` and reads the `Riko.toml` of all crates in the workspace.
fn read_all_configs() -> anyhow::Result<Vec<Config>> {
    let metadata = MetadataCommand::new().exec()?;
    let workspace_members_ids = metadata.workspace_members.iter().collect::<HashSet<_>>();
    let mut configs = Vec::new();
    for pkg in metadata
        .packages
        .into_iter()
        .filter(|x| workspace_members_ids.contains(&x.id))
    {
        let config_path = pkg
            .manifest_path
            .with_file_name(riko_core::config::FILENAME);
        let config = Config::read(&config_path)
            .with_context(|| format!("Failed to read config {}", config_path.display()))?;
        configs.push(config);
    }
    Ok(configs)
}

fn handle_error(err_root: anyhow::Error) -> ! {
    if let Some(err) = err_root.downcast_ref::<syn::Error>() {
        let location = err.span().start();
        eprintln!(
            "Syntax error at line {} column {}",
            location.line, location.column
        );
    }
    let result: anyhow::Result<()> = Err(err_root);
    result.unwrap();
    std::process::exit(1);
}
