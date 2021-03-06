mod config;

use config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    for config in Config::read_all_configs()?.iter() {
        if config.cached.entry.iter().next().is_none() {
            log::warn!(
                "Package `{}` does not have a `cdylib` or `lib` target, skipping…",
                &config.cached.crate_name
            );
            continue;
        } else if config.targets.is_empty() {
            log::warn!(
                "Package `{}` does not specify any Riko target, skipping…",
                &config.cached.crate_name
            );
            continue;
        }

        log::info!(
            "Generating language bindings for crate `{}` with entry `{}`",
            &config.cached.crate_name,
            config.cached.entry.display()
        );
        riko_core::bindgen(
            &config.cached.crate_name,
            &config.cached.entry,
            &config.cached.output_directory,
            config.targets.iter(),
        )
        .await?;
    }
    Ok(())
}
