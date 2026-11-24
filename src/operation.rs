use crate::config::Config;
use crate::journal;
use crate::organize::{move_dir_raw, move_raw, Action};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum OperationKind {
    Move,
    Undo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingOperation {
    kind: OperationKind,
    phase: String,
    action: String,
    src: PathBuf,
    dst: PathBuf,
    category: String,
    sha256: String,
    undo_of: Option<String>,
}

#[cfg(test)]
std::thread_local! {
    static TEST_REMOVE_FAULT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
fn set_remove_fault() {
    TEST_REMOVE_FAULT.with(|state| state.set(true));
}

#[cfg(test)]
fn remove_recovered_file(path: &Path) -> Result<()> {
    let should_fail = TEST_REMOVE_FAULT.with(|state| state.get());
    if should_fail {
        TEST_REMOVE_FAULT.with(|state| state.set(false));
        anyhow::bail!("Injected source removal failure");
    }
    if path.is_dir() {
        std::fs::remove_dir_all(path).context("Recovered source could not be removed")
    } else {
        std::fs::remove_file(path).context("Recovered source could not be removed")
    }
}

#[cfg(not(test))]
fn remove_recovered_file(path: &Path) -> Result<()> {
    if path.is_dir() {
        std::fs::remove_dir_all(path).context("Recovered source could not be removed")
    } else {
        std::fs::remove_file(path).context("Recovered source could not be removed")
    }
}

impl PendingOperation {
    fn state_path(config: &Config) -> PathBuf {
        config.target_dir.join(".1folder-operation.json")
    }

    fn write(&self, config: &Config) -> Result<()> {
        std::fs::create_dir_all(&config.target_dir)
            .context("Operation state directory could not be created")?;
        let path = Self::state_path(config);
        let temp = path.with_extension("json.tmp");
        std::fs::write(
            &temp,
            serde_json::to_vec_pretty(self).context("Operation state could not be serialized")?,
        )
        .context("Operation state could not be written")?;
        std::fs::rename(&temp, &path).context("Operation state could not be published")?;
        Ok(())
    }

    fn read(config: &Config) -> Result<Option<Self>> {
        let path = Self::state_path(config);
        match std::fs::read(&path) {
            Ok(raw) => Ok(Some(
                serde_json::from_slice(&raw).context("Operation state is malformed")?,
            )),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error).context("Operation state could not be read"),
        }
    }

    fn remove(config: &Config) -> Result<()> {
        match std::fs::remove_file(Self::state_path(config)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).context("Operation state could not be removed"),
        }
    }
}

pub fn reconcile(config: &Config) -> Result<()> {
    let Some(mut operation) = PendingOperation::read(config)? else {
        return Ok(());
    };
    if operation.phase == "pending" {
        match operation.kind {
            OperationKind::Move => {
                if operation.dst.exists() && operation.src.exists() {
                    crate::organize::verify_move_pair(&operation.src, &operation.dst)?;
                    remove_recovered_file(&operation.src)
                        .context("Recovered move source could not be removed")?;
                    operation.phase = "filesystem_done".to_string();
                    operation.write(config)?;
                } else if operation.dst.exists() && !operation.src.exists() {
                    operation.phase = "filesystem_done".to_string();
                    operation.write(config)?;
                } else {
                    log::warn!(
                        "1folder found an interrupted pending operation; no filesystem change was detected: {} → {}",
                        operation.src.display(),
                        operation.dst.display()
                    );
                    PendingOperation::remove(config)?;
                    return Ok(());
                }
            }
            OperationKind::Undo => {
                if operation.src.exists() && operation.dst.exists() {
                    crate::organize::verify_move_pair(&operation.dst, &operation.src)?;
                    remove_recovered_file(&operation.dst)
                        .context("Recovered undo source could not be removed")?;
                    operation.phase = "filesystem_done".to_string();
                    operation.write(config)?;
                } else if operation.src.exists() && !operation.dst.exists() {
                    operation.phase = "filesystem_done".to_string();
                    operation.write(config)?;
                } else {
                    log::warn!(
                        "1folder found an interrupted pending undo; no filesystem change was detected: {} → {}",
                        operation.dst.display(),
                        operation.src.display()
                    );
                    PendingOperation::remove(config)?;
                    return Ok(());
                }
            }
        }
    } else if operation.phase != "filesystem_done" {
        anyhow::bail!("Unknown 1folder operation phase: {}", operation.phase);
    }
    match operation.kind {
        OperationKind::Move => {
            if operation.dst.exists() && !operation.src.exists() {
                journal::append(
                    &config.journal_path(),
                    &operation.action,
                    &operation.src,
                    &operation.dst,
                    &operation.category,
                    &operation.sha256,
                )?;
            }
        }
        OperationKind::Undo => {
            if operation.src.exists() && !operation.dst.exists() {
                journal::append_undo(
                    &config.journal_path(),
                    operation.undo_of.as_deref(),
                    &operation.dst,
                    &operation.src,
                )?;
            }
        }
    }
    operation.phase = "journaled".to_string();
    operation.write(config)?;
    PendingOperation::remove(config)?;
    Ok(())
}

