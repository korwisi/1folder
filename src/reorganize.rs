use crate::classify;
use crate::config::{Config, FolderMode};
use crate::hash::sha256_file;
use crate::i18n::Messages;
#[cfg(test)]
use crate::journal;
use crate::notify;
use crate::organize::{free_destination, is_partial_download, Action, HashIndex, Outcome};
use crate::report;
use crate::tags;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

struct Item {
    path: PathBuf,
    current_category: String,
}

/// Re-applies the current classification rules to everything already inside
/// the target folder. Only direct children of each category folder are
/// considered (plus stray files in the target root); nested folder units are
/// re-evaluated as whole units, their contents stay untouched. Moves are
/// journaled as `reclassified` and can be undone with `--undo`.
pub fn run(config: &Config, dry_run: bool, report_path: Option<&Path>) -> Result<()> {
    let m = Messages::get(config.general.language);
    let mut index = HashIndex::with_target(&config.target_dir)?;
    index.refresh(&config.target_dir);

    let items = collect(config);
    let mut count = 0usize;
    for item in items {
        let outcome = if item.path.is_dir() {
            reclassify_folder(config, dry_run, &item)?
        } else {
            reclassify_file(config, dry_run, &item, &mut index)?
        };
        let Some(outcome) = outcome else { continue };
        log::info!("{} → {}", item.path.display(), outcome.dst.display());
        if let Some(report_path) = report_path {
            if let Err(e) = report::append(report_path, &outcome, dry_run) {
                log::warn!(
                    "{}",
                    crate::i18n::fmt(m.report_write_failed, &[&format!("{e:#}")])
                );
            }
        }
        count += 1;
    }

    if !dry_run {
        index.refresh(&config.target_dir);
        remove_empty_category_dirs(config);
    }
    if !dry_run {
        index.flush()?;
    }

    log::info!("{}", crate::i18n::fmt(m.reorganized_done, &[&count]));
    if count > 0 {
        notify::maybe_notify(
            &config.notify,
            m.reorganized_title,
            &crate::i18n::fmt(m.reorganized_done, &[&count]),
        );
    }
    Ok(())
}

fn collect(config: &Config) -> Vec<Item> {
    let mut items = Vec::new();
    for entry in sorted_children(&config.target_dir) {
        if classify::is_hidden(&entry) {
            continue;
        }
        if !entry.is_dir() {
            items.push(Item {
                current_category: String::new(),
                path: entry,
            });
            continue;
        }
        if entry
            .file_name()
            .is_some_and(|n| n.to_string_lossy() == config.dedupe.duplicates_subdir)
        {
            continue;
        }
        let current_category = entry
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        for child in sorted_children(&entry) {
            if classify::is_hidden(&child) {
                continue;
            }
            items.push(Item {
                current_category: current_category.clone(),
                path: child,
            });
        }
    }
    items
}

fn sorted_children(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .map(|entries| entries.filter_map(|e| e.ok()).map(|e| e.path()).collect())
        .unwrap_or_default();
    paths.sort();
    paths
}

fn reclassify_file(
    config: &Config,
    dry_run: bool,
    item: &Item,
    index: &mut HashIndex,
) -> Result<Option<Outcome>> {
    let path = item.path.as_path();
    if is_partial_download(path) {
        return Ok(None);
    }
    let needs_pdf_text = config.rules.iter().any(|r| !r.pdf_keywords.is_empty());
    let pdf_text = if needs_pdf_text {
        classify::pdf_sample_text(path, 3)
    } else {
        None
    };
    let (category, rule_name) = classify::classify_reason(config, path, pdf_text.as_deref());
    if category == item.current_category {
        return Ok(None);
    }

    let sha256 = sha256_file(path)?;
    let dest_dir = config.target_dir.join(&category);
    let file_name: PathBuf = path.file_name().map(PathBuf::from).unwrap_or_default();
    let dst = free_destination(&dest_dir, &file_name);
    let outcome = Outcome {
        action: Action::Reclassified,
        src: path.to_path_buf(),
        dst: dst.clone(),
        category: category.clone(),
        sha256: sha256.clone(),
        rule: rule_name,
    };
    if dry_run {
        log::info!("[dry-run] {} → {}", path.display(), dst.display());
        return Ok(Some(outcome));
    }

    let m = Messages::get(config.general.language);
    fs::create_dir_all(&dest_dir)
        .with_context(|| crate::i18n::fmt(m.dir_not_creatable, &[&dest_dir.display()]))?;
    crate::operation::move_file(config, Action::Reclassified, path, &dst, &category, &sha256)
        .with_context(|| crate::i18n::fmt(m.move_failed, &[&path.display(), &dst.display()]))?;
    apply_tag(config, &dst, &category);
    index.insert(&sha256, &dst);
    Ok(Some(outcome))
}

