use crate::i18n::Language;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const DEFAULT_CONFIG: &str = include_str!("../config.example.toml");

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    pub target_dir: PathBuf,
    pub watch: WatchConfig,
    #[serde(default)]
    pub dedupe: DedupeConfig,
    #[serde(default)]
    pub notify: NotifyConfig,
    #[serde(default)]
    pub tags: TagsConfig,
    #[serde(default)]
    pub folders: FoldersConfig,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default = "default_fallback_subdir")]
    pub fallback_subdir: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct GeneralConfig {
    #[serde(default)]
    pub language: Language,
    #[serde(default)]
    pub preserve_source_paths: PreserveMode,
}

#[derive(Debug, Deserialize, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PreserveMode {
    #[default]
    Off,
    Structure,
    Prefix,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FoldersConfig {
    #[serde(default)]
    pub mode: FolderMode,
    #[serde(default = "default_folders_subdir")]
    pub subdir: String,
    #[serde(default)]
    pub markers: Vec<String>,
}

impl Default for FoldersConfig {
    fn default() -> Self {
        Self {
            mode: FolderMode::default(),
            subdir: default_folders_subdir(),
            markers: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FolderMode {
    Ignore,
    #[default]
    MoveAsUnit,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WatchConfig {
    pub dirs: Vec<PathBuf>,
    #[serde(default = "default_stability_wait_ms")]
    pub stability_wait_ms: u64,
    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DedupeConfig {
    #[serde(default)]
    pub on_duplicate: DuplicateAction,
    #[serde(default = "default_duplicates_subdir")]
    pub duplicates_subdir: String,
}

impl Default for DedupeConfig {
    fn default() -> Self {
        Self {
            on_duplicate: DuplicateAction::default(),
            duplicates_subdir: default_duplicates_subdir(),
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DuplicateAction {
    #[default]
    Skip,
    KeepBoth,
    MoveToDuplicates,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct NotifyConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct TagsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub root_tag: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Rule {
    pub name: String,
    pub subdir: String,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub filename_keywords: Vec<String>,
    #[serde(default)]
    pub folder_keywords: Vec<String>,
    #[serde(default)]
    pub pdf_keywords: Vec<String>,
    #[serde(default)]
    pub color: Option<u8>,
}

fn default_stability_wait_ms() -> u64 {
    5000
}
fn default_debounce_ms() -> u64 {
    1000
}
fn default_duplicates_subdir() -> String {
    "Duplikate".to_string()
}
fn default_folders_subdir() -> String {
    "Sammlungen".to_string()
}
fn default_fallback_subdir() -> String {
    "Sonstiges".to_string()
}
fn default_true() -> bool {
    true
}

pub fn expand_tilde(path: &Path) -> PathBuf {
    let as_str = path.to_string_lossy();
    if as_str == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    if let Some(rest) = as_str.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    path.to_path_buf()
}

impl Config {
    pub fn load(path: &Path) -> Result<Config> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("Config not readable: {}", path.display()))?;
        let mut config: Config =
            toml::from_str(&raw).with_context(|| format!("Config invalid: {}", path.display()))?;
        config.validate_subdirectories()?;
        config.target_dir = expand_tilde(&config.target_dir);
        config.watch.dirs = config.watch.dirs.iter().map(|d| expand_tilde(d)).collect();
        Ok(config)
    }

    pub fn validate_subdirectories(&self) -> Result<()> {
        let mut values = vec![
            ("fallback_subdir", self.fallback_subdir.as_str()),
            ("folders.subdir", self.folders.subdir.as_str()),
            (
                "dedupe.duplicates_subdir",
                self.dedupe.duplicates_subdir.as_str(),
            ),
        ];
        values.extend(
            self.rules
                .iter()
                .map(|rule| (rule.name.as_str(), rule.subdir.as_str())),
        );

        for (field, value) in values {
            if !valid_subdirectory_name(value) {
                anyhow::bail!("Invalid destination subdirectory for {field}: {value:?}");
            }
        }
        Ok(())
    }

    pub fn journal_path(&self) -> PathBuf {
        self.target_dir.join(".1folder-journal.jsonl")
    }
}

fn valid_subdirectory_name(value: &str) -> bool {
    if value.is_empty() || value == "." || value == ".." {
        return false;
    }
    if value.contains('/') || value.contains('\\') {
        return false;
    }
    !Path::new(value).is_absolute()
}

pub fn default_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("1folder").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_config_parses() {
        let config: Config = toml::from_str(DEFAULT_CONFIG).expect("config must parse");
        assert!(!config.watch.dirs.is_empty());
        assert!(!config.rules.is_empty());
        assert_eq!(config.fallback_subdir, "Sonstiges");
        assert_eq!(config.dedupe.on_duplicate, DuplicateAction::Skip);
        assert_eq!(config.folders.mode, FolderMode::MoveAsUnit);
        assert_eq!(config.folders.subdir, "Sammlungen");
    }

    #[test]
    fn folder_mode_defaults_to_move_as_unit() {
        let config: Config =
            toml::from_str("target_dir = '/tmp/t'\n[watch]\ndirs = ['/w']").unwrap();
        assert_eq!(config.folders.mode, FolderMode::MoveAsUnit);
        assert_eq!(config.folders.subdir, "Sammlungen");
    }

    #[test]
    fn tilde_is_expanded() {
        let expanded = expand_tilde(Path::new("~/Downloads/x.pdf"));
        assert!(
            expanded.to_string_lossy().ends_with("Downloads/x.pdf"),
            "unexpected: {expanded:?}"
        );
        assert!(!expanded.to_string_lossy().starts_with('~'));
    }

    #[test]
    fn rejects_absolute_and_traversal_subdirectories() {
        let mut config: Config =
            toml::from_str("target_dir = '/tmp/t'\n[watch]\ndirs = ['/w']").unwrap();

        config.fallback_subdir = "/tmp/escape".to_string();
        assert!(config.validate_subdirectories().is_err());

        config.fallback_subdir = "../escape".to_string();
        assert!(config.validate_subdirectories().is_err());

        config.fallback_subdir = "Sonstiges".to_string();
        config.rules.push(Rule {
            name: "unsafe".to_string(),
            subdir: "nested\\escape".to_string(),
            extensions: vec![],
            filename_keywords: vec![],
            folder_keywords: vec![],
            pdf_keywords: vec![],
            color: None,
        });
        assert!(config.validate_subdirectories().is_err());

        config.rules.clear();
        config.folders.subdir = "../Folders".to_string();
        assert!(config.validate_subdirectories().is_err());

        config.folders.subdir = "Sammlungen".to_string();
        config.dedupe.duplicates_subdir = "/tmp/Duplicates".to_string();
        assert!(config.validate_subdirectories().is_err());
    }
}
