use crate::organize::Outcome;
use anyhow::Result;
use chrono::Local;
use serde::Serialize;
use std::io::Write;
use std::path::Path;

#[derive(Serialize)]
struct ReportEntry<'a> {
    ts: String,
    action: &'static str,
    src: &'a Path,
    dst: &'a Path,
    category: &'a str,
    rule: Option<&'a str>,
    dry_run: bool,
}

pub fn append(report_path: &Path, outcome: &Outcome, dry_run: bool) -> Result<()> {
    if dry_run {
        return Ok(());
    }
    let entry = ReportEntry {
        ts: Local::now().to_rfc3339(),
        action: outcome.action.as_str(),
        src: &outcome.src,
        dst: &outcome.dst,
        category: &outcome.category,
        rule: outcome.rule.as_deref(),
        dry_run,
    };
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(report_path)?;
    writeln!(file, "{}", serde_json::to_string(&entry)?)?;
    file.flush()?;
    file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organize::{Action, Outcome};
    use std::path::Path;

    #[test]
    fn append_reports_unwritable_destination() {
        let tmp = tempfile::tempdir().unwrap();
        let report_path = tmp.path().join("missing").join("report.jsonl");
        let outcome = Outcome {
            action: Action::Moved,
            src: tmp.path().join("source.txt"),
            dst: tmp.path().join("target.txt"),
            category: "Sonstiges".to_string(),
            sha256: "abc".to_string(),
            rule: None,
        };

        assert!(append(&report_path, &outcome, false).is_err());
    }

    #[test]
    fn dry_run_report_does_not_create_file() {
        let tmp = tempfile::tempdir().unwrap();
        let report_path = tmp.path().join("report.jsonl");
        let outcome = Outcome {
            action: Action::Moved,
            src: tmp.path().join("source.txt"),
            dst: tmp.path().join("target.txt"),
            category: "Sonstiges".to_string(),
            sha256: "abc".to_string(),
            rule: None,
        };

        append(&report_path, &outcome, true).unwrap();
        assert!(!Path::new(&report_path).exists());
    }
}
