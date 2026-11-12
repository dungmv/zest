use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// Flags for hiding the console window when running child processes on Windows
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitFileStatus {
    Modified,
    Added,
    Deleted,
    Untracked,
    Renamed,
    Copied,
    Conflicted,
    Unknown,
}

impl GitFileStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Modified => "Modified",
            Self::Added => "Added",
            Self::Deleted => "Deleted",
            Self::Untracked => "Untracked",
            Self::Renamed => "Renamed",
            Self::Copied => "Copied",
            Self::Conflicted => "Conflicted",
            Self::Unknown => "Unknown",
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Modified => "M",
            Self::Added => "A",
            Self::Deleted => "D",
            Self::Untracked => "U",
            Self::Renamed => "R",
            Self::Copied => "C",
            Self::Conflicted => "!",
            Self::Unknown => "?",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitStatusItem {
    pub path: String,
    pub status: GitFileStatus,
    pub staged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GitStatusResult {
    pub is_repo: bool,
    pub branch: String,
    pub tracking: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub staged_files: Vec<GitStatusItem>,
    pub unstaged_files: Vec<GitStatusItem>,
    pub untracked_files: Vec<GitStatusItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCommitInfo {
    pub hash: String,
    pub short_hash: String,
    pub author_name: String,
    pub author_email: String,
    pub relative_date: String,
    pub timestamp: i64,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCommitDetail {
    pub commit: GitCommitInfo,
    pub changed_files: Vec<String>,
    pub diff: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBranchInfo {
    pub name: String,
    pub is_current: bool,
    pub is_remote: bool,
    pub upstream: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRemote {
    pub name: String,
    pub fetch_url: String,
    pub push_url: String,
}

/// Helper to configure and execute a Git command safely
pub fn run_git_cmd(repo_dir: &Path, args: &[&str]) -> Result<String> {
    run_git_cmd_timeout(repo_dir, args, Duration::from_secs(30))
}

/// Helper to execute a Git command safely with a custom timeout
pub fn run_git_cmd_timeout(repo_dir: &Path, args: &[&str], timeout: Duration) -> Result<String> {
    run_git_cmd_internal(repo_dir, args, timeout, false)
}

/// Helper to execute a Git diff command safely with a custom timeout, treating exit code 0 and 1 as valid
pub fn run_git_diff_cmd_timeout(
    repo_dir: &Path,
    args: &[&str],
    timeout: Duration,
) -> Result<String> {
    run_git_cmd_internal(repo_dir, args, timeout, true)
}

/// Helper to execute a Git diff command safely
pub fn run_git_diff_cmd(repo_dir: &Path, args: &[&str]) -> Result<String> {
    run_git_diff_cmd_timeout(repo_dir, args, Duration::from_secs(30))
}

fn run_git_cmd_internal(
    repo_dir: &Path,
    args: &[&str],
    timeout: Duration,
    is_diff: bool,
) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    cmd.current_dir(repo_dir);
    cmd.env("GIT_TERMINAL_PROMPT", "0");

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let (tx, rx) = std::sync::mpsc::channel();
    let args_str = args.join(" ");

    std::thread::Builder::new()
        .name("git-cmd-runner".to_string())
        .spawn(move || {
            let output = cmd.output();
            let _ = tx.send(output);
        })
        .map_err(|e| anyhow!("Failed to spawn git worker thread: {}", e))?;

    match rx.recv_timeout(timeout) {
        Ok(Ok(output)) => {
            // For diff commands, exit code 0 (no diff) and exit code 1 (diff found) are both valid outcomes.
            let is_success = if is_diff {
                output.status.success()
                    || (output.status.code() == Some(1) && !output.stdout.is_empty())
            } else {
                output.status.success()
            };

            if is_success {
                Ok(String::from_utf8_lossy(&output.stdout).to_string())
            } else {
                let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let out = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let msg = if !err.is_empty() {
                    err
                } else if !out.is_empty() {
                    out
                } else {
                    format!("git exited with status {}", output.status)
                };
                Err(anyhow!(msg))
            }
        }
        Ok(Err(e)) => Err(anyhow!("Failed to execute 'git {}': {}", args_str, e)),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(anyhow!(
            "Git command timed out after {:?}: git {}",
            timeout,
            args_str
        )),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(anyhow!(
            "Git command worker thread disconnected unexpectedly"
        )),
    }
}

/// Check if Git CLI is installed and accessible in PATH
pub fn is_git_available() -> bool {
    let mut cmd = Command::new("git");
    cmd.arg("--version");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    cmd.output().map(|o| o.status.success()).unwrap_or(false)
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

mod refs;
mod status;

pub use refs::*;
pub use status::*;

#[cfg(test)]
mod tests;
