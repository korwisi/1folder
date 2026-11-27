use crate::config::{Config, FolderMode};
use crate::i18n::Messages;
use crate::organize::{self, process_file, process_folder, HashIndex};
use crate::report;
use anyhow::Result;
use notify::event::{AccessKind, AccessMode, ModifyKind};
use notify::{recommended_watcher, Event, EventKind, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{RecvTimeoutError, TryRecvError};
use std::time::{Duration, Instant};

const MAX_PROCESS_ATTEMPTS: usize = 10;
const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);

fn retry_delay(attempt: usize, debounce: Duration) -> Duration {
    let multiplier = 1usize << attempt.min(5);
    let delay = debounce
        .saturating_mul(multiplier as u32)
        .min(MAX_RETRY_DELAY);
    if delay.is_zero() {
        Duration::from_millis(200)
    } else {
        delay
    }
}

pub fn run_watch(config: &Config, dry_run: bool, report_path: Option<&Path>) -> Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = recommended_watcher(tx)?;

    let m = Messages::get(config.general.language);
    for dir in &config.watch.dirs {
        if !dir.is_dir() {
            log::warn!(
                "{}",
                crate::i18n::fmt(m.watch_dir_missing, &[&dir.display()])
            );
            continue;
        }
        watcher.watch(dir, RecursiveMode::Recursive)?;
        log::info!("{}", crate::i18n::fmt(m.watching, &[&dir.display()]));
    }
    log::info!(
        "{}",
        crate::i18n::fmt(m.target_folder, &[&config.target_dir.display()])
    );
    log::info!("{}", m.running_hint);

    let debounce = Duration::from_millis(config.watch.debounce_ms);
    let mut pending: HashMap<PathBuf, Instant> = HashMap::new();
    let mut retry_attempts: HashMap<PathBuf, usize> = HashMap::new();
    let mut index = HashIndex::with_target(&config.target_dir)?;

    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(event) => handle_event(event, &mut pending, debounce, m),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        process_due(
            &mut pending,
            &mut retry_attempts,
            config,
            dry_run,
            debounce,
            &mut index,
            report_path,
        )?;

        loop {
            match rx.try_recv() {
                Ok(event) => handle_event(event, &mut pending, debounce, m),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if !dry_run {
                        index.flush().ok();
                    }
                    return Ok(());
                }
            }
        }
    }
    if !dry_run {
        index.flush()?;
    }
    Ok(())
}

fn handle_event(
    event: Result<Event, notify::Error>,
    pending: &mut HashMap<PathBuf, Instant>,
    debounce: Duration,
    m: &'static Messages,
) {
    let event = match event {
        Ok(event) => event,
        Err(e) => {
            log::warn!("{}", crate::i18n::fmt(m.event_failed, &[&e]));
            return;
        }
    };
    match event.kind {
        EventKind::Remove(_) => {
            for path in event.paths {
                pending.remove(&path);
            }
        }
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => {
            // File handle closed after writing: for browser downloads this is the
            // strongest completion signal, so the file is ready immediately.
            for path in event.paths {
                let ready = Instant::now()
                    .checked_sub(debounce)
                    .unwrap_or_else(Instant::now);
                pending.insert(path, ready);
            }
        }
        EventKind::Modify(ModifyKind::Name(_)) => {
            // Browsers finish downloads by renaming the temp file. Track the new
            // name (event.paths) and drop stale entries that no longer exist.
            for path in event.paths {
                if path.exists() {
                    pending.insert(path, Instant::now());
                } else {
                    pending.remove(&path);
                }
            }
        }
        _ => {
            for path in event.paths {
                pending.insert(path, Instant::now());
            }
        }
    }
}

