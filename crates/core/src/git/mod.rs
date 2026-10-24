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
            if output.status.success() {
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

/// Parse git status --porcelain=v1 -b
pub fn git_status(repo_dir: &Path) -> Result<GitStatusResult> {
    let root = match find_git_root(repo_dir) {
        Some(r) => r,
        None => {
            return Ok(GitStatusResult {
                is_repo: false,
                branch: String::new(),
                ..Default::default()
            });
        }
    };

    let output = run_git_cmd(&root, &["status", "--porcelain=v1", "-b", "-uall"])?;
    let mut res = GitStatusResult {
        is_repo: true,
        branch: "HEAD".to_string(),
        tracking: None,
        ahead: 0,
        behind: 0,
        staged_files: Vec::new(),
        unstaged_files: Vec::new(),
        untracked_files: Vec::new(),
    };

    for line in output.lines() {
        if line.is_empty() {
            continue;
        }

        // Branch header line: ## main...origin/main [ahead 1, behind 2]
        // or: ## No commits yet on main
        // or: ## Initial commit on main
        if let Some(stripped) = line.strip_prefix("##") {
            let branch_line = stripped.trim();
            let branch_name = if let Some(b) = branch_line.strip_prefix("No commits yet on ") {
                b.trim()
            } else if let Some(b) = branch_line.strip_prefix("Initial commit on ") {
                b.trim()
            } else if let Some(branch_part) = branch_line.split("...").next() {
                branch_part.split_whitespace().next().unwrap_or("HEAD")
            } else {
                "HEAD"
            };
            res.branch = branch_name.to_string();

            if branch_line.contains("...") {
                let after_dots = branch_line.split("...").nth(1).unwrap_or("");
                let tracking = after_dots.split_whitespace().next().unwrap_or("");
                if !tracking.is_empty() {
                    res.tracking = Some(tracking.to_string());
                }

                if let Some(bracket_start) = branch_line.find('[') {
                    if let Some(bracket_end) = branch_line.find(']') {
                        let info = &branch_line[bracket_start + 1..bracket_end];
                        for part in info.split(',') {
                            let part = part.trim();
                            if let Some(ahead_str) = part.strip_prefix("ahead ") {
                                if let Ok(n) = ahead_str.trim().parse::<usize>() {
                                    res.ahead = n;
                                }
                            } else if let Some(behind_str) = part.strip_prefix("behind ") {
                                if let Ok(n) = behind_str.trim().parse::<usize>() {
                                    res.behind = n;
                                }
                            }
                        }
                    }
                }
            }
            continue;
        }

        // Porcelain format: XY PATH
        // X = index status, Y = working tree status
        if line.len() < 3 {
            continue;
        }

        let chars: Vec<char> = line.chars().collect();
        let index_code = chars[0];
        let work_code = chars[1];
        let path = line[3..].trim().to_string();

        // Untracked
        if index_code == '?' && work_code == '?' {
            res.untracked_files.push(GitStatusItem {
                path,
                status: GitFileStatus::Untracked,
                staged: false,
            });
            continue;
        }

        // Staged changes (index)
        if index_code != ' ' && index_code != '?' {
            let status = match index_code {
                'M' => GitFileStatus::Modified,
                'A' => GitFileStatus::Added,
                'D' => GitFileStatus::Deleted,
                'R' => GitFileStatus::Renamed,
                'C' => GitFileStatus::Copied,
                'U' => GitFileStatus::Conflicted,
                _ => GitFileStatus::Unknown,
            };
            res.staged_files.push(GitStatusItem {
                path: path.clone(),
                status,
                staged: true,
            });
        }

        // Working tree changes (unstaged)
        if work_code != ' ' && work_code != '?' {
            let status = match work_code {
                'M' => GitFileStatus::Modified,
                'A' => GitFileStatus::Added,
                'D' => GitFileStatus::Deleted,
                'R' => GitFileStatus::Renamed,
                'C' => GitFileStatus::Copied,
                'U' => GitFileStatus::Conflicted,
                _ => GitFileStatus::Unknown,
            };
            res.unstaged_files.push(GitStatusItem {
                path,
                status,
                staged: false,
            });
        }
    }

    Ok(res)
}

/// Retrieve diff for working tree or staged changes
pub fn git_diff(repo_dir: &Path, file_path: Option<&str>, staged: bool) -> Result<String> {
    let root = find_git_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());
    let mut args = vec!["diff"];
    if staged {
        args.push("--cached");
    }
    if let Some(fp) = file_path {
        args.push("--");
        args.push(fp);
    }
    run_git_cmd(&root, &args)
}

/// Retrieve diff for a specific commit
pub fn git_commit_diff(repo_dir: &Path, commit_hash: &str) -> Result<String> {
    let root = find_git_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());
    run_git_cmd(&root, &["show", "--patch", "--format=", commit_hash])
}

/// Stage a specific file
pub fn git_stage_file(repo_dir: &Path, file_path: &str) -> Result<()> {
    let root = find_git_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());
    run_git_cmd(&root, &["add", "--", file_path]).map(|_| ())
}

