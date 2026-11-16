use crate::config::{Config, FolderMode};
use crate::i18n::Messages;
use crate::organize::{self, process_file, process_folder, HashIndex};
use anyhow::Result;
use notify::event::{AccessKind, AccessMode, ModifyKind};
use notify::{Event, EventKind, RecursiveMode, Watcher, recommended_watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{RecvTimeoutError, TryRecvError};
use std::time::{Duration, Instant};

pub fn run_watch(config: &Config, dry_run: bool) -> Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = recommended_watcher(tx)?;

    let m = Messages::get(config.general.language);
    for dir in &config.watch.dirs {
        if !dir.is_dir() {
            log::warn!("{}", crate::i18n::fmt(m.watch_dir_missing, &[&dir.display()]));
            continue;
        }
        watcher.watch(dir, RecursiveMode::Recursive)?;
        log::info!("{}", crate::i18n::fmt(m.watching, &[&dir.display()]));
    }
    log::info!("{}", crate::i18n::fmt(m.target_folder, &[&config.target_dir.display()]));
    log::info!("{}", m.running_hint);

    let debounce = Duration::from_millis(config.watch.debounce_ms);
    let mut pending: HashMap<PathBuf, Instant> = HashMap::new();
    let mut index = HashIndex::new();

    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(event) => handle_event(event, &mut pending, debounce, m),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        process_due(&mut pending, config, dry_run, debounce, &mut index);

        loop {
            match rx.try_recv() {
                Ok(event) => handle_event(event, &mut pending, debounce, m),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }
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
                let ready = Instant::now().checked_sub(debounce).unwrap_or_else(Instant::now);
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
    config: &Config,
    dry_run: bool,
    debounce: Duration,
    index: &mut HashIndex,
) {
    let now = Instant::now();
    let due: Vec<PathBuf> = pending
        .iter()
        .filter(|(_, seen)| now.duration_since(**seen) >= debounce)
        .map(|(path, _)| path.clone())
        .collect();
    for path in due {
        pending.remove(&path);
        let unit = unit_root(config, &path);
        let result = if unit.is_dir() {
            process_folder(config, dry_run, &unit)
        } else {
            process_file(config, dry_run, &path, index)
        };
        if let Err(e) = result {
            let m = Messages::get(config.general.language);
            log::error!(
                "{}",
                crate::i18n::fmt(m.error_processing, &[&path.display(), &format!("{e:#}")])
            );
        }
    }
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

pub fn run_once(config: &Config, dry_run: bool) -> Result<()> {
    let m = Messages::get(config.general.language);
    let mut files: Vec<PathBuf> = Vec::new();
    let mut folders: Vec<PathBuf> = Vec::new();
    for dir in &config.watch.dirs {
        if !dir.is_dir() {
            log::warn!("{}", crate::i18n::fmt(m.watch_dir_missing, &[&dir.display()]));
            continue;
        }
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .collect()
            })
            .unwrap_or_default();
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
    let mut index = HashIndex::new();
    let mut moved = 0usize;
    for dir in &folders {
        match process_folder(config, dry_run, dir) {
            Ok(Some(_)) => moved += 1,
            Ok(None) => {}
            Err(e) => log::error!(
                "{}",
                crate::i18n::fmt(m.error_processing, &[&dir.display(), &format!("{e:#}")])
            ),
        }
    }
    for path in &files {
        match process_file(config, dry_run, path, &mut index) {
            Ok(Some(outcome)) if outcome.action == organize::Action::Moved => {
                moved += 1;
                log::info!(
                    "{:?}: {} → {}",
                    outcome.action,
                    outcome.src.display(),
                    outcome.dst.display()
                );
            }
            Ok(Some(outcome)) => log::info!(
                "{:?}: {} (category {}, sha256 {})",
                outcome.action,
                outcome.src.display(),
                outcome.category,
                &outcome.sha256[..12.min(outcome.sha256.len())]
            ),
            Ok(None) => {}
            Err(e) => log::error!(
                "{}",
                crate::i18n::fmt(m.error_processing, &[&path.display(), &format!("{e:#}")])
            ),
        }
    }
    log::info!("{}", crate::i18n::fmt(m.done_sorted, &[&moved]));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn browser_temp_files_are_detected() {
        assert!(organize::is_partial_download(Path::new("~/Downloads/ubuntu.iso.crdownload")));
        assert!(organize::is_partial_download(Path::new("~/Downloads/bericht.pdf.part")));
        assert!(organize::is_partial_download(Path::new("~/Downloads/bericht.zip.download/file.zip")));
        assert!(organize::is_partial_download(Path::new("~/Downloads/datei.tmp")));
    }

    #[test]
    fn finished_files_are_not_detected_as_partial() {
        assert!(!organize::is_partial_download(Path::new("~/Downloads/ubuntu.iso")));
        assert!(!organize::is_partial_download(Path::new("~/Downloads/bericht.pdf")));
        assert!(!organize::is_partial_download(Path::new("~/Downloads/tmp_foto.jpg")));
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
}
