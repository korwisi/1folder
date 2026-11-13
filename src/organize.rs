use crate::classify::{self, is_hidden, is_under};
use crate::config::{Config, DuplicateAction};
use crate::hash::sha256_file;
use crate::i18n::Messages;
use crate::journal;
use crate::notify;
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

const PARTIAL_DOWNLOAD_EXTS: &[&str] = &["crdownload", "part", "partial", "download", "tmp"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Moved,
    DuplicateSkipped,
    KeptBoth,
    MovedToDuplicates,
}

impl Action {
    pub fn as_str(&self) -> &'static str {
        match self {
            Action::Moved => "moved",
            Action::DuplicateSkipped => "duplicate_skipped",
            Action::KeptBoth => "kept_both",
            Action::MovedToDuplicates => "moved_to_duplicates",
        }
    }
}

#[derive(Debug)]
pub struct Outcome {
    pub action: Action,
    pub src: PathBuf,
    pub dst: PathBuf,
    pub category: String,
    pub sha256: String,
}

/// Content-hash index over the target folder. Detects duplicates regardless of
/// file names; unchanged files (same size + mtime) keep their cached hash.
pub struct HashIndex {
    cache: HashMap<PathBuf, (u64, Option<SystemTime>, String)>,
    by_hash: HashMap<String, PathBuf>,
}

impl Default for HashIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl HashIndex {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            by_hash: HashMap::new(),
        }
    }

    pub fn refresh(&mut self, target: &Path) {
        let mut entries: Vec<PathBuf> = Vec::new();
        for entry in walkdir::WalkDir::new(target).follow_links(false) {
            let Ok(entry) = entry else { continue };
            if entry.file_type().is_file() {
                entries.push(entry.path().to_path_buf());
            }
        }
        entries.sort();
        let mut fresh_cache = HashMap::new();
        let mut by_hash = HashMap::new();
        for path in entries {
            let meta = match fs::metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let key = (meta.len(), meta.modified().ok());
            let hash = match self.cache.get(&path) {
                Some((len, modified, hash)) if *len == key.0 && *modified == key.1 => hash.clone(),
                _ => match sha256_file(&path) {
                    Ok(h) => h,
                    Err(_) => continue,
                },
            };
            by_hash.entry(hash.clone()).or_insert_with(|| path.clone());
            fresh_cache.insert(path, (key.0, key.1, hash));
        }
        self.cache = fresh_cache;
        self.by_hash = by_hash;
    }

    pub fn lookup(&self, sha256: &str) -> Option<&Path> {
        self.by_hash.get(sha256).map(|p| p.as_path())
    }

    pub fn insert(&mut self, sha256: &str, path: &Path) {
        self.by_hash.entry(sha256.to_string()).or_insert_with(|| path.to_path_buf());
    }
}

pub fn is_partial_download(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        let ext = name
            .rsplit_once('.')
            .map(|(_, e)| e.to_lowercase())
            .unwrap_or_default();
        PARTIAL_DOWNLOAD_EXTS.contains(&ext.as_str())
    })
}

pub fn should_process(config: &Config, path: &Path) -> bool {
    if path.is_dir() || !path.exists() {
        return false;
    }
    if is_hidden(path) || is_under(path, &config.target_dir) {
        return false;
    }
    if path == config.journal_path() {
        return false;
    }
    if is_partial_download(path) {
        return false;
    }
    true
}

