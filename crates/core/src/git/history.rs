use anyhow::{anyhow, Result};
use std::path::Path;

use super::worktree::git_stage_files;
use super::{command::run_git_cmd, repository::find_git_root};

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

/// Commit staged changes
pub fn git_commit(repo_dir: &Path, message: &str) -> Result<String> {
    let root = find_git_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("Commit message cannot be empty"));
    }
    run_git_cmd(&root, &["commit", "-m", trimmed])
}

/// Stage the given paths, then commit exactly those paths. Other files that
/// were already staged remain in the index and are not part of the commit.
pub fn git_commit_files(repo_dir: &Path, message: &str, file_paths: &[&str]) -> Result<String> {
    let root = find_git_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("Commit message cannot be empty"));
    }
    if file_paths.is_empty() {
        return Err(anyhow!("No files selected to commit"));
    }
    // Stage first so untracked paths become known to git and the working-tree
    // content of each selected path is what gets committed.
    git_stage_files(repo_dir, file_paths)?;
    let mut args = vec!["commit", "-m", trimmed, "--only", "--"];
    args.extend_from_slice(file_paths);
    run_git_cmd(&root, &args)
}

/// Retrieve recent commit history
pub fn git_history(repo_dir: &Path, limit: usize) -> Result<Vec<GitCommitInfo>> {
    let root = match find_git_root(repo_dir) {
        Some(r) => r,
        None => return Ok(Vec::new()),
    };

    let limit_str = format!("-n{}", limit);
    // Use unit separator ASCII 0x1F between fields
    let format_arg = "--format=%H%x1f%h%x1f%an%x1f%ae%x1f%cr%x1f%ct%x1f%s";
    let output = match run_git_cmd(&root, &["log", &limit_str, format_arg]) {
        Ok(out) => out,
        Err(_) => return Ok(Vec::new()), // No commits yet in repo
    };

    let mut commits = Vec::new();
    for line in output.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('\x1f').collect();
        if parts.len() >= 7 {
            commits.push(GitCommitInfo {
                hash: parts[0].to_string(),
                short_hash: parts[1].to_string(),
                author_name: parts[2].to_string(),
                author_email: parts[3].to_string(),
                relative_date: parts[4].to_string(),
                timestamp: parts[5].parse::<i64>().unwrap_or(0),
                message: parts[6].to_string(),
            });
        }
    }

    Ok(commits)
}

/// Get detailed information and diff for a commit
pub fn git_commit_detail(repo_dir: &Path, hash: &str) -> Result<GitCommitDetail> {
    let root = find_git_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());

    // 1. Get commit header info
    let format_arg = "--format=%H%x1f%h%x1f%an%x1f%ae%x1f%cr%x1f%ct%x1f%B";
    let header_out = run_git_cmd(&root, &["show", "-s", format_arg, hash])?;
    let parts: Vec<&str> = header_out.split('\x1f').collect();
    if parts.len() < 7 {
        return Err(anyhow!("Failed to parse commit header for {}", hash));
    }

    let commit = GitCommitInfo {
        hash: parts[0].to_string(),
        short_hash: parts[1].to_string(),
        author_name: parts[2].to_string(),
        author_email: parts[3].to_string(),
        relative_date: parts[4].to_string(),
        timestamp: parts[5].parse::<i64>().unwrap_or(0),
        message: parts[6].trim().to_string(),
    };

    // 2. Get changed files
    let files_out =
        run_git_cmd(&root, &["show", "--name-only", "--format=", hash]).unwrap_or_default();
    let changed_files = files_out
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    // 3. Get diff
    let diff = run_git_cmd(&root, &["show", "--patch", "--format=", hash]).unwrap_or_default();

    Ok(GitCommitDetail {
        commit,
        changed_files,
        diff,
    })
}

/// Retrieve diff for a specific commit
pub fn git_commit_diff(repo_dir: &Path, commit_hash: &str) -> Result<String> {
    let root = find_git_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());
    run_git_cmd(&root, &["show", "--patch", "--format=", commit_hash])
}