fn reclassify_folder(config: &Config, dry_run: bool, item: &Item) -> Result<Option<Outcome>> {
    if config.folders.mode != FolderMode::MoveAsUnit {
        return Ok(None);
    }
    let (category, reason) = classify::classify_folder_reason(config, &item.path);
    if category == item.current_category {
        return Ok(None);
    }

    let dest_dir = config.target_dir.join(&category);
    let folder_name: PathBuf = item.path.file_name().map(PathBuf::from).unwrap_or_default();
    let dst = free_destination(&dest_dir, &folder_name);
    let outcome = Outcome {
        action: Action::Reclassified,
        src: item.path.clone(),
        dst: dst.clone(),
        category: category.clone(),
        sha256: String::new(),
        rule: reason,
    };
    if dry_run {
        log::info!("[dry-run] {} → {}", item.path.display(), dst.display());
        return Ok(Some(outcome));
    }

    let m = Messages::get(config.general.language);
    fs::create_dir_all(&dest_dir)
        .with_context(|| crate::i18n::fmt(m.dir_not_creatable, &[&dest_dir.display()]))?;
    crate::operation::move_folder(config, Action::Reclassified, &item.path, &dst, &category)
        .with_context(|| {
            crate::i18n::fmt(m.move_failed, &[&item.path.display(), &dst.display()])
        })?;
    apply_tag(config, &dst, &category);
    Ok(Some(outcome))
}

fn apply_tag(config: &Config, dst: &Path, category: &str) {
    let rule_color = config
        .rules
        .iter()
        .find(|r| r.subdir == category)
        .and_then(|r| r.color);
    tags::apply(
        &config.tags,
        config.general.language,
        dst,
        category,
        rule_color,
    );
}

