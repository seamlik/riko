mod config;

use config::Config;
use riko_core::ir::Crate;
use riko_core::jni::JniWriter;
use riko_core::TargetCodeWriter;

pub fn main() -> anyhow::Result<()> {
    if std::env::args().len() > 1 {
        eprintln!("No arguments are allowed.");
        std::process::exit(1);
    }

    let configs = Config::read_all_configs()?;
    for config in configs.iter() {
        if config.jni.enabled {
            let ir = Crate::parse(&config.cached.entry, config.cached.crate_name.clone())?;
            let mut output_directory = config.cached.output_directory.clone();
            output_directory.push("jni");
            let writer = JniWriter { output_directory };
            writer.write_all(&ir)?;
        }
    }
    Ok(())
}
