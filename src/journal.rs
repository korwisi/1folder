use anyhow::{Context, Result};
use chrono::Local;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::Path;

#[derive(Serialize)]
struct JournalEntry {
    ts: String,
    action: &'static str,
    src: String,
    dst: String,
    category: String,
    sha256: String,
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    undo_of: Option<String>,
}

#[derive(Deserialize)]
pub struct JournalRecord {
    pub ts: String,
    pub action: String,
    pub src: String,
    pub dst: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub undo_of: Option<String>,
}

fn entry_id(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0x1f]);
    }
    hex::encode(hasher.finalize())
}

fn write_entry(journal_path: &Path, entry: &JournalEntry) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(journal_path)
        .with_context(|| format!("Journal nicht öffnbar: {}", journal_path.display()))?;
    let line = serde_json::to_string(entry)?;
    writeln!(file, "{line}")?;
    Ok(())
}

pub fn append(
    journal_path: &Path,
    action: &'static str,
    src: &Path,
    dst: &Path,
    category: &str,
    sha256: &str,
) -> Result<()> {
    let ts = Local::now().to_rfc3339();
    let src_str = src.display().to_string();
    let dst_str = dst.display().to_string();
    let id = entry_id(&[&ts, action, &src_str, &dst_str, category, sha256]);
    write_entry(
        journal_path,
        &JournalEntry {
            ts,
            action,
            src: src_str,
            dst: dst_str,
            category: category.to_string(),
            sha256: sha256.to_string(),
            id,
            undo_of: None,
        },
    )
}

pub fn append_undo(
    journal_path: &Path,
    undo_of: Option<&str>,
    from: &Path,
    to: &Path,
) -> Result<()> {
    let ts = Local::now().to_rfc3339();
    let from_str = from.display().to_string();
    let to_str = to.display().to_string();
    let id = entry_id(&[&ts, "undone", &from_str, &to_str, "", ""]);
    write_entry(
        journal_path,
        &JournalEntry {
            ts,
            action: "undone",
            src: from_str,
            dst: to_str,
            category: String::new(),
            sha256: String::new(),
            id,
            undo_of: undo_of.map(|s| s.to_string()),
        },
    )
}

pub fn read(journal_path: &Path) -> Result<Vec<JournalRecord>> {
    let raw = match std::fs::read_to_string(journal_path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e.into()),
    };
    Ok(raw
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect())
}
