use crate::config::Config;
use crate::i18n::Messages;
use crate::journal::{self, JournalRecord};
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const UNDOABLE: &[&str] = &[
    "moved",
    "moved_folder",
    "kept_both",
    "moved_to_duplicates",
    "reclassified",
    "category_renamed",
];

pub struct UndoSelection {
    pub all: bool,
    pub count: Option<usize>,
    pub within: Option<Duration>,
    pub src_prefix: Option<PathBuf>,
}

pub fn parse_duration(raw: &str) -> Option<Duration> {
    let raw = raw.trim();
    let (digits, unit) = raw.split_at(raw.len().checked_sub(1)?);
    let multiplier = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        "d" => 86400,
        _ => return None,
    };
    let secs: u64 = digits.parse().ok()?;
    Some(Duration::from_secs(secs * multiplier))
}

pub fn run(config: &Config, dry_run: bool, selection: &UndoSelection) -> Result<()> {
    let m = Messages::get(config.general.language);
    let records = match journal::read(&config.journal_path()) {
        Ok(records) => records,
        Err(journal::JournalReadError::Malformed { line, reason }) => {
            anyhow::bail!(
                "{}",
                crate::i18n::fmt(m.journal_malformed, &[&line, &reason])
            );
        }
        Err(journal::JournalReadError::Truncated { line, reason }) => {
            anyhow::bail!(
                "{}",
                crate::i18n::fmt(m.journal_truncated, &[&line, &reason])
            );
        }
        Err(error) => return Err(error.into()),
    };
    let undone: HashSet<String> = records.iter().filter_map(|r| r.undo_of.clone()).collect();
    let mut candidates: Vec<&JournalRecord> = records
        .iter()
        .filter(|r| UNDOABLE.contains(&r.action.as_str()))
        .filter(|r| r.id.as_ref().is_none_or(|id| !undone.contains(id)))
        .filter(|r| within_selection(r, selection.within))
        .filter(|r| under_prefix(r, selection.src_prefix.as_deref()))
        .collect();
    candidates.reverse();
    if !selection.all {
        candidates.truncate(selection.count.unwrap_or(1));
    }
    if candidates.is_empty() {
        log::info!("{}", m.undo_nothing);
        return Ok(());
    }
    let mut restored = 0usize;
    for record in &candidates {
        if restore(config, dry_run, record)? {
            restored += 1;
        }
    }
    log::info!("{}", crate::i18n::fmt(m.undo_done, &[&restored]));
    Ok(())
}

fn within_selection(record: &JournalRecord, within: Option<Duration>) -> bool {
    let Some(within) = within else {
        return true;
    };
    let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&record.ts) else {
        return false;
    };
    let Ok(age) = SystemTime::now().duration_since(ts.into()) else {
        return false;
    };
    age <= within
}

fn under_prefix(record: &JournalRecord, prefix: Option<&Path>) -> bool {
    match prefix {
        None => true,
        Some(prefix) => Path::new(&record.src).starts_with(prefix),
    }
}

