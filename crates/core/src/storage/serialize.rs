use crate::storage::fingerprints::DiskFingerprints;
use anyhow::Result;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Serialize value to YAML deterministically:
/// - 2-space indent
/// - LF line endings (no CRLF even on Windows)
/// - Trailing newline
/// - No document start prefix (`---`)
pub fn to_yaml_string<T: Serialize>(value: &T) -> Result<String> {
    let mut yaml = serde_yaml::to_string(value)?;
    // Strip document prefix if serde_yaml added it
    if let Some(stripped) = yaml.strip_prefix("---\n") {
        yaml = stripped.to_string();
    } else if let Some(stripped) = yaml.strip_prefix("---") {
        yaml = stripped.trim_start_matches('\n').to_string();
    }
    // Normalize any CRLF to LF
    yaml = yaml.replace("\r\n", "\n");
    if !yaml.ends_with('\n') {
        yaml.push('\n');
    }
    Ok(yaml)
}

/// Windows sharing-violation retry helper: retries 5x with delay (50ms, 100ms, ...).
pub fn with_retry<T, F: FnMut() -> std::io::Result<T>>(mut op: F) -> std::io::Result<T> {
    let mut attempts = 0;
    loop {
        match op() {
            Ok(val) => return Ok(val),
            Err(e) => {
                attempts += 1;
                if attempts >= 5 {
                    return Err(e);
                }
                std::thread::sleep(std::time::Duration::from_millis(50 * attempts as u64));
            }
        }
    }
}

/// Atomic file writer that stages temporary files inside `.local/tmp/` (outside
/// watched `collections/` and `environments/` directories) and records SHA-256
/// content fingerprints in `DiskFingerprints` BEFORE mutating the filesystem so
/// the watcher can deterministically filter out self-echo events.
#[derive(Debug, Clone)]
pub struct AtomicWriter {
    /// Directory used for staging temporary files prior to atomic renames.
    /// On Windows, writes are performed directly to avoid sharing violations,
    /// so this field is only read on non-Windows targets.
    #[cfg_attr(windows, allow(dead_code))]
    tmp_dir: PathBuf,
    fingerprints: Arc<DiskFingerprints>,
}

impl AtomicWriter {
    pub fn new(tmp_dir: PathBuf, fingerprints: Arc<DiskFingerprints>) -> Self {
        cleanup_stale_temp_files(&tmp_dir);
        Self {
            tmp_dir,
            fingerprints,
        }
    }

    pub fn tmp_dir(&self) -> &Path {
        &self.tmp_dir
    }

    pub fn fingerprints(&self) -> &Arc<DiskFingerprints> {
        &self.fingerprints
    }

    pub fn record_read(&self, path: &Path, bytes: &[u8]) {
        self.fingerprints.record_content(path, bytes);
    }

    /// Writes `content` to `path` atomically and records its fingerprint prior
    /// to touching disk.
    pub fn write_bytes(&self, path: &Path, content: &[u8]) -> std::io::Result<()> {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent)?;

        self.fingerprints.record_content(path, content);