/// Unstage a specific file
pub fn git_unstage_file(repo_dir: &Path, file_path: &str) -> Result<()> {
    let root = find_git_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());
    run_git_cmd(&root, &["restore", "--staged", "--", file_path])
        .or_else(|_| run_git_cmd(&root, &["reset", "HEAD", "--", file_path]))
        .map(|_| ())
}

/// Stage multiple files in a single git command
pub fn git_stage_files(repo_dir: &Path, file_paths: &[&str]) -> Result<()> {
    if file_paths.is_empty() {
        return Ok(());
    }
    let root = find_git_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());
    let mut args = vec!["add", "--"];
    args.extend_from_slice(file_paths);
    run_git_cmd(&root, &args).map(|_| ())
}

/// Unstage multiple files in a single git command
pub fn git_unstage_files(repo_dir: &Path, file_paths: &[&str]) -> Result<()> {
    if file_paths.is_empty() {
        return Ok(());
    }
    let root = find_git_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());
    let mut args = vec!["restore", "--staged", "--"];
    args.extend_from_slice(file_paths);
    run_git_cmd(&root, &args)
        .or_else(|_| {
            let mut reset_args = vec!["reset", "HEAD", "--"];
            reset_args.extend_from_slice(file_paths);
            run_git_cmd(&root, &reset_args)
        })
        .map(|_| ())
}

/// Stage all modified and untracked files
pub fn git_stage_all(repo_dir: &Path) -> Result<()> {
    let root = find_git_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());
    run_git_cmd(&root, &["add", "-A"]).map(|_| ())
}

/// Unstage all staged files
pub fn git_unstage_all(repo_dir: &Path) -> Result<()> {
    let root = find_git_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());
    run_git_cmd(&root, &["restore", "--staged", "."])
        .or_else(|_| run_git_cmd(&root, &["reset"]))
        .map(|_| ())
}

