use anyhow::Result;
use std::path::Path;

use super::{command::run_git_cmd, repository::find_git_root};

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
