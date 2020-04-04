mod config;

use config::Config;
use riko_core::ir::Crate;

pub fn main() -> anyhow::Result<()> {
    env_logger::init();
    for config in Config::read_all_configs()?.iter() {
        if format!("{}", config.cached.entry.display()).is_empty() {
            log::warn!(
                "Package `{}` does not have a `cdylib` target, skipping.",
                &config.cached.crate_name
            );
            continue;
        }

        // Remove all generated code first because they interfere with the IR scanning
        let _ = std::fs::remove_dir_all(&config.cached.output_directory);

        riko_core::bindgen(
            &Crate::parse(&config.cached.entry, config.cached.crate_name.clone())?,
            &config.cached.output_directory,
            config.targets.iter(),
        )?;
    }
    Ok(())
}
