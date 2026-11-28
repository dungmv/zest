use anyhow::{anyhow, Result};
use std::path::Path;

use super::{
    command::{run_git_cmd, run_git_diff_cmd},
    repository::find_git_root,
};

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