pub fn move_file(
    config: &Config,
    action: Action,
    src: &Path,
    dst: &Path,
    category: &str,
    sha256: &str,
) -> Result<()> {
    let mut operation = PendingOperation {
        kind: OperationKind::Move,
        phase: "pending".to_string(),
        action: action.as_str().to_string(),
        src: src.to_path_buf(),
        dst: dst.to_path_buf(),
        category: category.to_string(),
        sha256: sha256.to_string(),
        undo_of: None,
    };
    operation.write(config)?;
    move_raw(src, dst)?;
    operation.phase = "filesystem_done".to_string();
    operation.write(config)?;
    journal::append(
        &config.journal_path(),
        action.as_str(),
        src,
        dst,
        category,
        sha256,
    )?;
    operation.phase = "journaled".to_string();
    operation.write(config)?;
    PendingOperation::remove(config)
}

pub fn move_folder(
    config: &Config,
    action: Action,
    src: &Path,
    dst: &Path,
    category: &str,
) -> Result<()> {
    let mut operation = PendingOperation {
        kind: OperationKind::Move,
        phase: "pending".to_string(),
        action: action.as_str().to_string(),
        src: src.to_path_buf(),
        dst: dst.to_path_buf(),
        category: category.to_string(),
        sha256: String::new(),
        undo_of: None,
    };
    operation.write(config)?;
    move_dir_raw(src, dst)?;
    operation.phase = "filesystem_done".to_string();
    operation.write(config)?;
    journal::append(
        &config.journal_path(),
        action.as_str(),
        src,
        dst,
        category,
        "",
    )?;
    operation.phase = "journaled".to_string();
    operation.write(config)?;
    PendingOperation::remove(config)
}

