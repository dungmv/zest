use anyhow::{anyhow, Result};
use std::path::Path;

use super::*;

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

/// Retrieve local and remote branches
pub fn git_branches(repo_dir: &Path) -> Result<Vec<GitBranchInfo>> {
    let root = match find_git_root(repo_dir) {
        Some(r) => r,
        None => return Ok(Vec::new()),
    };

    let output = run_git_cmd(
        &root,
        &[
            "branch",
            "-a",
            "--format=%(HEAD)|%(refname)|%(refname:short)|%(upstream:short)",
        ],
    )?;

    let mut branches = Vec::new();
    for line in output.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() >= 3 {
            let is_current = parts[0].trim() == "*";
            let refname = parts[1].trim();
            let name = parts[2].trim().to_string();
            let upstream = parts
                .get(3)
                .map(|u| u.trim().to_string())
                .filter(|u| !u.is_empty());

            // Skip symbolic remote HEAD refs like "refs/remotes/origin/HEAD",
            // which Git shortens to just the remote name (e.g. "origin").
            if refname.starts_with("refs/remotes/") && refname.ends_with("/HEAD") {
                continue;
            }

            let is_remote = refname.starts_with("refs/remotes/");

            branches.push(GitBranchInfo {
                name,
                is_current,
                is_remote,
                upstream,
            });
        }
    }

    Ok(branches)
}

/// Checkout an existing branch
pub fn git_checkout(repo_dir: &Path, branch: &str) -> Result<()> {
    let root = find_git_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());
    run_git_cmd(&root, &["checkout", branch]).map(|_| ())
}

/// Create and checkout a new branch
pub fn git_create_branch(repo_dir: &Path, new_branch: &str) -> Result<()> {
    let root = find_git_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());
    let trimmed = new_branch.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("Branch name cannot be empty"));
    }
    run_git_cmd(&root, &["checkout", "-b", trimmed]).map(|_| ())
}

/// Delete a local branch
pub fn git_delete_branch(repo_dir: &Path, branch: &str, force: bool) -> Result<String> {
    let root = find_git_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());
    let flag = if force { "-D" } else { "-d" };
    run_git_cmd(&root, &["branch", flag, branch])
}

/// Pull changes from upstream
pub fn git_pull(repo_dir: &Path) -> Result<String> {
    let root = find_git_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());
    run_git_cmd(&root, &["pull"])
}

/// Push changes to upstream
pub fn git_push(repo_dir: &Path) -> Result<String> {
    let root = find_git_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());
    run_git_cmd(&root, &["push"])
}

/// Fetch changes from upstream remote
pub fn git_fetch(repo_dir: &Path) -> Result<String> {
    let root = find_git_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());
    run_git_cmd(&root, &["fetch"])
}

/// Initialize a new git repository
pub fn git_init(repo_dir: &Path) -> Result<()> {
    run_git_cmd(repo_dir, &["init"]).map(|_| ())
}

/// Retrieve configured remotes
pub fn git_get_remotes(repo_dir: &Path) -> Result<Vec<GitRemote>> {
    let root = find_git_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());
    let output = run_git_cmd(&root, &["remote", "-v"])?;
    let mut map: std::collections::BTreeMap<String, (String, String)> =
        std::collections::BTreeMap::new();

    for line in output.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            let name = parts[0].to_string();
            let url = parts[1].to_string();
            let kind = parts[2];
            let entry = map
                .entry(name)
                .or_insert_with(|| (String::new(), String::new()));
            if kind.contains("fetch") {
                entry.0 = url;
            } else if kind.contains("push") {
                entry.1 = url;
            }
        }
    }

    Ok(map
        .into_iter()
        .map(|(name, (fetch_url, push_url))| {
            let push_url = if push_url.is_empty() {
                fetch_url.clone()
            } else {
                push_url
            };
            GitRemote {
                name,
                fetch_url,
                push_url,
            }
        })
        .collect())
}

/// Add a new git remote
pub fn git_add_remote(repo_dir: &Path, name: &str, url: &str) -> Result<()> {
    let root = find_git_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());
    let trimmed_name = name.trim();
    let trimmed_url = url.trim();
    if trimmed_name.is_empty() || trimmed_url.is_empty() {
        return Err(anyhow!("Remote name and URL cannot be empty"));
    }
    run_git_cmd(&root, &["remote", "add", trimmed_name, trimmed_url]).map(|_| ())
}

/// Remove a git remote
pub fn git_remove_remote(repo_dir: &Path, name: &str) -> Result<()> {
    let root = find_git_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());
    let trimmed_name = name.trim();
    if trimmed_name.is_empty() {
        return Err(anyhow!("Remote name cannot be empty"));
    }
    run_git_cmd(&root, &["remote", "remove", trimmed_name]).map(|_| ())
}

/// Set the URL of an existing git remote
pub fn git_set_remote_url(repo_dir: &Path, name: &str, url: &str) -> Result<()> {
    let root = find_git_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());
    let trimmed_name = name.trim();
    let trimmed_url = url.trim();
    if trimmed_name.is_empty() || trimmed_url.is_empty() {
        return Err(anyhow!("Remote name and URL cannot be empty"));
    }
    run_git_cmd(&root, &["remote", "set-url", trimmed_name, trimmed_url]).map(|_| ())
}
