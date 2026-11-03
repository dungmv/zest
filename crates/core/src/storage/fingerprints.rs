use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Last known on-disk state of a workspace file as observed or written by the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fingerprint {
    /// SHA-256 digest of the file's normalized byte content.
    Content([u8; 32]),
    /// The file was intentionally removed or renamed away by the app.
    Absent,
}

/// Thread-safe registry of on-disk file fingerprints used by the workspace
/// watcher to distinguish self-echo events from genuine external modifications
/// (analogous to VS Code's `etag` and Zed's `saved_mtime` / `saved_version`).
#[derive(Debug, Default)]
pub struct DiskFingerprints {
    map: Mutex<HashMap<String, Fingerprint>>,
}

impl DiskFingerprints {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the SHA-256 fingerprint of `bytes` for `path`.
    /// Call this BEFORE writing to disk (and after reading from disk) so the
    /// watcher never observes app-written content without a matching entry.
    pub fn record_content(&self, path: &Path, bytes: &[u8]) {
        let hash = sha256_bytes(bytes);
        let key = normalize_path_key(path);
        let mut guard = self.map.lock().unwrap_or_else(|p| p.into_inner());
        guard.insert(key, Fingerprint::Content(hash));
    }

    /// Records that `path` was deleted or renamed away by the app.
    pub fn record_absent(&self, path: &Path) {
        let key = normalize_path_key(path);
        let mut guard = self.map.lock().unwrap_or_else(|p| p.into_inner());
        guard.insert(key, Fingerprint::Absent);
    }

    /// Removes any recorded fingerprint for `path`.
    pub fn forget(&self, path: &Path) {
        let key = normalize_path_key(path);
        let mut guard = self.map.lock().unwrap_or_else(|p| p.into_inner());
        guard.remove(&key);
    }

    /// Returns the currently recorded fingerprint for `path`, if any.
    pub fn get(&self, path: &Path) -> Option<Fingerprint> {
        let key = normalize_path_key(path);
        let guard = self.map.lock().unwrap_or_else(|p| p.into_inner());
        guard.get(&key).copied()
    }

    /// Returns `true` when the current on-disk state of `path` matches the last
    /// fingerprint recorded by the app (meaning a watcher notification for
    /// `path` is a self-echo and should be ignored).
    pub fn matches_disk(&self, path: &Path) -> bool {
        let key = normalize_path_key(path);
        let recorded = {
            let guard = self.map.lock().unwrap_or_else(|p| p.into_inner());
            guard.get(&key).copied()
        };

        match recorded {
            Some(Fingerprint::Content(expected)) => match std::fs::read(path) {
                Ok(bytes) => sha256_bytes(&bytes) == expected,
                Err(_) => false,
            },
            Some(Fingerprint::Absent) => {
                if !path.exists() {
                    true
                } else {
                    // File reappeared on disk externally; clear stale Absent marker.
                    let mut guard = self.map.lock().unwrap_or_else(|p| p.into_inner());
                    if guard.get(&key) == Some(&Fingerprint::Absent) {
                        guard.remove(&key);
                    }
                    false
                }
            }
            None => false,
        }
    }
}

pub fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

pub fn strip_unc_str(s: &str) -> &str {
    let s = s.strip_prefix(r"\\?\").unwrap_or(s);
    if s.starts_with("/private/var/") || s.starts_with("/private/tmp/") {
        &s["/private".len()..]
    } else {
        s
    }
}

pub fn strip_unc_prefix(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        p.to_path_buf()
    }
}

/// Produces a normalized, platform-consistent lookup key for `path` (strips
/// Windows UNC prefixes and macOS `/private/var` or `/private/tmp` symlink
/// prefixes, unifies separators, and ASCII-lowercases).
pub fn normalize_path_key(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let trimmed = strip_unc_str(&raw);
    trimmed
        .chars()
        .map(|c| {
            if c == '/' {
                '\\'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect()
}

pub fn path_matches(p1: &Path, p2: &Path) -> bool {
    let s1 = p1.to_string_lossy();
    let s2 = p2.to_string_lossy();
    let t1 = strip_unc_str(&s1);
    let t2 = strip_unc_str(&s2);

    let normalize_ch = |c: char| -> char {
        if c == '/' {
            '\\'
        } else {
            c.to_ascii_lowercase()
        }
    };

    t1.chars()
        .map(normalize_ch)
        .eq(t2.chars().map(normalize_ch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_fingerprint_matches_and_detects_external_change() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("dev.yaml");
        let fp = DiskFingerprints::new();

        let initial = b"variables:\n  - name: host\n    value: localhost\n";
        fp.record_content(&file, initial);
        std::fs::write(&file, initial).unwrap();
        assert!(fp.matches_disk(&file));

        // External modification changes content -> must not match
        std::fs::write(&file, b"variables:\n  - name: host\n    value: prod\n").unwrap();
        assert!(!fp.matches_disk(&file));

        // External deletion when content was expected -> must not match
        std::fs::remove_file(&file).unwrap();
        assert!(!fp.matches_disk(&file));
    }

    #[test]
    fn test_absent_fingerprint_handles_repeated_checks_and_recreation() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("gone.yaml");
        let fp = DiskFingerprints::new();

        fp.record_absent(&file);
        // Multiple FSEvents deliveries for the same deletion all match
        assert!(fp.matches_disk(&file));
        assert!(fp.matches_disk(&file));

        // External recreation of the file must be detected (not suppressed)
        std::fs::write(&file, b"name: Recreated\n").unwrap();
        assert!(!fp.matches_disk(&file));
        assert_eq!(fp.get(&file), None);
    }

    #[test]
    fn test_normalize_path_key_cross_platform() {
        assert_eq!(
            normalize_path_key(Path::new("/private/var/folders/xx/dev.yaml")),
            normalize_path_key(Path::new("/var/folders/XX/DEV.yaml"))
        );
        assert_eq!(
            normalize_path_key(Path::new(r"\\?\C:\Workspace\environments\dev.yaml")),
            normalize_path_key(Path::new("c:/workspace/environments/dev.yaml"))
        );
    }
}