pub fn undo(config: &Config, undo_of: Option<&str>, dst: &Path, src: &Path) -> Result<()> {
    let mut operation = PendingOperation {
        kind: OperationKind::Undo,
        phase: "pending".to_string(),
        action: "undone".to_string(),
        src: src.to_path_buf(),
        dst: dst.to_path_buf(),
        category: String::new(),
        sha256: String::new(),
        undo_of: undo_of.map(str::to_string),
    };
    operation.write(config)?;
    move_raw(dst, src)?;
    operation.phase = "filesystem_done".to_string();
    operation.write(config)?;
    journal::append_undo(&config.journal_path(), undo_of, dst, src)?;
    PendingOperation::remove(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(dir: &Path) -> Config {
        let mut config: Config = toml::from_str(crate::config::DEFAULT_CONFIG).unwrap();
        config.target_dir = dir.join("target");
        config.watch.dirs = vec![dir.join("Desktop")];
        config.notify.enabled = false;
        config
    }

    #[test]
    fn reconcile_journals_completed_move() {
        let tmp = tempfile::tempdir().unwrap();
        let config = test_config(tmp.path());
        let src = config.watch.dirs[0].join("a.jpg");
        let dst = config.target_dir.join("Bilder/a.jpg");
        std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
        std::fs::write(&dst, b"image").unwrap();

        let operation = PendingOperation {
            kind: OperationKind::Move,
            phase: "filesystem_done".to_string(),
            action: "moved".to_string(),
            src: src.clone(),
            dst: dst.clone(),
            category: "Bilder".to_string(),
            sha256: "abc".to_string(),
            undo_of: None,
        };
        operation.write(&config).unwrap();

        reconcile(&config).unwrap();

        let records = journal::read(&config.journal_path()).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].action, "moved");
        assert!(!PendingOperation::state_path(&config).exists());
    }

    #[test]
    fn reconcile_journals_completed_undo() {
        let tmp = tempfile::tempdir().unwrap();
        let config = test_config(tmp.path());
        let dst = config.target_dir.join("Bilder/a.jpg");
        let src = config.watch.dirs[0].join("a.jpg");
        std::fs::create_dir_all(src.parent().unwrap()).unwrap();
        std::fs::write(&src, b"image").unwrap();

        let operation = PendingOperation {
            kind: OperationKind::Undo,
            phase: "filesystem_done".to_string(),
            action: "undone".to_string(),
            src: src.clone(),
            dst: dst.clone(),
            category: String::new(),
            sha256: String::new(),
            undo_of: Some("legacy-id".to_string()),
        };
        operation.write(&config).unwrap();

        reconcile(&config).unwrap();

        let records = journal::read(&config.journal_path()).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].action, "undone");
        assert_eq!(records[0].undo_of.as_deref(), Some("legacy-id"));
        assert!(!PendingOperation::state_path(&config).exists());
    }

    #[test]
    fn reconcile_removes_copied_source_after_verification() {
        let tmp = tempfile::tempdir().unwrap();
        let config = test_config(tmp.path());
        let src = config.watch.dirs[0].join("a.jpg");
        let dst = config.target_dir.join("Bilder/a.jpg");
        std::fs::create_dir_all(src.parent().unwrap()).unwrap();
        std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
        std::fs::write(&src, b"image").unwrap();
        std::fs::write(&dst, b"image").unwrap();

        let operation = PendingOperation {
            kind: OperationKind::Move,
            phase: "pending".to_string(),
            action: "moved".to_string(),
            src: src.clone(),
            dst: dst.clone(),
            category: "Bilder".to_string(),
            sha256: "abc".to_string(),
            undo_of: None,
        };
        operation.write(&config).unwrap();

        reconcile(&config).unwrap();

        assert!(!src.exists());
        assert!(dst.exists());
        assert!(journal::read(&config.journal_path()).unwrap().len() == 1);
    }

    #[test]
    fn move_keeps_pending_state_when_journal_append_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let config = test_config(tmp.path());
        let src = config.watch.dirs[0].join("a.jpg");
        let dst = config.target_dir.join("Bilder/a.jpg");
        std::fs::create_dir_all(src.parent().unwrap()).unwrap();
        std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
        std::fs::write(&src, b"image").unwrap();

        journal::set_test_fault(journal::TestJournalFault::Append);
        let error = move_file(&config, Action::Moved, &src, &dst, "Bilder", "abc").unwrap_err();

        assert!(error
            .to_string()
            .contains("Injected journal append failure"));
        assert!(!src.exists());
        assert!(dst.exists());
        assert!(PendingOperation::state_path(&config).exists());

        reconcile(&config).unwrap();
        assert_eq!(journal::read(&config.journal_path()).unwrap().len(), 1);
    }

    #[test]
    fn undo_keeps_pending_state_when_journal_append_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let config = test_config(tmp.path());
        let dst = config.target_dir.join("Bilder/a.jpg");
        let src = config.watch.dirs[0].join("a.jpg");
        std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
        std::fs::create_dir_all(src.parent().unwrap()).unwrap();
        std::fs::write(&dst, b"image").unwrap();

        journal::set_test_fault(journal::TestJournalFault::Append);
        let error = undo(&config, Some("move-id"), &dst, &src).unwrap_err();

        assert!(error
            .to_string()
            .contains("Injected journal append failure"));
        assert!(!dst.exists());
        assert!(src.exists());
        assert!(PendingOperation::state_path(&config).exists());

        reconcile(&config).unwrap();
        let records = journal::read(&config.journal_path()).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].undo_of.as_deref(), Some("move-id"));
    }

    #[test]
    fn reconciliation_retries_source_removal_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let config = test_config(tmp.path());
        let src = config.watch.dirs[0].join("a.jpg");
        let dst = config.target_dir.join("Bilder/a.jpg");
        std::fs::create_dir_all(src.parent().unwrap()).unwrap();
        std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
        std::fs::write(&src, b"image").unwrap();
        std::fs::write(&dst, b"image").unwrap();

        let operation = PendingOperation {
            kind: OperationKind::Move,
            phase: "pending".to_string(),
            action: "moved".to_string(),
            src: src.clone(),
            dst: dst.clone(),
            category: "Bilder".to_string(),
            sha256: "abc".to_string(),
            undo_of: None,
        };
        operation.write(&config).unwrap();
        set_remove_fault();

        let error = reconcile(&config).unwrap_err();
        assert!(error
            .to_string()
            .contains("Recovered move source could not be removed"));
        assert!(src.exists());
        assert!(PendingOperation::state_path(&config).exists());

        reconcile(&config).unwrap();
        assert!(!src.exists());
        assert!(dst.exists());
    }
}
