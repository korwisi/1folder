use crate::hash::sha256_file;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct CachedFile {
    pub size: u64,
    pub modified: i64,
    #[serde(default)]
    pub modified_nanos: u32,
    #[serde(default)]
    pub device: Option<u64>,
    #[serde(default)]
    pub inode: Option<u64>,
    #[serde(default)]
    pub changed: Option<i64>,
    #[serde(default)]
    pub changed_nanos: Option<u32>,
    pub hash: String,
}

/// Persistent SHA-256 index over the target folder.
/// Stores path, size, modification time, and hash for each file.
/// Reuses hashes when file metadata (size/mtime) matches; rehashes on changes.
#[derive(Debug)]
pub struct PersistentIndex {
    cache: HashMap<PathBuf, CachedFile>,
    by_hash: HashMap<String, PathBuf>,
    index_path: PathBuf,
    dirty: bool,
}

impl PersistentIndex {
    /// Load or create a fresh index at the given path.
    pub fn load_or_create(target_dir: &Path) -> Result<Self> {
        let index_path = Self::index_file_path(target_dir);
        let cache = if index_path.exists() {
            Self::read_index(&index_path)?
        } else {
            HashMap::new()
        };
        let by_hash = Self::build_by_hash(&cache);
        Ok(Self {
            cache,
            by_hash,
            index_path,
            dirty: false,
        })
    }

    fn index_file_path(target_dir: &Path) -> PathBuf {
        target_dir.join(".1folder_index.json")
    }

    fn read_index(path: &Path) -> Result<HashMap<PathBuf, CachedFile>> {
        let file = File::open(path)
            .with_context(|| format!("Failed to open index: {}", path.display()))?;
        let reader = BufReader::new(file);
        let index: HashMap<PathBuf, CachedFile> = serde_json::from_reader(reader)
            .with_context(|| format!("Failed to parse index: {}", path.display()))?;
        Ok(index)
    }

    fn build_by_hash(cache: &HashMap<PathBuf, CachedFile>) -> HashMap<String, PathBuf> {
        cache
            .iter()
            .map(|(path, cached)| (cached.hash.clone(), path.clone()))
            .collect()
    }

    fn time_to_parts(time: SystemTime) -> (i64, u32) {
        match time.duration_since(SystemTime::UNIX_EPOCH) {
            Ok(duration) => (duration.as_secs() as i64, duration.subsec_nanos()),
            Err(error) => {
                let duration = error.duration();
                if duration.subsec_nanos() == 0 {
                    (-(duration.as_secs() as i64), 0)
                } else {
                    (
                        -(duration.as_secs() as i64) - 1,
                        1_000_000_000 - duration.subsec_nanos(),
                    )
                }
            }
        }
    }

    #[allow(dead_code)]
    fn system_time_from_parts(ts: i64, nanos: u32) -> SystemTime {
        if ts >= 0 {
            SystemTime::UNIX_EPOCH + std::time::Duration::new(ts as u64, nanos)
        } else {
            SystemTime::UNIX_EPOCH - std::time::Duration::from_secs((-ts) as u64)
                + std::time::Duration::from_nanos(nanos as u64)
        }
    }

    #[cfg(unix)]
    fn unix_identity(meta: &fs::Metadata) -> (Option<u64>, Option<u64>, Option<i64>, Option<u32>) {
        use std::os::unix::fs::MetadataExt;
        (
            Some(meta.dev()),
            Some(meta.ino()),
            Some(meta.ctime()),
            Some(meta.ctime_nsec() as u32),
        )
    }

    #[cfg(not(unix))]
    fn unix_identity(_meta: &fs::Metadata) -> (Option<u64>, Option<u64>, Option<i64>, Option<u32>) {
        (None, None, None, None)
    }

