mod config;

use config::Config;
use riko_core::ir::Crate;

pub fn main() -> anyhow::Result<()> {
    if std::env::args().len() > 1 {
        eprintln!("No arguments are allowed.");
        std::process::exit(1);
    }

    for config in Config::read_all_configs()?.iter() {
        if format!("{}", config.cached.entry.display()).is_empty() {
            log::info!(
                "Package `{}` does not have a `cdylib` target, skipping.",
                &config.cached.crate_name
            );
            continue;
        }
        riko_core::bindgen(
            &Crate::parse(&config.cached.entry, config.cached.crate_name.clone())?,
            &config.cached.output_directory,
            config.targets.iter(),
        )?;
    }
    Ok(())
}
