use crate::config::{valid_subdirectory_name, Config};
use crate::organize::Action;
use crate::tags;
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

pub fn rename(
    config: &Config,
    config_path: &Path,
    old: &str,
    new: &str,
    dry_run: bool,
) -> Result<()> {
    if !valid_subdirectory_name(old) {
        bail!("Invalid old category name: {old:?}");
    }
    if !valid_subdirectory_name(new) {
        bail!("Invalid new category name: {new:?}");
    }
    if old == new {
        bail!("Old and new category names must be different");
    }

    let old_dir = config.target_dir.join(old);
    let new_dir = config.target_dir.join(new);
    if !old_dir.is_dir() {
        bail!("Category folder does not exist: {}", old_dir.display());
    }
    if new_dir.exists() {
        bail!("Destination category already exists: {}", new_dir.display());
    }

    if dry_run {
        println!("[dry-run] {} → {}", old_dir.display(), new_dir.display());
        println!(
            "[dry-run] update category references in {}",
            config_path.display()
        );
        return Ok(());
    }

    let raw = fs::read_to_string(config_path)
        .with_context(|| format!("Config not readable: {}", config_path.display()))?;
    let updated = rename_category_references(&raw, old, new);
    if updated == raw {
        bail!(
            "No category references for {old:?} were found in {}",
            config_path.display()
        );
    }

    write_config_atomically(config_path, &updated)?;

    let move_result =
        crate::operation::move_folder(config, Action::CategoryRenamed, &old_dir, &new_dir, new);
    if let Err(error) = move_result {
        if old_dir.is_dir() && !new_dir.exists() {
            let _ = write_config_atomically(config_path, &raw);
        }
        return Err(error);
    }

    let color = config
        .rules
        .iter()
        .find(|rule| rule.subdir == old)
        .and_then(|rule| rule.color);
    retag_tree(&new_dir, config, new, color)?;
    println!(
        "Renamed category {} → {}",
        old_dir.display(),
        new_dir.display()
    );
    println!("Updated category references in {}", config_path.display());
    println!("Restart the background agent to load the updated category configuration.");
    Ok(())
}

fn rename_category_references(raw: &str, old: &str, new: &str) -> String {
    raw.lines()
        .map(|line| {
            let Some((key, rhs)) = line.split_once('=') else {
                return line.to_string();
            };
            let key = key.trim();
            if !matches!(key, "subdir" | "fallback_subdir" | "duplicates_subdir") {
                return line.to_string();
            }
            let rhs = rhs.trim();
            let quoted = format!("\"{old}\"");
            if let Some(comment) = rhs.strip_prefix(&quoted) {
                let separator = if comment.trim_start().starts_with('#') {
                    comment.to_string()
                } else {
                    String::new()
                };
                let indent = line.len() - line.trim_start().len();
                let indent = " ".repeat(indent);
                return format!("{indent}{key} = \"{new}\"{separator}");
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
        + if raw.ends_with('\n') { "\n" } else { "" }
}

fn write_config_atomically(path: &Path, contents: &str) -> Result<()> {
    let temp = path.with_extension("toml.tmp");
    fs::write(&temp, contents)
        .with_context(|| format!("Config not writable: {}", temp.display()))?;
    fs::rename(&temp, path)
        .with_context(|| format!("Config could not be replaced: {}", path.display()))
}

fn retag_tree(path: &Path, config: &Config, category: &str, color: Option<u8>) -> Result<()> {
    for entry in walkdir::WalkDir::new(path).follow_links(false) {
        let Ok(entry) = entry else { continue };
        if entry.file_type().is_file() {
            tags::apply(
                &config.tags,
                config.general.language,
                entry.path(),
                category,
                color,
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn rename_category_references_preserve_comments() {
        let raw =
            "fallback_subdir = \"Old\" # fallback\nsubdir = \"Old\" # folder\nother = \"Old\"\n";
        let updated = rename_category_references(raw, "Old", "New");
        assert_eq!(
            updated,
            "fallback_subdir = \"New\" # fallback\nsubdir = \"New\" # folder\nother = \"Old\"\n"
        );
    }

    #[test]
    fn rename_category_references_requires_exact_name() {
        let raw = "subdir = \"OldArchive\"\n";
        assert_eq!(rename_category_references(raw, "Old", "New"), raw);
    }

    #[test]
    fn rename_moves_folder_and_updates_config() {
        let tmp = tempfile::tempdir().unwrap();
        let mut config: Config = toml::from_str(crate::config::DEFAULT_CONFIG).unwrap();
        config.target_dir = tmp.path().join("target");
        config.watch.dirs = vec![];
        config.notify.enabled = false;
        let config_path = tmp.path().join("config.toml");
        let raw = "target_dir = \"target\"\nfallback_subdir = \"Old\"\nsubdir = \"Old\"\n";
        fs::write(&config_path, raw).unwrap();
        fs::create_dir_all(config.target_dir.join("Old")).unwrap();
        fs::write(config.target_dir.join("Old/file.txt"), b"text").unwrap();

        rename(&config, &config_path, "Old", "New", false).unwrap();

        assert!(config.target_dir.join("New/file.txt").exists());
        assert!(!config.target_dir.join("Old").exists());
        assert!(fs::read_to_string(&config_path)
            .unwrap()
            .contains("subdir = \"New\""));
        assert!(fs::read_to_string(&config_path)
            .unwrap()
            .contains("fallback_subdir = \"New\""));
        assert!(crate::journal::read(&config.journal_path())
            .unwrap()
            .iter()
            .any(|record| record.action == "category_renamed"));
    }
}
