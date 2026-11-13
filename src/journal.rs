use anyhow::{Context, Result};
use chrono::Local;
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
struct JournalEntry {
    ts: String,
    action: &'static str,
    src: String,
    dst: String,
    category: String,
    sha256: String,
}

pub fn append(
    journal_path: &PathBuf,
    action: &'static str,
    src: &Path,
    dst: &Path,
    category: &str,
    sha256: &str,
) -> Result<()> {
    let entry = JournalEntry {
        ts: Local::now().to_rfc3339(),
        action,
        src: src.display().to_string(),
        dst: dst.display().to_string(),
        category: category.to_string(),
        sha256: sha256.to_string(),
    };
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(journal_path)
        .with_context(|| format!("Journal nicht öffnbar: {}", journal_path.display()))?;
    let line = serde_json::to_string(&entry)?;
    writeln!(file, "{line}")?;
    Ok(())
}