        let res = self.write_bytes_inner(parent, path, content);
        if res.is_err() {
            self.fingerprints.forget(path);
        }
        res
    }

    #[cfg(windows)]
    fn write_bytes_inner(
        &self,
        _parent: &Path,
        path: &Path,
        content: &[u8],
    ) -> std::io::Result<()> {
        std::fs::write(path, content)
    }

    #[cfg(not(windows))]
    fn write_bytes_inner(&self, parent: &Path, path: &Path, content: &[u8]) -> std::io::Result<()> {
        let temp_name = format!(
            ".tmp_{}_{}",
            uuid::Uuid::new_v4().simple(),
            path.file_name().and_then(|n| n.to_str()).unwrap_or("file")
        );

        // Primary path: stage in `.local/tmp/` so watched directories never see
        // transient temp-file creation events.
        if std::fs::create_dir_all(&self.tmp_dir).is_ok() {
            let staged_path = self.tmp_dir.join(&temp_name);
            if std::fs::write(&staged_path, content).is_ok() {
                if with_retry(|| std::fs::rename(&staged_path, path)).is_ok() {
                    return Ok(());
                }
                let _ = std::fs::remove_file(&staged_path);
            }
        }

        // Fallback (e.g. EXDEV when `.local` is a cross-volume symlink): stage
        // a dot-prefixed temp file in the target directory (ignored by watcher).
        let fallback_temp = parent.join(&temp_name);
        std::fs::write(&fallback_temp, content)?;
        let rename_res = with_retry(|| std::fs::rename(&fallback_temp, path));
        if rename_res.is_err() {
            let _ = std::fs::remove_file(&fallback_temp);
        }
        rename_res
    }

    /// Writes `content` to `path` atomically with LF line endings.
    pub fn write_str(&self, path: &Path, content: &str) -> std::io::Result<()> {
        let normalized = content.replace("\r\n", "\n");
        self.write_bytes(path, normalized.as_bytes())
    }

    /// Removes `path` if it exists, recording `Fingerprint::Absent` beforehand.
    pub fn remove_file(&self, path: &Path) -> std::io::Result<()> {
        self.fingerprints.record_absent(path);
        if path.exists() {
            with_retry(|| std::fs::remove_file(path))?;
        }
        Ok(())
    }

    /// Renames `from` to `to`, recording `Absent` on `from` and the transferred
    /// content fingerprint on `to` before renaming.
    pub fn rename_file(&self, from: &Path, to: &Path) -> std::io::Result<()> {
        let existing_bytes = std::fs::read(from).ok();
        self.fingerprints.record_absent(from);
        if let Some(ref bytes) = existing_bytes {
            self.fingerprints.record_content(to, bytes);
        }
        let res = with_retry(|| std::fs::rename(from, to));
        if res.is_err() {
            if let Some(ref bytes) = existing_bytes {
                self.fingerprints.record_content(from, bytes);
            }
            self.fingerprints.forget(to);
        }
        res
    }
}

fn cleanup_stale_temp_files(tmp_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(tmp_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(".tmp_"))
        {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Standalone atomic write helper (used where no `AtomicWriter` instance is needed).
pub fn atomic_write(path: &Path, content: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;

    #[cfg(windows)]
    {
        std::fs::write(path, content)
    }

    #[cfg(not(windows))]
    {
        let temp_name = format!(
            ".tmp_{}_{}",
            uuid::Uuid::new_v4().simple(),
            path.file_name().and_then(|n| n.to_str()).unwrap_or("file")
        );
        let temp_path = parent.join(temp_name);

        std::fs::write(&temp_path, content)?;

        let rename_res = with_retry(|| std::fs::rename(&temp_path, path));
        if rename_res.is_err() {
            let _ = std::fs::remove_file(&temp_path);
        }
        rename_res
    }
}

/// Standalone atomic write string (ensuring LF line endings).
pub fn atomic_write_str(path: &Path, content: &str) -> std::io::Result<()> {
    let normalized = content.replace("\r\n", "\n");
    atomic_write(path, normalized.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_writer_stages_in_tmp_dir_and_records_fingerprint() {
        let dir = tempfile::tempdir().unwrap();
        let tmp_dir = dir.path().join(".local").join("tmp");
        let env_dir = dir.path().join("environments");
        std::fs::create_dir_all(&tmp_dir).unwrap();

        // Plant a leftover temp file from a simulated prior crash
        let stale_tmp = tmp_dir.join(".tmp_stale_dev.yaml");
        std::fs::write(&stale_tmp, "stale").unwrap();
        assert!(stale_tmp.exists());

        let fp = Arc::new(DiskFingerprints::new());
        let writer = AtomicWriter::new(tmp_dir.clone(), fp.clone());
        assert_eq!(writer.tmp_dir(), tmp_dir.as_path());

        // Stale temp file must be cleaned up on initialization
        assert!(!stale_tmp.exists());

        let target = env_dir.join("dev.yaml");
        writer.write_str(&target, "variables: []\n").unwrap();

        assert!(target.exists());
        assert!(fp.matches_disk(&target));

        // No `.tmp_*` files left in either `.local/tmp` or `environments/`
        let env_entries: Vec<_> = std::fs::read_dir(&env_dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(env_entries, vec!["dev.yaml".to_string()]);
    }
}