fn process_due(
    pending: &mut HashMap<PathBuf, Instant>,
    retry_attempts: &mut HashMap<PathBuf, usize>,
    config: &Config,
    dry_run: bool,
    debounce: Duration,
    index: &mut HashIndex,
    report_path: Option<&Path>,
) -> Result<()> {
    let now = Instant::now();
    let due: Vec<PathBuf> = pending
        .iter()
        .filter(|(_, seen)| now.duration_since(**seen) >= debounce)
        .map(|(path, _)| path.clone())
        .collect();
    let has_due = !due.is_empty();
    for path in due {
        pending.remove(&path);
        let unit = unit_root(config, &path);
        let result = if unit.is_dir() {
            process_folder(config, dry_run, &unit)
        } else {
            process_file(config, dry_run, &path, index)
        };
        match result {
            Ok(Some(outcome)) => {
                retry_attempts.remove(&path);
                if let Err(e) = write_report(config, dry_run, report_path, Some(&outcome)) {
                    let m = Messages::get(config.general.language);
                    log::warn!(
                        "{}",
                        crate::i18n::fmt(
                            m.error_processing,
                            &[&path.display(), &format!("report write failed: {e:#}")],
                        )
                    );
                }
            }
            Ok(None) => {
                retry_attempts.remove(&path);
            }
            Err(e) => {
                let m = Messages::get(config.general.language);
                log::error!(
                    "{}",
                    crate::i18n::fmt(m.error_processing, &[&path.display(), &format!("{e:#}")])
                );
                let attempts = retry_attempts.entry(path.clone()).or_insert(0);
                *attempts += 1;
                if *attempts < MAX_PROCESS_ATTEMPTS {
                    let delay = retry_delay(*attempts, debounce);
                    pending.insert(
                        path.clone(),
                        Instant::now()
                            .checked_add(delay)
                            .unwrap_or_else(Instant::now),
                    );
                    log::warn!(
                        "{}",
                        crate::i18n::fmt(
                            m.error_processing,
                            &[
                                &path.display(),
                                &format!(
                                    "retrying in {}ms (attempt {}/{})",
                                    delay.as_millis(),
                                    *attempts,
                                    MAX_PROCESS_ATTEMPTS
                                )
                            ],
                        )
                    );
                } else {
                    log::error!(
                        "{}",
                        crate::i18n::fmt(
                            m.error_processing,
                            &[&path.display(), &"retry limit reached"],
                        )
                    );
                    retry_attempts.remove(&path);
                }
            }
        }
    }
    if has_due && !dry_run {
        index.flush()?;
    }
    Ok(())
}

fn write_report(
    _config: &Config,
    dry_run: bool,
    report_path: Option<&Path>,
    outcome: Option<&organize::Outcome>,
) -> Result<()> {
    let (Some(report_path), Some(outcome)) = (report_path, outcome) else {
        return Ok(());
    };
    report::append(report_path, outcome, dry_run)
}

fn unit_root(config: &Config, path: &Path) -> PathBuf {
    if config.folders.mode != FolderMode::MoveAsUnit {
        return path.to_path_buf();
    }
    for dir in &config.watch.dirs {
        if let Ok(rel) = path.strip_prefix(dir) {
            if let Some(first) = rel.components().next() {
                return dir.join(first);
            }
        }
    }
    path.to_path_buf()
}

