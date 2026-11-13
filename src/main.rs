use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

mod classify;
mod config;
mod hash;
mod i18n;
mod journal;
mod notify;
mod organize;
mod watch;

#[derive(Parser)]
#[command(
    name = "1folder",
    version,
    about = "Keeps one folder organized: watches folders, deduplicates and sorts files."
)]
struct Cli {
    #[arg(long, value_name = "FILE")]
    config: Option<PathBuf>,
    #[arg(long, help = "Scan the watched folders once and exit")]
    once: bool,
    #[arg(long, help = "Only show what would happen, change nothing")]
    dry_run: bool,
    #[arg(long, help = "Write the default configuration and exit")]
    init_config: bool,
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();

    if cli.init_config {
        return init_config(cli.config);
    }

    let config_path = match cli.config {
        Some(path) => path,
        None => config::default_config_path().context(
            "No default config location found on this system; pass --config <FILE> explicitly",
        )?,
    };
    if !config_path.exists() {
        anyhow::bail!(
            "Config not found: {}\nRun `onefolder --init-config` first or pass --config <FILE>.",
            config_path.display()
        );
    }
    let config = config::Config::load(&config_path)?;
    log::debug!("Config loaded from {}", config_path.display());

    if cli.once {
        watch::run_once(&config, cli.dry_run)
    } else {
        watch::run_watch(&config, cli.dry_run)
    }
}

fn init_config(path: Option<PathBuf>) -> Result<()> {
    let path = match path {
        Some(p) => p,
        None => config::default_config_path()
            .context("No default config location found; pass --config <FILE>")?,
    };
    if path.exists() {
        anyhow::bail!("Config already exists: {}", path.display());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Directory not creatable: {}", parent.display()))?;
    }
    std::fs::write(&path, config::DEFAULT_CONFIG)
        .with_context(|| format!("Config not writable: {}", path.display()))?;
    println!("Default config written: {}", path.display());
    Ok(())
}
