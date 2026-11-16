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
    Ok(())
}