pub fn run_once(config: &Config, dry_run: bool, report_path: Option<&Path>) -> Result<()> {
    let m = Messages::get(config.general.language);
    let mut files: Vec<PathBuf> = Vec::new();
    let mut folders: Vec<PathBuf> = Vec::new();
    let mut enumeration_failed = false;
    for dir in &config.watch.dirs {
        if !dir.is_dir() {
            log::warn!(
                "{}",
                crate::i18n::fmt(m.watch_dir_missing, &[&dir.display()])
            );
            continue;
        }
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) => {
                enumeration_failed = true;
                log::error!(
                    "{}",
                    crate::i18n::fmt(
                        m.watched_dir_unreadable,
                        &[&format!("{}: {}", dir.display(), e)]
                    )
                );
                continue;
            }
        };
        let mut entry_paths = Vec::new();
        for entry in entries {
            match entry {
                Ok(entry) => entry_paths.push(entry.path()),
                Err(e) => {
                    enumeration_failed = true;
                    log::error!(
                        "{}",
                        crate::i18n::fmt(
                            m.watched_entry_unreadable,
                            &[&dir.display(), &format!("{e}")],
                        )
                    );
                }
            }
        }
        let mut entries = entry_paths;
        entries.sort();
        for path in entries {
            if path.is_dir() {
                if config.folders.mode == FolderMode::MoveAsUnit
                    && organize::folder_should_process(config, &path)
                {
                    folders.push(path);
                }
            } else if organize::should_process(config, &path) {
                files.push(path);
            }
        }
    }
    log::info!(
        "{}",
        crate::i18n::fmt(m.items_to_check, &[&(files.len() + folders.len())])
    );
    let mut index = HashIndex::with_target(&config.target_dir)?;
    let mut moved = 0usize;
    let mut processing_failed = false;
    for dir in &folders {
        match process_folder(config, dry_run, dir) {
            Ok(Some(outcome)) => {
                moved += 1;
                if let Err(e) = write_report(config, dry_run, report_path, Some(&outcome)) {
                    processing_failed = true;
                    log::error!(
                        "{}",
                        crate::i18n::fmt(m.report_write_failed, &[&format!("{e:#}")])
                    );
                }
            }
            Ok(None) => {}
            Err(e) => {
                processing_failed = true;
                log::error!(
                    "{}",
                    crate::i18n::fmt(m.error_processing, &[&dir.display(), &format!("{e:#}")])
                );
            }
        }
    }
    for path in &files {
        match process_file(config, dry_run, path, &mut index) {
            Ok(Some(outcome)) if outcome.action == organize::Action::Moved => {
                moved += 1;
                if let Err(e) = write_report(config, dry_run, report_path, Some(&outcome)) {
                    processing_failed = true;
                    log::error!(
                        "{}",
                        crate::i18n::fmt(m.report_write_failed, &[&format!("{e:#}")])
                    );
                }
                log::info!(
                    "{:?}: {} → {}",
                    outcome.action,
                    outcome.src.display(),
                    outcome.dst.display()
                );
            }
            Ok(Some(outcome)) => {
                log::info!(
                    "{:?}: {} (category {}, sha256 {})",
                    outcome.action,
                    outcome.src.display(),
                    outcome.category,
                    &outcome.sha256[..12.min(outcome.sha256.len())]
                );
                if let Err(e) = write_report(config, dry_run, report_path, Some(&outcome)) {
                    processing_failed = true;
                    log::error!(
                        "{}",
                        crate::i18n::fmt(m.report_write_failed, &[&format!("{e:#}")])
                    );
                }
            }
            Ok(None) => {}
            Err(e) => {
                processing_failed = true;
                log::error!(
                    "{}",
                    crate::i18n::fmt(m.error_processing, &[&path.display(), &format!("{e:#}")])
                );
            }
        }
    }
    log::info!("{}", crate::i18n::fmt(m.done_sorted, &[&moved]));
    if !dry_run {
        index.flush()?;
    }
    if enumeration_failed || processing_failed {
        anyhow::bail!("One or more watched entries could not be processed");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn retry_delay_has_bounded_exponential_backoff() {
        let debounce = Duration::from_millis(100);
        assert_eq!(retry_delay(1, debounce), Duration::from_millis(200));
        assert_eq!(retry_delay(5, debounce), Duration::from_millis(3200));
        assert_eq!(retry_delay(100, Duration::from_secs(10)), MAX_RETRY_DELAY);
    }

    #[test]
    fn browser_temp_files_are_detected() {
        assert!(organize::is_partial_download(Path::new(
            "~/Downloads/ubuntu.iso.crdownload"
        )));
        assert!(organize::is_partial_download(Path::new(
            "~/Downloads/bericht.pdf.part"
        )));
        assert!(organize::is_partial_download(Path::new(
            "~/Downloads/bericht.zip.download/file.zip"
        )));
        assert!(organize::is_partial_download(Path::new(
            "~/Downloads/datei.tmp"
        )));
    }

    #[test]
    fn finished_files_are_not_detected_as_partial() {
        assert!(!organize::is_partial_download(Path::new(
            "~/Downloads/ubuntu.iso"
        )));
        assert!(!organize::is_partial_download(Path::new(
            "~/Downloads/bericht.pdf"
        )));
        assert!(!organize::is_partial_download(Path::new(
            "~/Downloads/tmp_foto.jpg"
        )));
    }

    fn watch_config(dirs: &[&str]) -> Config {
        let mut config: Config = toml::from_str(crate::config::DEFAULT_CONFIG).unwrap();
        config.watch.dirs = dirs.iter().map(PathBuf::from).collect();
        config
    }

    #[test]
    fn unit_root_maps_to_top_level_folder_in_unit_mode() {
        let config = watch_config(&["/watched"]);
        assert_eq!(
            unit_root(&config, Path::new("/watched/repo/src/main.rs")),
            PathBuf::from("/watched/repo")
        );
        assert_eq!(
            unit_root(&config, Path::new("/watched/repo")),
            PathBuf::from("/watched/repo")
        );
        assert_eq!(
            unit_root(&config, Path::new("/watched/loose.txt")),
            PathBuf::from("/watched/loose.txt")
        );
        assert_eq!(
            unit_root(&config, Path::new("/elsewhere/file.txt")),
            PathBuf::from("/elsewhere/file.txt")
        );
    }

    #[test]
    fn unit_root_returns_path_itself_in_ignore_mode() {
        let mut config = watch_config(&["/watched"]);
        config.folders.mode = crate::config::FolderMode::Ignore;
        assert_eq!(
            unit_root(&config, Path::new("/watched/repo/src/main.rs")),
            PathBuf::from("/watched/repo/src/main.rs")
        );
    }

    #[test]
    fn once_mode_dry_run_has_no_side_effects() {
        let tmp = tempfile::tempdir().unwrap();
        let mut config: Config = toml::from_str(crate::config::DEFAULT_CONFIG).unwrap();
        config.target_dir = tmp.path().join("target");
        config.watch.dirs = vec![tmp.path().join("Desktop")];
        config.notify.enabled = false;
        let watched = tmp.path().join("Desktop");
        std::fs::create_dir_all(&watched).unwrap();
        std::fs::write(watched.join("foto.jpg"), b"j").unwrap();
        let repo = watched.join("some-repo");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::write(repo.join("main.rs"), b"fn main() {}").unwrap();
        std::fs::write(repo.join("README.md"), b"# repo").unwrap();
        let report_path = tmp.path().join("report.jsonl");
        let main_snapshot = std::fs::read(repo.join("main.rs")).unwrap();
        let readme_snapshot = std::fs::read(repo.join("README.md")).unwrap();
        let foto_snapshot = std::fs::read(watched.join("foto.jpg")).unwrap();

        run_once(&config, true, Some(&report_path)).unwrap();

        assert_eq!(
            std::fs::read(watched.join("foto.jpg")).unwrap(),
            foto_snapshot
        );
        assert_eq!(std::fs::read(repo.join("main.rs")).unwrap(), main_snapshot);
        assert_eq!(
            std::fs::read(repo.join("README.md")).unwrap(),
            readme_snapshot
        );
        assert!(!config.target_dir.exists());
        assert!(!config.journal_path().exists());
        assert!(!config.target_dir.join(".1folder_index.json").exists());
        assert!(!report_path.exists());
        assert!(watched.join("foto.jpg").exists());
        assert!(repo.exists());
    }
}
