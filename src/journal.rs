use anyhow::{Context, Result};
use chrono::Local;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::Path;

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TestJournalFault {
    Append,
}

#[cfg(test)]
std::thread_local! {
    static TEST_JOURNAL_FAULT: std::cell::Cell<Option<TestJournalFault>> =
        const { std::cell::Cell::new(None) };
}

#[cfg(test)]
pub(crate) fn set_test_fault(fault: TestJournalFault) {
    TEST_JOURNAL_FAULT.with(|state| state.set(Some(fault)));
}

#[cfg(test)]
fn fail_test_fault() -> Result<()> {
    let should_fail =
        TEST_JOURNAL_FAULT.with(|state| state.get() == Some(TestJournalFault::Append));
    if should_fail {
        TEST_JOURNAL_FAULT.with(|state| state.set(None));
        anyhow::bail!("Injected journal append failure");
    }
    Ok(())
}

#[cfg(not(test))]
fn fail_test_fault() -> Result<()> {
    Ok(())
}

#[derive(Debug)]
pub enum JournalReadError {
    Unreadable(std::io::Error),
    Malformed { line: usize, reason: String },
    Truncated { line: usize, reason: String },
}

impl std::fmt::Display for JournalReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable(error) => write!(f, "journal could not be read: {error}"),
            Self::Malformed { line, reason } => {
                write!(f, "malformed journal record at line {line}: {reason}")
            }
            Self::Truncated { line, reason } => {
                write!(f, "truncated final journal record at line {line}: {reason}")
            }
        }
    }
}

impl std::error::Error for JournalReadError {}

#[derive(Serialize)]
struct JournalEntry {
    ts: String,
    action: String,
    src: String,
    dst: String,
    category: String,
    sha256: String,
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    undo_of: Option<String>,
}

#[derive(Debug, Deserialize)]
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
    fail_test_fault()?;
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
    action: &str,
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
            action: action.to_string(),
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
            action: "undone".to_string(),
            src: from_str,
            dst: to_str,
            category: String::new(),
            sha256: String::new(),
            id,
            undo_of: undo_of.map(|s| s.to_string()),
        },
    )
}

pub fn read(journal_path: &Path) -> std::result::Result<Vec<JournalRecord>, JournalReadError> {
    let raw = match std::fs::read(journal_path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(JournalReadError::Unreadable(e)),
    };
    if raw.is_empty() {
        return Ok(vec![]);
    }

    let ends_with_newline = raw.ends_with(b"\n");
    let text = std::str::from_utf8(&raw).map_err(|error| {
        let prefix = &raw[..error.valid_up_to()];
        let line = prefix.iter().filter(|byte| **byte == b'\n').count() + 1;
        if !ends_with_newline {
            JournalReadError::Truncated {
                line,
                reason: "invalid UTF-8".to_string(),
            }
        } else {
            JournalReadError::Malformed {
                line,
                reason: "invalid UTF-8".to_string(),
            }
        }
    })?;

    let mut records = Vec::new();
    let lines: Vec<&str> = text.split('\n').collect();
    let line_count = if ends_with_newline {
        lines.len() - 1
    } else {
        lines.len()
    };

    for (index, raw_line) in lines.iter().enumerate().take(line_count) {
        let line_number = index + 1;
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if line.is_empty() {
            return Err(JournalReadError::Malformed {
                line: line_number,
                reason: "empty record".to_string(),
            });
        }
        match serde_json::from_str::<JournalRecord>(line) {
            Ok(record) => {
                if !ends_with_newline && line_number == lines.len() {
                    return Err(JournalReadError::Truncated {
                        line: line_number,
                        reason: "record is not terminated by a newline".to_string(),
                    });
                }
                records.push(record);
            }
            Err(error) => {
                let truncated = !ends_with_newline && line_number == lines.len();
                if truncated {
                    return Err(JournalReadError::Truncated {
                        line: line_number,
                        reason: error.to_string(),
                    });
                }
                return Err(JournalReadError::Malformed {
                    line: line_number,
                    reason: error.to_string(),
                });
            }
        }
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_line(action: &str) -> String {
        serde_json::json!({
            "ts": "2026-11-24T15:00:00+01:00",
            "action": action,
            "src": "/tmp/source",
            "dst": "/tmp/destination"
        })
        .to_string()
    }

    #[test]
    fn read_accepts_complete_records() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("journal.jsonl");
        std::fs::write(
            &path,
            format!("{}\n{}\n", valid_line("moved"), valid_line("undone")),
        )
        .unwrap();

        let records = read(&path).unwrap();
        assert_eq!(records.len(), 2);
    }

    #[test]
    fn read_reports_malformed_middle_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("journal.jsonl");
        std::fs::write(
            &path,
            format!(
                "{}\nnot-json\n{}\n",
                valid_line("moved"),
                valid_line("undone")
            ),
        )
        .unwrap();

        match read(&path).unwrap_err() {
            JournalReadError::Malformed { line, .. } => assert_eq!(line, 2),
            error => panic!("unexpected error: {error:?}"),
        }
    }

    #[test]
    fn read_reports_truncated_final_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("journal.jsonl");
        std::fs::write(&path, valid_line("moved").trim_end_matches('}')).unwrap();

        match read(&path).unwrap_err() {
            JournalReadError::Truncated { line, .. } => assert_eq!(line, 1),
            error => panic!("unexpected error: {error:?}"),
        }
    }

    #[test]
    fn read_reports_complete_unterminated_final_record() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("journal.jsonl");
        std::fs::write(&path, valid_line("moved")).unwrap();

        assert!(matches!(
            read(&path),
            Err(JournalReadError::Truncated { line: 1, .. })
        ));
    }
}
