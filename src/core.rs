use crate::config::{Config, FolderMode};
use crate::organize::{self, Action, HashIndex, Outcome};
use crate::report;
use crate::undo::{self, UndoSelection};
use anyhow::{Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewOperation {
    pub action: String,
    pub src: PathBuf,
    pub dst: PathBuf,
    pub category: String,
    pub sha256: String,
    pub rule: Option<String>,
}

impl From<Outcome> for PreviewOperation {
    fn from(outcome: Outcome) -> Self {
        Self {
            action: action_name(outcome.action).to_string(),
            src: outcome.src,
            dst: outcome.dst,
            category: outcome.category,
            sha256: outcome.sha256,
            rule: outcome.rule,
        }
    }
}

fn action_name(action: Action) -> &'static str {
    match action {
        Action::Moved => "moved",
        Action::MovedFolder => "moved_folder",
        Action::DuplicateSkipped => "duplicate_skipped",
        Action::KeptBoth => "kept_both",
        Action::MovedToDuplicates => "moved_to_duplicates",
        Action::Reclassified => "reclassified",
        Action::CategoryRenamed => "category_renamed",
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewIssue {
    pub path: Option<PathBuf>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewSummary {
    pub operations: Vec<PreviewOperation>,
    pub issues: Vec<PreviewIssue>,
    pub items_checked: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ActivityRecord {
    pub id: Option<String>,
    pub timestamp: String,
    pub action: String,
    pub src: PathBuf,
    pub dst: PathBuf,
    pub category: Option<String>,
    pub sha256: Option<String>,
    pub undo_of: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ActivitySummary {
    pub records: Vec<ActivityRecord>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CoreStatus {
    pub target_dir: PathBuf,
    pub target_exists: bool,
    pub watched_dirs: Vec<PathBuf>,
    pub missing_watched_dirs: Vec<PathBuf>,
    pub pending_operation: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RunSummary {
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UndoSummary {
    pub restored: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TargetIconSummary {
    pub target_dir: PathBuf,
}

pub fn load_config(path: &Path) -> Result<Config> {
    Config::load(path)
}

pub fn status(config: &Config) -> Result<CoreStatus> {
    let missing_watched_dirs = config
        .watch
        .dirs
        .iter()
        .filter(|dir| !dir.is_dir())
        .cloned()
        .collect();
    let pending_operation = config.target_dir.join(".1folder-operation.json").is_file();
    Ok(CoreStatus {
        target_dir: config.target_dir.clone(),
        target_exists: config.target_dir.is_dir(),
        watched_dirs: config.watch.dirs.clone(),
        missing_watched_dirs,
        pending_operation,
    })
}

pub fn run_once(config: &Config, dry_run: bool, report_path: Option<&Path>) -> Result<RunSummary> {
    crate::watch::run_once(config, dry_run, report_path)?;
    Ok(RunSummary { dry_run })
}

pub fn reconcile(config: &Config) -> Result<()> {
    crate::operation::reconcile(config)
}

pub fn set_target_folder_icon(config: &Config) -> Result<TargetIconSummary> {
    crate::icon::set_target_folder_icon(&config.target_dir)?;
    Ok(TargetIconSummary {
        target_dir: config.target_dir.clone(),
    })
}

pub fn save_config(path: &Path, raw: &str) -> Result<()> {
    crate::config::Config::save_raw(path, raw)
}

pub fn history(config: &Config) -> Result<ActivitySummary> {
    let records = crate::journal::read(&config.journal_path()).with_context(|| {
        format!(
            "Activity history could not be read: {}",
            config.journal_path().display()
        )
    })?;
    Ok(ActivitySummary {
        records: records
            .into_iter()
            .map(|record| ActivityRecord {
                id: record.id,
                timestamp: record.ts,
                action: record.action,
                src: PathBuf::from(record.src),
                dst: PathBuf::from(record.dst),
                category: record.category,
                sha256: record.sha256,
                undo_of: record.undo_of,
            })
            .collect(),
    })
}

pub fn undo(config: &Config, dry_run: bool, selection: &UndoSelection) -> Result<UndoSummary> {
    Ok(UndoSummary {
        restored: undo::run(config, dry_run, selection)?,
    })
}

pub fn reorganize(config: &Config, dry_run: bool, report_path: Option<&Path>) -> Result<()> {
    crate::reorganize::run(config, dry_run, report_path)
}

pub fn rename_category(
    config: &Config,
    config_path: &Path,
    old: &str,
    new: &str,
    dry_run: bool,
) -> Result<()> {
    crate::category::rename(config, config_path, old, new, dry_run)
}

pub fn preview(config: &Config, report_path: Option<&Path>) -> Result<PreviewSummary> {
    let mut folders = Vec::new();
    let mut files = Vec::new();
    let mut issues = Vec::new();

    for dir in &config.watch.dirs {
        if !dir.is_dir() {
            issues.push(PreviewIssue {
                path: Some(dir.clone()),
                message: "Watched directory is missing".to_string(),
            });
            continue;
        }
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(error) => {
                issues.push(PreviewIssue {
                    path: Some(dir.clone()),
                    message: format!("Watched directory could not be read: {error}"),
                });
                continue;
            }
        };
        let mut paths = Vec::new();
        for entry in entries {
            match entry {
                Ok(entry) => paths.push(entry.path()),
                Err(error) => issues.push(PreviewIssue {
                    path: Some(dir.clone()),
                    message: format!("Watched directory entry could not be read: {error}"),
                }),
            }
        }
        paths.sort();
        for path in paths {
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

    let mut operations = Vec::new();
    let items_checked = folders.len() + files.len();
    let mut index = HashIndex::with_target(&config.target_dir)?;
    for dir in folders {
        match organize::process_folder(config, true, &dir) {
            Ok(Some(outcome)) => {
                if let Some(report_path) = report_path {
                    report::append(report_path, &outcome, true)?;
                }
                operations.push(outcome.into());
            }
            Ok(None) => {}
            Err(error) => issues.push(PreviewIssue {
                path: Some(dir),
                message: format!("{error:#}"),
            }),
        }
    }
    for path in files {
        match organize::process_file(config, true, &path, &mut index) {
            Ok(Some(outcome)) => {
                if let Some(report_path) = report_path {
                    report::append(report_path, &outcome, true)?;
                }
                operations.push(outcome.into());
            }
            Ok(None) => {}
            Err(error) => issues.push(PreviewIssue {
                path: Some(path),
                message: format!("{error:#}"),
            }),
        }
    }

    Ok(PreviewSummary {
        operations,
        issues,
        items_checked,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn preview_config(tmp: &tempfile::TempDir) -> (Config, PathBuf) {
        let watched = tmp.path().join("watched");
        let target = tmp.path().join("target");
        fs::create_dir_all(&watched).unwrap();
        let config: Config = toml::from_str(&format!(
            r#"target_dir = "{}"

[watch]
dirs = ["{}"]
stability_wait_ms = 100
debounce_ms = 100

[[rules]]
name = "images"
subdir = "Images"
extensions = ["jpg"]
"#,
            target.display(),
            watched.display()
        ))
        .unwrap();
        (config, watched)
    }

    #[test]
    fn preview_reports_operations_without_side_effects() {
        let tmp = tempfile::tempdir().unwrap();
        let (config, watched) = preview_config(&tmp);
        fs::write(watched.join("photo.jpg"), b"jpeg").unwrap();
        let report_path = tmp.path().join("preview.jsonl");

        let summary = preview(&config, Some(&report_path)).unwrap();

        assert_eq!(summary.operations.len(), 1);
        assert_eq!(summary.operations[0].action, "moved");
        assert_eq!(summary.operations[0].category, "Images");
        assert!(summary.issues.is_empty());
        assert!(watched.join("photo.jpg").exists());
        assert!(!config.target_dir.exists());
        assert!(!report_path.exists());
    }

    #[test]
    fn preview_reports_missing_watched_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let (config, watched) = preview_config(&tmp);
        fs::remove_dir(&watched).unwrap();

        let summary = preview(&config, None).unwrap();

        assert_eq!(summary.operations.len(), 0);
        assert_eq!(summary.issues.len(), 1);
        assert!(summary.issues[0].message.contains("missing"));
    }

    #[test]
    fn status_reports_missing_watched_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let (config, watched) = preview_config(&tmp);
        fs::remove_dir(&watched).unwrap();

        let status = status(&config).unwrap();

        assert_eq!(status.missing_watched_dirs, vec![watched]);
        assert!(!status.pending_operation);
    }

    #[test]
    fn history_returns_ui_neutral_journal_records() {
        let tmp = tempfile::tempdir().unwrap();
        let (config, _) = preview_config(&tmp);
        fs::create_dir_all(&config.target_dir).unwrap();
        fs::write(
            config.journal_path(),
            format!(
                "{}\n",
                serde_json::json!({
                    "ts": "2026-11-27T15:00:00+01:00",
                    "action": "moved",
                    "src": "/tmp/photo.jpg",
                    "dst": "/tmp/target/Images/photo.jpg",
                    "category": "Images",
                    "sha256": "abc",
                    "id": "record-id"
                })
            ),
        )
        .unwrap();

        let history = history(&config).unwrap();

        assert_eq!(history.records.len(), 1);
        assert_eq!(history.records[0].action, "moved");
        assert_eq!(history.records[0].category.as_deref(), Some("Images"));
        assert_eq!(history.records[0].id.as_deref(), Some("record-id"));
    }
}
