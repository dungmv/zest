use anyhow::{anyhow, Result};
use std::path::Path;

use super::{command::run_git_cmd, repository::find_git_root};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRemote {
    pub name: String,
    pub fetch_url: String,
    pub push_url: String,
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
