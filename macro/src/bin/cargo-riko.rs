use riko_core::config::Config;
use riko_core::ir::Crate;
use riko_core::jni::JniWriter;
use riko_core::TargetCodeWriter;

pub fn main() {
    if std::env::args().len() > 1 {
        panic!("No arguments are allowed.")
    }
    if let Err(err) = run() {
        handle_error(err);
    }
}

fn run() -> anyhow::Result<()> {
    let configs = Config::read_all_configs()?;
    for config in configs.iter() {
        if config.jni.enabled {
            let ir = Crate::parse(&config.cached.entry, config.cached.crate_name.clone())?;
            let writer = JniWriter::new(config);
            writer.write_all(&ir)?;
        }
    }
    Ok(())
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
