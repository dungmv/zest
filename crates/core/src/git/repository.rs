use anyhow::Result;
use std::path::{Path, PathBuf};

use super::command::run_git_cmd;

/// Initialize a new git repository
pub fn git_init(repo_dir: &Path) -> Result<()> {
    run_git_cmd(repo_dir, &["init"]).map(|_| ())
}

/// Find repository root directory starting from start_path traversing upwards
pub fn find_git_root(start_path: &Path) -> Option<PathBuf> {
    let mut current = if start_path.is_file() {
        start_path.parent()?.to_path_buf()
    } else {
        start_path.to_path_buf()
    };

    loop {
        if current.join(".git").exists() {
            return Some(current);
        }
        if !current.pop() {
            break;
        }
    }
    None
}

/// Quickly detect if path is inside a Git repository without spawning git child process.
/// Inspects `.git` directory and reads `.git/HEAD` if present.
pub fn detect_git_repo_fast(start_path: &Path) -> Option<(PathBuf, Option<String>)> {
    let root = find_git_root(start_path)?;
    let git_dir = root.join(".git");
    let head_path = if git_dir.is_file() {
        // Submodule or worktree: gitdir: <path>
        if let Ok(content) = std::fs::read_to_string(&git_dir) {
            if let Some(target) = content.trim().strip_prefix("gitdir:") {
                let p = PathBuf::from(target.trim());
                let real_git_dir = if p.is_relative() { root.join(p) } else { p };
                real_git_dir.join("HEAD")
            } else {
                git_dir.join("HEAD")
            }
        } else {
            git_dir.join("HEAD")
        }
    } else {
        git_dir.join("HEAD")
    };

    let branch = if let Ok(content) = std::fs::read_to_string(&head_path) {
        let trimmed = content.trim();
        if let Some(ref_path) = trimmed.strip_prefix("ref: refs/heads/") {
            Some(ref_path.to_string())
        } else if trimmed.len() >= 7 && !trimmed.contains(' ') {
            // Detached HEAD: show short hash
            Some(trimmed[..7.min(trimmed.len())].to_string())
        } else {
            None
        }
    } else {
        None
    };

    Some((root, branch))
}
