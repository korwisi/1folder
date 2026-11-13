use crate::config::Config;
use crate::organize::{self, process_file, HashIndex};
use anyhow::Result;
use notify::event::{AccessKind, AccessMode, ModifyKind};
use notify::{Event, EventKind, RecursiveMode, Watcher, recommended_watcher};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{RecvTimeoutError, TryRecvError};
use std::time::{Duration, Instant};

pub fn run_watch(config: &Config, dry_run: bool) -> Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = recommended_watcher(tx)?;

    for dir in &config.watch.dirs {
        if !dir.is_dir() {
            log::warn!(
                "Überwachter Ordner existiert nicht, wird übersprungen: {}",
                dir.display()
            );
            continue;
        }
        watcher.watch(dir, RecursiveMode::Recursive)?;
        log::info!("Überwache: {}", dir.display());
    }
    log::info!("Zielordner: {}", config.target_dir.display());
    log::info!("1folder läuft – beenden mit Strg+C");

    let debounce = Duration::from_millis(config.watch.debounce_ms);
    let mut pending: HashMap<PathBuf, Instant> = HashMap::new();
    let mut index = HashIndex::new();

    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(event) => handle_event(event, &mut pending, debounce),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        process_due(&mut pending, config, dry_run, debounce, &mut index);

        loop {
            match rx.try_recv() {
                Ok(event) => handle_event(event, &mut pending, debounce),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }
    }
    Ok(())
}

fn handle_event(event: Result<Event, notify::Error>, pending: &mut HashMap<PathBuf, Instant>, debounce: Duration) {
    let event = match event {
        Ok(event) => event,
        Err(e) => {
            log::warn!("Dateisystem-Ereignis fehlgeschlagen: {e}");
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
        if let Err(e) = process_file(config, dry_run, &path, index) {
            log::error!("Fehler bei {}: {e:#}", path.display());
        }
    }
}

pub fn run_once(config: &Config, dry_run: bool) -> Result<()> {
    let mut files: Vec<PathBuf> = Vec::new();
    for dir in &config.watch.dirs {
        if !dir.is_dir() {
            log::warn!("Ordner existiert nicht, wird übersprungen: {}", dir.display());
            continue;
        }
        for entry in walkdir::WalkDir::new(dir).follow_links(false) {
            match entry {
                Ok(entry) => {
                    let path = entry.path();
                    if organize::should_process(config, path) {
                        files.push(path.to_path_buf());
                    }
                }
                Err(e) => log::warn!("Durchsuchen fehlgeschlagen: {e}"),
            }
        }
    }
    files.sort();
    log::info!("{} Datei(en) zu prüfen", files.len());
    let mut index = HashIndex::new();
    let mut moved = 0usize;
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
                "{:?}: {} (Kategorie {}, sha256 {})",
                outcome.action,
                outcome.src.display(),
                outcome.category,
                &outcome.sha256[..12.min(outcome.sha256.len())]
            ),
            Ok(None) => {}
            Err(e) => log::error!("Fehler bei {}: {e:#}", path.display()),
        }
    }
    log::info!("Fertig: {moved} Datei(en) einsortiert");
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
}