    /// Refresh the index against the target folder. Reconciles missing files,
    /// hashes new/changed files, and reuses hashes for unchanged files.
    pub fn refresh(&mut self, target: &Path) -> Result<()> {
        let mut entries: Vec<PathBuf> = Vec::new();
        for entry in walkdir::WalkDir::new(target).follow_links(false) {
            let Ok(entry) = entry else { continue };
            if entry.file_type().is_file() {
                let path = entry.path().to_path_buf();
                let file_name = path.file_name().unwrap_or_default();
                // Skip 1folder's own state and system files
                if file_name != std::ffi::OsStr::new(".1folder_index.json")
                    && file_name != std::ffi::OsStr::new(".1folder-journal.jsonl")
                    && file_name != std::ffi::OsStr::new(".1folder-operation.json")
                    && !file_name
                        .to_string_lossy()
                        .starts_with(".1folder-operation")
                    && file_name != std::ffi::OsStr::new(".DS_Store")
                {
                    entries.push(path);
                }
            }
        }
        entries.sort();

        let mut fresh_cache = HashMap::new();
        let mut by_hash = HashMap::new();

        for path in entries {
            let meta = match fs::metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };

            let size = meta.len();
            let mtime = meta.modified().ok();
            let (mtime_secs, mtime_nanos) = mtime.map(Self::time_to_parts).unwrap_or((0, 0));
            let (device, inode, changed, changed_nanos) = Self::unix_identity(&meta);

            let hash = match self.cache.get(&path) {
                Some(cached)
                    if cached.size == size
                        && cached.modified == mtime_secs
                        && cached.modified_nanos == mtime_nanos
                        && cached.device == device
                        && cached.inode == inode
                        && cached.changed == changed
                        && cached.changed_nanos == changed_nanos =>
                {
                    cached.hash.clone()
                }
                _ => match sha256_file(&path) {
                    Ok(h) => h,
                    Err(_) => continue,
                },
            };

            by_hash.entry(hash.clone()).or_insert_with(|| path.clone());
            fresh_cache.insert(
                path,
                CachedFile {
                    size,
                    modified: mtime_secs,
                    modified_nanos: mtime_nanos,
                    device,
                    inode,
                    changed,
                    changed_nanos,
                    hash,
                },
            );
        }

        let changed = fresh_cache.len() != self.cache.len()
            || fresh_cache
                .iter()
                .any(|(path, cached)| self.cache.get(path) != Some(cached));