fn remove_empty_category_dirs(config: &Config) {
    for dir in sorted_children(&config.target_dir) {
        let is_duplicates_dir = dir
            .file_name()
            .is_some_and(|n| n.to_string_lossy() == config.dedupe.duplicates_subdir);
        if !dir.is_dir() || classify::is_hidden(&dir) || is_duplicates_dir {
            continue;
        }
        if fs::read_dir(&dir).is_ok_and(|mut e| e.next().is_none()) {
            if let Err(e) = fs::remove_dir(&dir) {
                log::warn!("Failed to remove empty folder {}: {}", dir.display(), e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Rule;
    use std::path::Path;

    fn config_with_target(dir: &Path) -> Config {
        let mut config: Config = toml::from_str(crate::config::DEFAULT_CONFIG).unwrap();
        config.target_dir = dir.to_path_buf();
        config.watch.dirs = vec![];
        config.notify.enabled = false;
        config
    }

    fn rule_with_ext(name: &str, subdir: &str, ext: &str) -> Rule {
        Rule {
            name: name.to_string(),
            subdir: subdir.to_string(),
            extensions: vec![ext.to_string()],
            filename_keywords: vec![],
            folder_keywords: vec![],
            pdf_keywords: vec![],
            color: None,
        }
    }

    #[test]
    fn reclassifies_file_after_rule_change() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target");
        let mut config = config_with_target(&target);
        let old = target.join("Bilder/foto.jpg");
        fs::create_dir_all(old.parent().unwrap()).unwrap();
        fs::write(&old, b"jpegdata").unwrap();

        config
            .rules
            .insert(0, rule_with_ext("fotos-neu", "FotosNeu", "jpg"));
        run(&config, false, None).unwrap();

        assert!(target.join("FotosNeu/foto.jpg").exists());
        assert!(!old.exists());
        let records = journal::read(&config.journal_path()).unwrap();
        assert!(records
            .iter()
            .any(|r| r.action == "reclassified" && r.dst.ends_with("FotosNeu/foto.jpg")));
    }

    #[test]
    fn file_without_matching_rule_moves_to_fallback() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target");
        let config = config_with_target(&target);
        let old = target.join("Dokumente/mystery.xyz");
        fs::create_dir_all(old.parent().unwrap()).unwrap();
        fs::write(&old, b"data").unwrap();

        run(&config, false, None).unwrap();
        assert!(target.join("Sonstiges/mystery.xyz").exists());
        assert!(!old.exists());
        assert!(
            !target.join("Dokumente").exists(),
            "emptied category is removed"
        );
    }

    #[test]
    fn correctly_placed_files_stay() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target");
        let config = config_with_target(&target);
        let kept = target.join("Bilder/foto.jpg");
        fs::create_dir_all(kept.parent().unwrap()).unwrap();
        fs::write(&kept, b"jpegdata").unwrap();

        run(&config, false, None).unwrap();
        assert!(kept.exists());
        assert!(!config.journal_path().exists());
    }

    #[test]
    fn dry_run_reports_without_moving() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target");
        let mut config = config_with_target(&target);
        let old = target.join("Bilder/foto.jpg");
        fs::create_dir_all(old.parent().unwrap()).unwrap();
        fs::write(&old, b"jpegdata").unwrap();
        config
            .rules
            .insert(0, rule_with_ext("fotos-neu", "FotosNeu", "jpg"));
        let report_path = tmp.path().join("report.jsonl");

        run(&config, true, Some(&report_path)).unwrap();
        assert!(old.exists());
        assert!(!target.join("FotosNeu").exists());
        assert!(!config.journal_path().exists());
        assert!(!report_path.exists());
    }

    #[test]
    fn empty_category_dirs_are_removed() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target");
        let config = config_with_target(&target);
        fs::create_dir_all(target.join("Leer")).unwrap();

        run(&config, false, None).unwrap();
        assert!(!target.join("Leer").exists());
    }

    #[test]
    fn duplicates_subdir_is_left_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target");
        let config = config_with_target(&target);
        let dup = target.join("Duplikate/mystery.xyz");
        fs::create_dir_all(dup.parent().unwrap()).unwrap();
        fs::write(&dup, b"data").unwrap();

        run(&config, false, None).unwrap();
        assert!(dup.exists());
    }

    #[test]
    fn name_collision_gets_suffix_instead_of_overwriting() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target");
        let config = config_with_target(&target);
        let old = target.join("Dokumente/mystery.xyz");
        fs::create_dir_all(old.parent().unwrap()).unwrap();
        fs::write(&old, b"new").unwrap();
        fs::create_dir_all(target.join("Sonstiges")).unwrap();
        fs::write(target.join("Sonstiges/mystery.xyz"), b"existing").unwrap();

        run(&config, false, None).unwrap();
        assert_eq!(
            fs::read(target.join("Sonstiges/mystery.xyz")).unwrap(),
            b"existing"
        );
        assert!(target.join("Sonstiges/mystery (2).xyz").exists());
    }

    #[test]
    fn folder_unit_moves_when_folder_keyword_rule_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target");
        let mut config = config_with_target(&target);
        let old = target.join("Sammlungen/Urlaub_2024");
        fs::create_dir_all(old.join("sub")).unwrap();
        fs::write(old.join("foto.jpg"), b"j").unwrap();
        fs::write(old.join("sub/b.jpg"), b"j").unwrap();

        config.rules.insert(
            0,
            Rule {
                name: "privat".to_string(),
                subdir: "Privat".to_string(),
                extensions: vec![],
                filename_keywords: vec![],
                folder_keywords: vec!["urlaub".to_string()],
                pdf_keywords: vec![],
                color: None,
            },
        );

        run(&config, false, None).unwrap();
        assert!(target.join("Privat/Urlaub_2024/foto.jpg").exists());
        assert!(target.join("Privat/Urlaub_2024/sub/b.jpg").exists());
        assert!(!old.exists());
    }

    #[test]
    fn reclassified_moves_are_undoable() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target");
        let mut config = config_with_target(&target);
        let old = target.join("Bilder/foto.jpg");
        fs::create_dir_all(old.parent().unwrap()).unwrap();
        fs::write(&old, b"jpegdata").unwrap();
        config
            .rules
            .insert(0, rule_with_ext("fotos-neu", "FotosNeu", "jpg"));

        run(&config, false, None).unwrap();
        assert!(target.join("FotosNeu/foto.jpg").exists());

        let selection = crate::undo::UndoSelection {
            all: true,
            count: None,
            within: None,
            src_prefix: None,
        };
        crate::undo::run(&config, false, &selection).unwrap();
        assert!(old.exists());
        assert!(!target.join("FotosNeu/foto.jpg").exists());
    }
}
