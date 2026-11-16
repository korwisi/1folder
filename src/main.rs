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
mod tags;
mod undo;
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
    #[arg(
        long,
        value_name = "N|all",
        num_args = 0..=1,
        default_missing_value = "1",
        help = "Undo moves recorded in the journal: the last N items, or 'all'"
    )]
    undo: Option<String>,
    #[arg(
        long,
        value_name = "DURATION",
        help = "With --undo: only items moved within this duration, e.g. 30m, 2h, 1d"
    )]
    within: Option<String>,
    #[arg(
        long,
        value_name = "DIR",
        help = "With --undo: only items originally moved from under this directory"
    )]
    src_prefix: Option<PathBuf>,
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();

    if cli.init_config {
        return init_config(cli.config);
    }

    let config_path = match cli.config.clone() {
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

    if let Some(undo_arg) = &cli.undo {
        return run_undo(&config, &cli, undo_arg);
    }

    if cli.once {
        watch::run_once(&config, cli.dry_run)
    } else {
        watch::run_watch(&config, cli.dry_run)
    }
}

fn run_undo(config: &config::Config, cli: &Cli, undo_arg: &str) -> Result<()> {
    let m = i18n::Messages::get(config.general.language);
    let all = undo_arg.eq_ignore_ascii_case("all");
    let count = undo_arg.parse::<usize>().ok();
    if !all && count.is_none() {
        anyhow::bail!(
            "{}",
            i18n::fmt(m.undo_invalid_target, &[&undo_arg])
        );
    }
    let within = match &cli.within {
        Some(raw) => match undo::parse_duration(raw) {
            Some(duration) => Some(duration),
            None => anyhow::bail!("{}", i18n::fmt(m.undo_invalid_within, &[raw])),
        },
        None => None,
    };
    let selection = undo::UndoSelection {
        all,
        count,
        within,
        src_prefix: cli.src_prefix.as_deref().map(config::expand_tilde),
    };
    undo::run(config, cli.dry_run, &selection)
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