        self.cache = fresh_cache;
        self.by_hash = by_hash;
        if changed {
            self.dirty = true;
        }
        Ok(())
    }

    /// Look up a file by its SHA-256 hash.
    pub fn lookup(&self, sha256: &str) -> Option<&Path> {
        self.by_hash.get(sha256).map(|p| p.as_path())
    }

    /// Register a new file/hash pair.
    pub fn insert(&mut self, sha256: &str, path: &Path) {
        self.by_hash
            .entry(sha256.to_string())
            .or_insert_with(|| path.to_path_buf());
    }

    /// Remove a file from the index (e.g., after it's moved out of target).
    #[allow(dead_code)]
    pub fn remove(&mut self, path: &Path) -> Option<CachedFile> {
        if let Some(cached) = self.cache.remove(path) {
            if self.by_hash.get(&cached.hash) == Some(&path.to_path_buf()) {
                self.by_hash.remove(&cached.hash);
            }
            self.dirty = true;
            Some(cached)
        } else {
            None
        }
    }

    /// Mark the index as dirty so it will be persisted on next flush.
    #[allow(dead_code)]
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Write the index to disk atomically (via temp file + rename).
    pub fn flush(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }

        let temp_path = self.index_path.with_extension("json.tmp");
        {
            let file = File::create(&temp_path)
                .with_context(|| format!("Failed to create temp index: {}", temp_path.display()))?;
            let writer = BufWriter::new(file);
            serde_json::to_writer_pretty(writer, &self.cache)
                .with_context(|| format!("Failed to write index: {}", temp_path.display()))?;
        }

        fs::rename(&temp_path, &self.index_path).with_context(|| {
            format!(
                "Failed to rename temp index to {}",
                self.index_path.display()
            )
        })?;

        self.dirty = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "run explicitly with: cargo test --release index::tests::benchmark_hash_index_refresh -- --ignored --nocapture"]
    fn benchmark_hash_index_refresh() {
        for file_count in [100, 1_000, 5_000] {
            let dir = tempfile::tempdir().unwrap();
            for index in 0..file_count {
                fs::write(
                    dir.path().join(format!("file-{index:05}.bin")),
                    vec![b'x'; 1024],
                )
                .unwrap();
            }

            let mut idx = PersistentIndex::load_or_create(dir.path()).unwrap();
            let cold_started = std::time::Instant::now();
            idx.refresh(dir.path()).unwrap();
            let cold = cold_started.elapsed();

            let mut warm_runs = Vec::new();
            for _ in 0..3 {
                let warm_started = std::time::Instant::now();
                idx.refresh(dir.path()).unwrap();
                warm_runs.push(warm_started.elapsed());
            }
            let warm_average =
                warm_runs.iter().sum::<std::time::Duration>() / warm_runs.len() as u32;

            println!(
                "{file_count} target files: cold={cold:?}, warm_average={warm_average:?}, warm_runs={warm_runs:?}"
            );
        }
    }

    #[test]
    fn fresh_index_has_no_entries() {
        let dir = tempfile::tempdir().unwrap();
        let idx = PersistentIndex::load_or_create(dir.path()).unwrap();
        assert!(idx.cache.is_empty());
        assert!(idx.by_hash.is_empty());
    }

    #[test]
    fn refresh_discovers_files_and_computes_hashes() {
        let dir = tempfile::tempdir().unwrap();
        let file_a = dir.path().join("a.txt");
        let file_b = dir.path().join("b.txt");
        fs::write(&file_a, b"hello").unwrap();
        fs::write(&file_b, b"world").unwrap();

        let mut idx = PersistentIndex::load_or_create(dir.path()).unwrap();
        idx.refresh(dir.path()).unwrap();

        assert_eq!(idx.cache.len(), 2);
        assert_eq!(idx.by_hash.len(), 2);
        assert!(idx.lookup("hello").is_none());
    }

    #[test]
    fn refresh_reuses_hash_for_unchanged_files() {
        let dir = tempfile::tempdir().unwrap();
        let file_a = dir.path().join("a.txt");
        fs::write(&file_a, b"hello").unwrap();

        let mut idx = PersistentIndex::load_or_create(dir.path()).unwrap();
        idx.refresh(dir.path()).unwrap();
        idx.flush().unwrap();
        idx.dirty = false;

        let first_hash = idx.cache.get(&file_a).map(|c| c.hash.clone()).unwrap();

        // Refresh again without changing the file
        idx.refresh(dir.path()).unwrap();
        let second_hash = idx.cache.get(&file_a).map(|c| c.hash.clone()).unwrap();

        assert_eq!(first_hash, second_hash);
        assert!(!idx.dirty);
    }

    #[test]
    fn refresh_rehashes_modified_files() {
        let dir = tempfile::tempdir().unwrap();
        let file_a = dir.path().join("a.txt");
        fs::write(&file_a, b"hello").unwrap();

        let mut idx = PersistentIndex::load_or_create(dir.path()).unwrap();
        idx.refresh(dir.path()).unwrap();

        // Modify the file
        std::thread::sleep(std::time::Duration::from_millis(100));
        fs::write(&file_a, b"hello world").unwrap();

        idx.refresh(dir.path()).unwrap();
        assert!(idx.dirty);
    }

    #[test]
    fn refresh_rehashes_same_size_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let file_a = dir.path().join("a.txt");
        fs::write(&file_a, b"hello").unwrap();

        let fixed_mtime = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let times = fs::FileTimes::new()
            .set_modified(fixed_mtime)
            .set_accessed(fixed_mtime);
        fs::File::options()
            .write(true)
            .open(&file_a)
            .unwrap()
            .set_times(times)
            .unwrap();

        let mut idx = PersistentIndex::load_or_create(dir.path()).unwrap();
        idx.refresh(dir.path()).unwrap();
        let first_hash = idx.cache.get(&file_a).unwrap().hash.clone();

        fs::write(&file_a, b"world").unwrap();
        fs::File::options()
            .write(true)
            .open(&file_a)
            .unwrap()
            .set_times(times)
            .unwrap();

        idx.refresh(dir.path()).unwrap();
        let replacement_hash = idx.cache.get(&file_a).unwrap().hash.clone();
        assert_ne!(first_hash, replacement_hash);
        assert!(idx.dirty);
    }

    #[test]
    fn flush_creates_index_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_a = dir.path().join("a.txt");
        fs::write(&file_a, b"hello").unwrap();

        let mut idx = PersistentIndex::load_or_create(dir.path()).unwrap();
        idx.refresh(dir.path()).unwrap();
        idx.flush().unwrap();

        let index_path = PersistentIndex::index_file_path(dir.path());
        assert!(index_path.exists());
    }

    #[test]
    fn load_index_recovers_state() {
        let dir = tempfile::tempdir().unwrap();
        let file_a = dir.path().join("a.txt");
        fs::write(&file_a, b"hello").unwrap();

        let mut idx1 = PersistentIndex::load_or_create(dir.path()).unwrap();
        idx1.refresh(dir.path()).unwrap();
        idx1.flush().unwrap();

        // Load again
        let idx2 = PersistentIndex::load_or_create(dir.path()).unwrap();
        assert_eq!(idx1.cache.len(), idx2.cache.len());
        assert_eq!(idx1.by_hash.len(), idx2.by_hash.len());
    }

    #[test]
    fn remove_deletes_file_from_index() {
        let dir = tempfile::tempdir().unwrap();
        let file_a = dir.path().join("a.txt");
        fs::write(&file_a, b"hello").unwrap();

        let mut idx = PersistentIndex::load_or_create(dir.path()).unwrap();
        idx.refresh(dir.path()).unwrap();
        assert_eq!(idx.cache.len(), 1);

        idx.remove(&file_a);
        assert_eq!(idx.cache.len(), 0);
        assert!(idx.dirty);
    }

    #[test]
    fn insert_adds_file_to_by_hash() {
        let dir = tempfile::tempdir().unwrap();
        let file_a = dir.path().join("a.txt");
        let test_hash = "abc123";

        let mut idx = PersistentIndex::load_or_create(dir.path()).unwrap();
        idx.insert(test_hash, &file_a);

        assert_eq!(idx.lookup(test_hash), Some(file_a.as_path()));
    }

    #[test]
    fn skip_index_file_itself() {
        let dir = tempfile::tempdir().unwrap();
        let file_a = dir.path().join("a.txt");
        fs::write(&file_a, b"hello").unwrap();

        let mut idx = PersistentIndex::load_or_create(dir.path()).unwrap();
        idx.refresh(dir.path()).unwrap();
        idx.flush().unwrap();

        // Refresh again - should not include the .1folder_index.json file
        idx.refresh(dir.path()).unwrap();
        assert_eq!(idx.cache.len(), 1);
    }

    #[test]
    fn skip_journal_and_index_state_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("kept.txt"), b"kept").unwrap();
        fs::write(
            dir.path().join(".1folder-journal.jsonl"),
            b"{\"action\":\"moved\"}\n",
        )
        .unwrap();

        let mut idx = PersistentIndex::load_or_create(dir.path()).unwrap();
        idx.refresh(dir.path()).unwrap();
        idx.flush().unwrap();

        assert_eq!(idx.cache.len(), 1);
        assert!(idx.lookup("anything").is_none());
    }
}
