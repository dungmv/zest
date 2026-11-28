use anyhow::{anyhow, Result};
use std::path::Path;

use super::{command::run_git_cmd, repository::find_git_root};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBranchInfo {
    pub name: String,
    pub is_current: bool,
    pub is_remote: bool,
    pub upstream: Option<String>,
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