fn restore(config: &Config, dry_run: bool, record: &JournalRecord) -> Result<bool> {
    let m = Messages::get(config.general.language);
    let dst = PathBuf::from(&record.dst);
    let src = PathBuf::from(&record.src);
    if !dst.exists() {
        log::warn!(
            "{}",
            crate::i18n::fmt(m.undo_dst_missing, &[&dst.display()])
        );
        return Ok(false);
    }
    if dry_run {
        log::info!("[dry-run] {} → {}", dst.display(), src.display());
        return Ok(true);
    }
    let parent = src.parent().unwrap_or(&src).to_path_buf();
    std::fs::create_dir_all(&parent)
        .with_context(|| crate::i18n::fmt(m.dir_not_creatable, &[&parent.display()]))?;
    let target = if src.exists() {
        let name = src.file_name().map(Path::new).unwrap_or(src.as_path());
        crate::organize::free_destination(&parent, name)
    } else {
        src.clone()
    };
    crate::operation::undo(config, record.id.as_deref(), &dst, &target)
        .with_context(|| crate::i18n::fmt(m.move_failed, &[&dst.display(), &target.display()]))?;
    log::info!(
        "{}",
        crate::i18n::fmt(m.undo_restored, &[&dst.display(), &target.display()])
    );
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::organize::{process_file, process_folder, HashIndex};
    use std::fs;

    fn config_with_target(dir: &Path) -> Config {
        let mut config: Config = toml::from_str(crate::config::DEFAULT_CONFIG).unwrap();
        config.target_dir = dir.join("target");
        config.watch.dirs = vec![dir.join("Desktop")];
        config.notify.enabled = false;
        config
    }

    fn move_sample(config: &Config, name: &str, content: &[u8]) {
        let src = config.watch.dirs[0].join(name);
        fs::create_dir_all(src.parent().unwrap()).unwrap();
        fs::write(&src, content).unwrap();
        process_file(config, false, &src, &mut HashIndex::new()).unwrap();
    }

    fn selection(all: bool, count: Option<usize>) -> UndoSelection {
        UndoSelection {
            all,
            count,
            within: None,
            src_prefix: None,
        }
    }

    #[test]
    fn parse_duration_accepts_units() {
        assert_eq!(parse_duration("30s"), Some(Duration::from_secs(30)));
        assert_eq!(parse_duration("10m"), Some(Duration::from_secs(600)));
        assert_eq!(parse_duration("2h"), Some(Duration::from_secs(7200)));
        assert_eq!(parse_duration("1d"), Some(Duration::from_secs(86400)));
        assert_eq!(parse_duration("x"), None);
        assert_eq!(parse_duration("5x"), None);
    }

    #[test]
    fn undo_restores_last_move_and_protects_against_double_undo() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_with_target(tmp.path());
        move_sample(&config, "a.jpg", b"j");

        run(&config, false, &selection(true, None)).unwrap();
        assert!(config.watch.dirs[0].join("a.jpg").exists());
        assert!(!config.target_dir.join("Bilder/a.jpg").exists());

        run(&config, false, &selection(true, None)).unwrap();
        assert!(
            config.watch.dirs[0].join("a.jpg").exists(),
            "second undo must not move the file again"
        );
    }

    #[test]
    fn undo_limit_restores_only_last_n() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_with_target(tmp.path());
        move_sample(&config, "a.jpg", b"j");
        move_sample(&config, "b.txt", b"t");

        run(&config, false, &selection(false, Some(1))).unwrap();
        assert!(config.watch.dirs[0].join("b.txt").exists());
        assert!(!config.watch.dirs[0].join("a.jpg").exists());
        assert!(config.target_dir.join("Bilder/a.jpg").exists());
    }

    #[test]
    fn undo_src_prefix_filters_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_with_target(tmp.path());
        move_sample(&config, "alter-desktop/a.jpg", b"j");
        move_sample(&config, "anderes/b.txt", b"t");

        let sel = UndoSelection {
            all: true,
            count: None,
            within: None,
            src_prefix: Some(config.watch.dirs[0].join("alter-desktop")),
        };
        run(&config, false, &sel).unwrap();
        assert!(config.watch.dirs[0].join("alter-desktop/a.jpg").exists());
        assert!(!config.watch.dirs[0].join("anderes/b.txt").exists());
        assert!(config.target_dir.join("Dokumente/b.txt").exists());
    }

    #[test]
    fn undo_renames_on_collision_at_source() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_with_target(tmp.path());
        move_sample(&config, "a.jpg", b"new");
        fs::write(config.watch.dirs[0].join("a.jpg"), b"occupied").unwrap();

        run(&config, false, &selection(true, None)).unwrap();
        assert_eq!(
            fs::read(config.watch.dirs[0].join("a.jpg")).unwrap(),
            b"occupied"
        );
        assert_eq!(
            fs::read(config.watch.dirs[0].join("a (2).jpg")).unwrap(),
            b"new"
        );
    }

    #[test]
    fn undo_restores_moved_folder_to_original_place() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_with_target(tmp.path());
        let dir = config.watch.dirs[0].join("vorlesung");
        fs::create_dir_all(dir.join("assets")).unwrap();
        fs::write(dir.join(".decker"), b"").unwrap();
        fs::write(dir.join("assets/slide.png"), b"p").unwrap();
        process_folder(&config, false, &dir).unwrap().unwrap();
        assert!(config.target_dir.join("Sammlungen/vorlesung").exists());

        run(&config, false, &selection(true, None)).unwrap();
        assert!(!config.target_dir.join("Sammlungen/vorlesung").exists());
        assert!(dir.join(".decker").exists());
        assert!(dir.join("assets/slide.png").exists());
    }

    #[test]
    fn dry_run_reports_without_moving() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_with_target(tmp.path());
        move_sample(&config, "a.jpg", b"j");

        run(&config, true, &selection(true, None)).unwrap();
        assert!(config.target_dir.join("Bilder/a.jpg").exists());
        assert!(!config.watch.dirs[0].join("a.jpg").exists());
    }

    #[test]
    fn legacy_journal_entries_without_id_can_be_undone() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_with_target(tmp.path());
        let dst = config.target_dir.join("Bilder/a.jpg");
        fs::create_dir_all(dst.parent().unwrap()).unwrap();
        fs::write(&dst, b"j").unwrap();
        fs::write(
            config.journal_path(),
            format!(
                "{{\"ts\":\"{}\",\"action\":\"moved\",\"src\":\"{}\",\"dst\":\"{}\",\"category\":\"Bilder\",\"sha256\":\"abc\"}}\n",
                chrono::Local::now().to_rfc3339(),
                config.watch.dirs[0].join("a.jpg").display(),
                dst.display()
            ),
        )
        .unwrap();

        run(&config, false, &selection(true, None)).unwrap();
        assert!(config.watch.dirs[0].join("a.jpg").exists());
        assert!(!dst.exists());
    }

    #[test]
    fn undo_refuses_malformed_middle_record() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_with_target(tmp.path());
        fs::create_dir_all(&config.target_dir).unwrap();
        fs::write(
            config.journal_path(),
            format!(
                "{{\"ts\":\"{}\",\"action\":\"moved\",\"src\":\"{}\",\"dst\":\"{}\"}}\nbroken\n",
                chrono::Local::now().to_rfc3339(),
                config.watch.dirs[0].join("a.jpg").display(),
                config.target_dir.join("Bilder/a.jpg").display()
            ),
        )
        .unwrap();

        let error = run(&config, false, &selection(true, None)).unwrap_err();
        assert!(error.to_string().contains("line 2") || error.to_string().contains("Zeile 2"));
    }

    #[test]
    fn undo_refuses_truncated_final_record() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_with_target(tmp.path());
        fs::create_dir_all(&config.target_dir).unwrap();
        fs::write(config.journal_path(), b"{\"ts\":\"unfinished").unwrap();

        let error = run(&config, false, &selection(true, None)).unwrap_err();
        assert!(
            error.to_string().contains("Truncated final journal record")
                || error
                    .to_string()
                    .contains("Abgeschnittener letzter Journaleintrag")
        );
        assert!(error.to_string().contains("line 1") || error.to_string().contains("Zeile 1"));
    }
}