pub fn process_file(
    config: &Config,
    dry_run: bool,
    path: &Path,
    index: &mut HashIndex,
) -> Result<Option<Outcome>> {
    if !should_process(config, path) {
        return Ok(None);
    }
    wait_until_stable(path, Duration::from_millis(config.watch.stability_wait_ms));

    let sha256 = sha256_file(path)?;
    let needs_pdf_text = config.rules.iter().any(|r| !r.pdf_keywords.is_empty());
    let pdf_text = if needs_pdf_text {
        classify::pdf_sample_text(path, 3)
    } else {
        None
    };
    let category = classify::classify(config, path, pdf_text.as_deref());
    let dest_dir = config.target_dir.join(&category);
    let m = Messages::get(config.general.language);
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow!("{}", crate::i18n::fmt(m.path_without_name, &[&path.display()])))?
        .to_owned();
    let file_name = PathBuf::from(file_name);

    index.refresh(&config.target_dir);
    let outcome = if let Some(existing) = index.lookup(&sha256).map(|p| p.to_path_buf()) {
        handle_duplicate(config, dry_run, path, &existing, &dest_dir, &file_name, &sha256)?
    } else {
        let existing = dest_dir.join(&file_name);
        let dst = if existing.exists() {
            free_destination(&dest_dir, &file_name)
        } else {
            existing.clone()
        };
        move_file(config, dry_run, path, &dst, Action::Moved, &category, &sha256, index)?
    };
    Ok(Some(outcome))
}

fn handle_duplicate(
    config: &Config,
    dry_run: bool,
    src: &Path,
    existing: &Path,
    dest_dir: &Path,
    file_name: &Path,
    sha256: &str,
) -> Result<Outcome> {
    match config.dedupe.on_duplicate {
        DuplicateAction::Skip => {
            let m = Messages::get(config.general.language);
            log::info!(
                "{}",
                crate::i18n::fmt(
                    m.duplicate_skipped_log,
                    &[&src.display(), &existing.display()]
                )
            );
            notify::maybe_notify(
                &config.notify,
                m.duplicate_skipped_title,
                &crate::i18n::fmt(
                    m.duplicate_already_in,
                    &[&file_name.to_string_lossy(), &existing.display()],
                ),
            );
            if !dry_run {
                let category = dest_dir
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                journal::append(
                    &config.journal_path(),
                    Action::DuplicateSkipped.as_str(),
                    src,
                    existing,
                    &category,
                    sha256,
                )?;
            }
            Ok(Outcome {
                action: Action::DuplicateSkipped,
                src: src.to_path_buf(),
                dst: existing.to_path_buf(),
                category: dest_dir
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
                sha256: sha256.to_string(),
            })
        }
        DuplicateAction::KeepBoth => {
            let dst = free_destination(dest_dir, file_name);
            let category = category_of(dest_dir, config);
            move_file(config, dry_run, src, &dst, Action::KeptBoth, &category, sha256, &mut HashIndex::new())
        }
        DuplicateAction::MoveToDuplicates => {
            let dup_dir = config.target_dir.join(&config.dedupe.duplicates_subdir);
            let dst = free_destination(&dup_dir, file_name);
            move_file(
                config,
                dry_run,
                src,
                &dst,
                Action::MovedToDuplicates,
                &config.dedupe.duplicates_subdir,
                sha256,
                &mut HashIndex::new(),
            )
        }
    }
}

fn category_of(dir: &Path, config: &Config) -> String {
    if dir == config.target_dir {
        return config.fallback_subdir.clone();
    }
    dir.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| config.fallback_subdir.clone())
}

#[allow(clippy::too_many_arguments)]
fn move_file(
    config: &Config,
    dry_run: bool,
    src: &Path,
    dst: &Path,
    action: Action,
    category: &str,
    sha256: &str,
    index: &mut HashIndex,
) -> Result<Outcome> {
    if dry_run {
        log::info!("[dry-run] {} → {}", src.display(), dst.display());
        return Ok(Outcome {
            action,
            src: src.to_path_buf(),
            dst: dst.to_path_buf(),
            category: category.to_string(),
            sha256: sha256.to_string(),
        });
    }
    let m = Messages::get(config.general.language);
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).with_context(|| {
            crate::i18n::fmt(m.dir_not_creatable, &[&parent.display()])
        })?;
    }
    move_raw(src, dst).with_context(|| {
        crate::i18n::fmt(m.move_failed, &[&src.display(), &dst.display()])
    })?;
    log::info!("{} → {}", src.display(), dst.display());
    notify::maybe_notify(
        &config.notify,
        m.file_sorted_title,
        &format!(
            "{} → {}",
            src.file_name().map(|n| n.to_string_lossy()).unwrap_or_default(),
            dst.display()
        ),
    );
    journal::append(&config.journal_path(), action.as_str(), src, dst, category, sha256)?;
    index.insert(sha256, dst);
    Ok(Outcome {
        action,
        src: src.to_path_buf(),
        dst: dst.to_path_buf(),
        category: category.to_string(),
        sha256: sha256.to_string(),
    })
}