/// Discard changes in a file (restore to HEAD or delete if untracked/new)
pub fn git_discard_file(repo_dir: &Path, file_path: &str) -> Result<()> {
    let root = find_git_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());
    let norm_path = file_path.replace('\\', "/");

    // 1. Unstage if staged
    let _ = run_git_cmd(&root, &["restore", "--staged", "--", &norm_path]);

    // 2. Restore working tree
    let res = run_git_cmd(&root, &["restore", "--", &norm_path])
        .or_else(|_| run_git_cmd(&root, &["checkout", "--", &norm_path]));

    let full = root.join(file_path);

    // 3. Check if file existed in HEAD
    let in_head = run_git_cmd(&root, &["cat-file", "-e", &format!("HEAD:{}", norm_path)]).is_ok();

    if !in_head || res.is_err() {
        // File never existed in HEAD, so it was a newly added or untracked file.
        // First try git clean to remove safely via git
        if run_git_cmd(&root, &["clean", "-f", "--", &norm_path]).is_err() {
            // Fallback: only delete if canonical path is verified to reside strictly within root
            if let (Ok(can_full), Ok(can_root)) = (full.canonicalize(), root.canonicalize()) {
                if can_full.starts_with(&can_root) && can_full.is_file() {
                    let _ = std::fs::remove_file(can_full);
                }
            }
        }
    }

    Ok(())
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
            "--format=%(HEAD)|%(refname:short)|%(upstream:short)",
        ],
    )?;

    let mut branches = Vec::new();
    for line in output.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() >= 2 {
            let is_current = parts[0].trim() == "*";
            let name = parts[1].trim().to_string();
            let upstream = parts
                .get(2)
                .map(|u| u.trim().to_string())
                .filter(|u| !u.is_empty());
            let is_remote = name.starts_with("origin/") || name.starts_with("remotes/");

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_availability() {
        assert!(is_git_available());
    }

    #[test]
    fn test_git_status_in_current_repo() {
        let repo_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = find_git_root(repo_dir);
        assert!(root.is_some());

        let status = git_status(repo_dir).unwrap();
        assert!(status.is_repo);
        assert!(!status.branch.is_empty());
    }

    #[test]
    fn test_git_history_in_current_repo() {
        let repo_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let history = git_history(repo_dir, 5).unwrap();
        assert!(!history.is_empty());
        assert!(!history[0].hash.is_empty());
        assert!(!history[0].short_hash.is_empty());
    }

    #[test]
    fn test_git_branches_in_current_repo() {
        let repo_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let branches = git_branches(repo_dir).unwrap();
        assert!(!branches.is_empty());
        assert!(branches.iter().any(|b| b.is_current));
    }

    #[test]
    fn test_git_operations_lifecycle() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_git_lifecycle_{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // 1. init
        assert!(git_init(&temp_dir).is_ok());
        // Configure author for commit in test environment
        let _ = run_git_cmd(&temp_dir, &["config", "user.name", "Test Runner"]);
        let _ = run_git_cmd(&temp_dir, &["config", "user.email", "test@example.com"]);

        // 2. create file
        let file_path = temp_dir.join("request.yaml");
        std::fs::write(&file_path, "name: Test Request\nmethod: GET\n").unwrap();

        // 3. status - untracked
        let status = git_status(&temp_dir).unwrap();
        assert!(status.is_repo);
        assert_eq!(status.untracked_files.len(), 1);
        assert_eq!(status.untracked_files[0].path, "request.yaml");

        // 4. stage file
        assert!(git_stage_file(&temp_dir, "request.yaml").is_ok());
        let status = git_status(&temp_dir).unwrap();
        assert_eq!(status.staged_files.len(), 1);
        assert_eq!(status.staged_files[0].path, "request.yaml");

        // 5. unstage file
        assert!(git_unstage_file(&temp_dir, "request.yaml").is_ok());
        let status = git_status(&temp_dir).unwrap();
        assert_eq!(status.staged_files.len(), 0);

        // 5b. batch stage and unstage files
        assert!(git_stage_files(&temp_dir, &["request.yaml"]).is_ok());
        let status = git_status(&temp_dir).unwrap();
        assert_eq!(status.staged_files.len(), 1);
        assert!(git_unstage_files(&temp_dir, &["request.yaml"]).is_ok());
        let status = git_status(&temp_dir).unwrap();
        assert_eq!(status.staged_files.len(), 0);

        // 6. stage all and commit
        assert!(git_stage_all(&temp_dir).is_ok());
        assert!(git_commit(&temp_dir, "feat: initial test request").is_ok());

        // 7. history
        let history = git_history(&temp_dir, 10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].message, "feat: initial test request");

        // 7b. commit only selected files (untracked path is staged implicitly)
        std::fs::write(temp_dir.join("extra.yaml"), "name: Extra\n").unwrap();
        std::fs::write(&file_path, "name: Test Request\nmethod: DELETE\n").unwrap();
        assert!(git_commit_files(&temp_dir, "feat: add extra request", &["extra.yaml"]).is_ok());
        let status = git_status(&temp_dir).unwrap();
        assert!(status
            .unstaged_files
            .iter()
            .any(|f| f.path == "request.yaml"));
        assert!(!status
            .staged_files
            .iter()
            .chain(&status.unstaged_files)
            .chain(&status.untracked_files)
            .any(|f| f.path == "extra.yaml"));
        let history = git_history(&temp_dir, 10).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].message, "feat: add extra request");

        // 8. modify file and check diff
        std::fs::write(&file_path, "name: Test Request\nmethod: POST\n").unwrap();
        let diff = git_diff(&temp_dir, Some("request.yaml"), false).unwrap();
        assert!(diff.contains("-method: GET"));
        assert!(diff.contains("+method: POST"));

        // 9. branch and checkout
        assert!(git_create_branch(&temp_dir, "feature-branch").is_ok());
        let branches = git_branches(&temp_dir).unwrap();
        assert!(branches
            .iter()
            .any(|b| b.name == "feature-branch" && b.is_current));

        // 10. discard modified file
        assert!(git_discard_file(&temp_dir, "request.yaml").is_ok());
        let content_after_discard = std::fs::read_to_string(&file_path).unwrap();
        assert!(content_after_discard.contains("method: GET"));

        // 11. discard untracked new file
        let new_untracked = temp_dir.join("new_req.yaml");
        std::fs::write(&new_untracked, "name: Untracked\n").unwrap();
        assert!(new_untracked.is_file());
        assert!(git_discard_file(&temp_dir, "new_req.yaml").is_ok());
        assert!(!new_untracked.exists());

        // 12. remote management
        assert!(git_add_remote(&temp_dir, "origin", "https://github.com/test/repo.git").is_ok());
        let remotes = git_get_remotes(&temp_dir).unwrap();
        assert_eq!(remotes.len(), 1);
        assert_eq!(remotes[0].name, "origin");
        assert_eq!(remotes[0].fetch_url, "https://github.com/test/repo.git");

        assert!(
            git_set_remote_url(&temp_dir, "origin", "https://github.com/test/new-repo.git").is_ok()
        );
        let remotes = git_get_remotes(&temp_dir).unwrap();
        assert_eq!(remotes[0].fetch_url, "https://github.com/test/new-repo.git");

        assert!(git_remove_remote(&temp_dir, "origin").is_ok());
        let remotes = git_get_remotes(&temp_dir).unwrap();
        assert!(remotes.is_empty());

        // 13. detect_git_repo_fast
        let fast = detect_git_repo_fast(&temp_dir);
        assert!(fast.is_some());
        let (root, branch) = fast.unwrap();
        assert_eq!(root, temp_dir);
        assert!(branch.is_some());

        // Non-repo path
        let non_repo = std::env::temp_dir().join(format!("non_repo_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&non_repo);
        assert!(detect_git_repo_fast(&non_repo).is_none());
        let _ = std::fs::remove_dir_all(&non_repo);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
