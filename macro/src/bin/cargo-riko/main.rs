mod config;

use config::Config;
use riko_core::ir::Crate;

pub fn main() -> anyhow::Result<()> {
    if std::env::args().len() > 1 {
        eprintln!("No arguments are allowed.");
        std::process::exit(1);
    }

    let configs = Config::read_all_configs()?;
    for config in configs.iter() {
        for writer in riko_core::create_target_code_writers(config.targets.iter()).into_iter() {
            let ir = Crate::parse(&config.cached.entry, config.cached.crate_name.clone())?;

            let mut output_directory = config.cached.output_directory.clone();
            output_directory.push("jni");

            writer.write_all(&ir, &output_directory)?;
        }
    }
    Ok(())
}