fn move_raw(src: &Path, dst: &Path) -> Result<()> {
    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(e)
            if e.kind() == std::io::ErrorKind::CrossesDevices || e.raw_os_error() == Some(18) =>
        {
            fs::copy(src, dst)?;
            fs::remove_file(src)?;
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

pub fn free_destination(dest_dir: &Path, file_name: &Path) -> PathBuf {
    let candidate = dest_dir.join(file_name);
    if !candidate.exists() {
        return candidate;
    }
    let stem = file_name
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let ext = file_name
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    for n in 2.. {
        let candidate = dest_dir.join(format!("{stem} ({n}){ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

fn wait_until_stable(path: &Path, max_wait: Duration) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if let Ok(modified) = meta.modified() {
        if let Ok(age) = modified.elapsed() {
            if age > Duration::from_secs(60) {
                return true;
            }
        }
    }
    let deadline = Instant::now() + max_wait;
    let mut last = Some((meta.len(), meta.modified().ok()));
    loop {
        std::thread::sleep(Duration::from_millis(250));
        let current = fs::metadata(path).ok().map(|m| (m.len(), m.modified().ok()));
        if current.is_some() && current == last {
            return true;
        }
        last = current;
        if Instant::now() >= deadline {
            return current.is_some();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DEFAULT_CONFIG;

    fn config_with_target(dir: &Path) -> Config {
        let mut config: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        config.target_dir = dir.to_path_buf();
        config.watch.dirs = vec![];
        config.notify.enabled = false;
        config
    }

    #[test]
    fn moves_file_into_category_and_journals() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target");
        let config = config_with_target(&target);
        let src = tmp.path().join("Fotourlaub.jpg");
        std::fs::write(&src, b"jpegdata").unwrap();

        let outcome = process_file(&config, false, &src, &mut HashIndex::new())
            .unwrap()
            .unwrap();
        assert_eq!(outcome.action, Action::Moved);
        assert_eq!(outcome.category, "Bilder");
        assert!(target.join("Bilder/Fotourlaub.jpg").exists());
        assert!(!src.exists());
        assert!(config.journal_path().exists());
    }

    #[test]
    fn duplicate_is_skipped_even_under_different_name() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target");
        let config = config_with_target(&target);
        let src1 = tmp.path().join("a.jpg");
        let src2 = tmp.path().join("b.jpg");
        std::fs::write(&src1, b"same").unwrap();
        std::fs::write(&src2, b"same").unwrap();
        let mut index = HashIndex::new();

        assert_eq!(
            process_file(&config, false, &src1, &mut index).unwrap().unwrap().action,
            Action::Moved
        );
        let second = process_file(&config, false, &src2, &mut index).unwrap().unwrap();
        assert_eq!(second.action, Action::DuplicateSkipped);
        assert_eq!(second.dst, target.join("Bilder/a.jpg"));
        assert!(src2.exists(), "Quelle soll bei skip liegen bleiben");
    }

    #[test]
    fn same_name_different_content_is_renamed() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target");
        let config = config_with_target(&target);
        let dir1 = tmp.path().join("a");
        let dir2 = tmp.path().join("b");
        std::fs::create_dir_all(&dir1).unwrap();
        std::fs::create_dir_all(&dir2).unwrap();
        let src1 = dir1.join("report.pdf");
        let src2 = dir2.join("report.pdf");
        std::fs::write(&src1, b"v1").unwrap();
        std::fs::write(&src2, b"v2").unwrap();
        let mut index = HashIndex::new();

        process_file(&config, false, &src1, &mut index).unwrap();
        let outcome = process_file(&config, false, &src2, &mut index).unwrap().unwrap();
        assert_eq!(outcome.action, Action::Moved);
        assert!(target.join("Sonstiges/report (2).pdf").exists());
    }

    #[test]
    fn keep_both_stores_content_duplicate_under_new_name() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target");
        let mut config = config_with_target(&target);
        config.dedupe.on_duplicate = DuplicateAction::KeepBoth;
        let src1 = tmp.path().join("a.jpg");
        let src2 = tmp.path().join("b.jpg");
        std::fs::write(&src1, b"same").unwrap();
        std::fs::write(&src2, b"same").unwrap();
        let mut index = HashIndex::new();

        process_file(&config, false, &src1, &mut index).unwrap();
        let outcome = process_file(&config, false, &src2, &mut index).unwrap().unwrap();
        assert_eq!(outcome.action, Action::KeptBoth);
        assert!(target.join("Bilder/b.jpg").exists());
    }

    #[test]
    fn dry_run_changes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target");
        let config = config_with_target(&target);
        let src = tmp.path().join("note.txt");
        std::fs::write(&src, b"text").unwrap();

        let outcome = process_file(&config, true, &src, &mut HashIndex::new())
            .unwrap()
            .unwrap();
        assert_eq!(outcome.action, Action::Moved);
        assert!(src.exists());
        assert!(!target.exists());
        assert!(!config.journal_path().exists());
    }

    #[test]
    fn files_in_target_are_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target");
        let config = config_with_target(&target);
        let inner = target.join("Bilder/x.jpg");
        fs::create_dir_all(inner.parent().unwrap()).unwrap();
        std::fs::write(&inner, b"x").unwrap();
        assert!(!should_process(&config, &inner));
    }

    #[test]
    fn hidden_files_are_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_with_target(tmp.path());
        assert!(!should_process(&config, &tmp.path().join(".DS_Store")));
    }

    #[test]
    fn partial_downloads_are_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_with_target(tmp.path());
        assert!(!should_process(&config, &tmp.path().join("film.iso.crdownload")));
        assert!(!should_process(&config, &tmp.path().join("bericht.pdf.part")));
        let safari_dir = tmp.path().join("bericht.zip.download");
        std::fs::create_dir_all(&safari_dir).unwrap();
        let inner = safari_dir.join("bericht.zip");
        std::fs::write(&inner, b"partial").unwrap();
        assert!(!should_process(&config, &inner));
    }

    #[test]
    fn free_destination_avoids_collisions() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), b"1").unwrap();
        std::fs::write(dir.path().join("a (2).txt"), b"2").unwrap();
        let next = free_destination(dir.path(), Path::new("a.txt"));
        assert_eq!(next, dir.path().join("a (3).txt"));
    }

    #[test]
    fn hash_index_finds_content_across_names_and_survives_refresh() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target");
        let existing = target.join("Bilder/original.jpg");
        fs::create_dir_all(existing.parent().unwrap()).unwrap();
        std::fs::write(&existing, b"same-bytes").unwrap();

        let mut index = HashIndex::new();
        index.refresh(&target);
        let hash = sha256_file(&existing).unwrap();
        assert_eq!(index.lookup(&hash), Some(existing.as_path()));

        let copy = target.join("Bilder/copy.jpg");
        std::fs::write(&copy, b"same-bytes").unwrap();
        index.refresh(&target);
        let found = index.lookup(&hash).expect("hash must be found after refresh");
        assert!(found == existing.as_path() || found == copy.as_path());
    }
}
