use anyhow::Result;
use std::path::Path;

use super::*;

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
    let diff = run_git_diff_cmd(&root, &args)?;
    if !diff.trim().is_empty() {
        return Ok(diff);
    }

    // If diff is empty and this is an unstaged check for a specific file,
    // check if it is an untracked file (e.g. newly created request).
    if !staged {
        if let Some(fp) = file_path {
            let full_path = root.join(fp);
            if full_path.is_file() {
                let status_res =
                    run_git_cmd(&root, &["status", "--porcelain=v1", "-uall", "--", fp]);
                if let Ok(st_out) = status_res {
                    if st_out.trim_start().starts_with("??") {
                        let no_index_diff = run_git_diff_cmd(
                            &root,
                            &["diff", "--no-index", "--", "/dev/null", fp],
                        )?;
                        return Ok(no_index_diff);
                    }
                }
            }
        }
    }

    Ok(diff)
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

/// Discard changes in a file (restore to HEAD, optionally delete if untracked/new)
///
/// `delete_untracked` must only be `true` for explicit git-panel contexts
/// (git-clean semantics). Workspace-data callers (sidebar) must pass `false`
/// so a never-committed file is kept on disk instead of being destroyed.
pub fn git_discard_file(repo_dir: &Path, file_path: &str, delete_untracked: bool) -> Result<()> {
    let norm_path = file_path.replace('\\', "/");
    let path_obj = Path::new(&norm_path);

    // Validate path: reject empty, absolute paths, and traversal (..)
    if norm_path.is_empty()
        || norm_path.starts_with('/')
        || path_obj.is_absolute()
        || path_obj.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir
                    | std::path::Component::Prefix(_)
                    | std::path::Component::RootDir
            )
        })
    {
        return Err(anyhow!(
            "Invalid relative file path for discard: '{}'",
            file_path
        ));
    }

    let root = find_git_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());

    // Check if file exists in HEAD and/or git index
    let in_head = run_git_cmd(&root, &["cat-file", "-e", &format!("HEAD:{}", norm_path)]).is_ok();
    let in_index = run_git_cmd(&root, &["ls-files", "--error-unmatch", "--", &norm_path]).is_ok();

    if in_head {
        // Tracked file in HEAD
        // 1. Unstage if staged
        let _ = run_git_cmd(&root, &["restore", "--staged", "--", &norm_path])
            .or_else(|_| run_git_cmd(&root, &["reset", "HEAD", "--", &norm_path]));

        // 2. Restore working tree
        run_git_cmd(&root, &["restore", "--", &norm_path])
            .or_else(|_| run_git_cmd(&root, &["checkout", "HEAD", "--", &norm_path]))
            .map_err(|e| anyhow!("Failed to restore working tree for '{}': {}", norm_path, e))?;

        return Ok(());
    }

    if in_index {
        // Staged newly-added file (in index, but never in HEAD)
        // 1. Unstage so it moves to untracked
        let _ = run_git_cmd(&root, &["restore", "--staged", "--", &norm_path])
            .or_else(|_| run_git_cmd(&root, &["reset", "HEAD", "--", &norm_path]));

        if !delete_untracked {
            // Workspace-data semantics: keep untracked file on disk
            return Ok(());
        }
    } else if !delete_untracked {
        // Untracked file (never in HEAD, not in index), sidebar semantics: no-op, keep file
        return Ok(());
    }

    // Git-panel clean semantics for untracked/newly-added file: delete the file
    let full = root.join(&norm_path);
    if full.exists() {
        let clean_res = run_git_cmd(&root, &["clean", "-f", "--", &norm_path]);
        if clean_res.is_err() || full.exists() {
            if let (Ok(can_full), Ok(can_root)) = (full.canonicalize(), root.canonicalize()) {
                if can_full.starts_with(&can_root) && can_full.is_file() {
                    std::fs::remove_file(&can_full).map_err(|e| {
                        anyhow!("Failed to remove untracked file '{}': {}", norm_path, e)
                    })?;
                } else if can_full.is_dir()
                    && can_full.starts_with(&can_root)
                    && can_full != can_root
                {
                    std::fs::remove_dir_all(&can_full).map_err(|e| {
                        anyhow!(
                            "Failed to remove untracked directory '{}': {}",
                            norm_path,
                            e
                        )
                    })?;
                }
            } else if full.exists() {
                let _ = std::fs::remove_file(&full);
            }
        }
    }

    if full.exists() {
        return Err(anyhow!(
            "Untracked file '{}' still exists after discard",
            norm_path
        ));
    }

    Ok(())
}
