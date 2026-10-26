use super::request_node::SidebarReqItem;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use zest_core::models::Folder;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SidebarGitStatus {
    New,      // Untracked ('??') or Staged Added ('A') -> Green
    Modified, // Modified ('M', 'R', 'C', '!') or unsaved -> Yellow
    #[default]
    Clean, // Committed and clean -> Normal (White)
}

impl SidebarGitStatus {
    #[inline]
    pub fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Modified, _) | (_, Self::Modified) => Self::Modified,
            (Self::New, _) | (_, Self::New) => Self::New,
            _ => Self::Clean,
        }
    }
}

/// Pre-normalized index of Git file statuses for O(1) per-request lookup without
/// per-frame string allocations.
#[derive(Debug, Clone, Default)]
pub struct GitStatusIndex {
    is_repo: bool,
    by_req_id: HashMap<String, SidebarGitStatus>,
}

impl GitStatusIndex {
    pub fn from_status(git_status: Option<&zest_core::git::GitStatusResult>) -> Self {
        let Some(git) = git_status else {
            return Self::default();
        };
        if !git.is_repo {
            return Self::default();
        }
        let cap = git.untracked_files.len() + git.staged_files.len() + git.unstaged_files.len();
        if cap == 0 {
            return Self {
                is_repo: true,
                by_req_id: HashMap::new(),
            };
        }

        let mut by_req_id = HashMap::with_capacity(cap * 2);
        let mut insert_path = |raw_path: &str, status: SidebarGitStatus| {
            let norm: Cow<'_, str> = if raw_path.contains('\\') {
                Cow::Owned(raw_path.replace('\\', "/"))
            } else {
                Cow::Borrowed(raw_path)
            };
            if let Some(rest) = norm
                .strip_prefix("collections/")
                .or_else(|| norm.rsplit_once("/collections/").map(|(_, r)| r))
            {
                by_req_id.entry(rest.to_string()).or_insert(status);
            }
            by_req_id.entry(norm.into_owned()).or_insert(status);
        };

        // 1. Untracked (new file)
        for item in &git.untracked_files {
            insert_path(&item.path, SidebarGitStatus::New);
        }

        // 2. Staged
        for item in &git.staged_files {
            let st = match item.status {
                zest_core::git::GitFileStatus::Added => SidebarGitStatus::New,
                _ => SidebarGitStatus::Modified,
            };
            insert_path(&item.path, st);
        }

        // 3. Unstaged (working tree changes)
        for item in &git.unstaged_files {
            insert_path(&item.path, SidebarGitStatus::Modified);
        }

        Self {
            is_repo: true,
            by_req_id,
        }
    }

    #[inline]
    pub fn request_status(&self, req_id: &str, is_unsaved: bool) -> SidebarGitStatus {
        if is_unsaved {
            return SidebarGitStatus::Modified;
        }
        if !self.is_repo || self.by_req_id.is_empty() {
            return SidebarGitStatus::Clean;
        }
        if req_id.contains('\\') {
            let norm = req_id.replace('\\', "/");
            self.by_req_id
                .get(&norm)
                .copied()
                .unwrap_or(SidebarGitStatus::Clean)
        } else {
            self.by_req_id
                .get(req_id)
                .copied()
                .unwrap_or(SidebarGitStatus::Clean)
        }
    }
}

/// Computes git statuses for all requests and folders in a single O(F + R) pass.
/// Returns `(request_statuses_by_index, folder_statuses_by_index)`.
pub fn compute_all_git_statuses(
    folders: &[Folder],
    requests: &[SidebarReqItem],
    git_index: &GitStatusIndex,
    unsaved_requests: &HashSet<String>,
) -> (Vec<SidebarGitStatus>, Vec<SidebarGitStatus>) {
    let mut folder_statuses = vec![SidebarGitStatus::Clean; folders.len()];
    let mut req_statuses = Vec::with_capacity(requests.len());

    let folder_map: HashMap<&str, usize> = folders
        .iter()
        .enumerate()
        .map(|(i, f)| (f.id.as_str(), i))
        .collect();

    for req in requests {
        let is_unsaved = unsaved_requests.contains(&req.id);
        let st = git_index.request_status(&req.id, is_unsaved);
        req_statuses.push(st);

        if st != SidebarGitStatus::Clean {
            if let Some(fid) = req.folder_id.as_deref() {
                let mut cur = Some(fid);
                let mut steps = 0;
                while let Some(cid) = cur {
                    if let Some(&f_idx) = folder_map.get(cid) {
                        let prev = folder_statuses[f_idx];
                        if prev == SidebarGitStatus::Modified {
                            break;
                        }
                        let merged = prev.merge(st);
                        folder_statuses[f_idx] = merged;
                        cur = folders[f_idx].parent_id.as_deref();
                        steps += 1;
                        if steps > folders.len() {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
        }
    }

    (req_statuses, folder_statuses)
}

#[cfg(test)]
pub fn calculate_request_git_status(
    req_id: &str,
    git_status: Option<&zest_core::git::GitStatusResult>,
    is_unsaved: bool,
) -> SidebarGitStatus {
    if is_unsaved {
        return SidebarGitStatus::Modified;
    }
    let index = GitStatusIndex::from_status(git_status);
    index.request_status(req_id, false)
}

#[cfg(test)]
pub fn calculate_folder_git_status(
    folder_id: &str,
    folders: &[Folder],
    requests: &[SidebarReqItem],
    git_status: Option<&zest_core::git::GitStatusResult>,
    unsaved_requests: &HashSet<String>,
) -> SidebarGitStatus {
    let index = GitStatusIndex::from_status(git_status);
    let (_, folder_statuses) =
        compute_all_git_statuses(folders, requests, &index, unsaved_requests);
    folders
        .iter()
        .position(|f| f.id == folder_id)
        .map(|idx| folder_statuses[idx])
        .unwrap_or(SidebarGitStatus::Clean)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use zest_core::git::{GitFileStatus, GitStatusItem, GitStatusResult};
    use zest_core::models::HttpMethod;

    #[test]
    fn test_calculate_request_git_status_untracked_is_new() {
        let status = GitStatusResult {
            is_repo: true,
            untracked_files: vec![GitStatusItem {
                path: "collections/users/get.yaml".to_string(),
                status: GitFileStatus::Untracked,
                staged: false,
            }],
            ..Default::default()
        };

        let result = calculate_request_git_status("users/get.yaml", Some(&status), false);
        assert_eq!(result, SidebarGitStatus::New);
    }

    #[test]
    fn test_calculate_request_git_status_staged_added_is_new() {
        let status = GitStatusResult {
            is_repo: true,
            staged_files: vec![GitStatusItem {
                path: "collections/login.yaml".to_string(),
                status: GitFileStatus::Added,
                staged: true,
            }],
            ..Default::default()
        };

        let result = calculate_request_git_status("login.yaml", Some(&status), false);
        assert_eq!(result, SidebarGitStatus::New);
    }

    #[test]
    fn test_calculate_request_git_status_unstaged_modified_is_modified() {
        let status = GitStatusResult {
            is_repo: true,
            unstaged_files: vec![GitStatusItem {
                path: "collections/auth/login.yaml".to_string(),
                status: GitFileStatus::Modified,
                staged: false,
            }],
            ..Default::default()
        };

        let result = calculate_request_git_status("auth/login.yaml", Some(&status), false);
        assert_eq!(result, SidebarGitStatus::Modified);
    }

    #[test]
    fn test_calculate_request_git_status_staged_modified_is_modified() {
        let status = GitStatusResult {
            is_repo: true,
            staged_files: vec![GitStatusItem {
                path: "collections/checkout.yaml".to_string(),
                status: GitFileStatus::Modified,
                staged: true,
            }],
            ..Default::default()
        };

        let result = calculate_request_git_status("checkout.yaml", Some(&status), false);
        assert_eq!(result, SidebarGitStatus::Modified);
    }

    #[test]
    fn test_calculate_request_git_status_unsaved_is_modified() {
        let status = GitStatusResult {
            is_repo: true,
            ..Default::default()
        };

        let result = calculate_request_git_status("req1.yaml", Some(&status), true);
        assert_eq!(result, SidebarGitStatus::Modified);
    }

    #[test]
    fn test_calculate_request_git_status_clean_is_clean() {
        let status = GitStatusResult {
            is_repo: true,
            staged_files: vec![GitStatusItem {
                path: "collections/other.yaml".to_string(),
                status: GitFileStatus::Modified,
                staged: true,
            }],
            ..Default::default()
        };

        let result = calculate_request_git_status("req_clean.yaml", Some(&status), false);
        assert_eq!(result, SidebarGitStatus::Clean);
    }

    #[test]
    fn test_calculate_request_git_status_non_repo_is_clean() {
        let status = GitStatusResult {
            is_repo: false,
            ..Default::default()
        };

        let result = calculate_request_git_status("req.yaml", Some(&status), false);
        assert_eq!(result, SidebarGitStatus::Clean);
    }

    #[test]
    fn test_calculate_folder_git_status() {
        let folders = vec![
            Folder {
                id: "auth".to_string(),
                workspace_id: "ws".to_string(),
                parent_id: None,
                name: "Auth".to_string(),
                sort_order: 0,
                headers: Vec::new(),
                auth: zest_core::models::Auth::Inherit,
            },
            Folder {
                id: "auth/sub".to_string(),
                workspace_id: "ws".to_string(),
                parent_id: Some("auth".to_string()),
                name: "Sub".to_string(),
                sort_order: 0,
                headers: Vec::new(),
                auth: zest_core::models::Auth::Inherit,
            },
        ];

        let requests = vec![
            SidebarReqItem {
                id: "auth/sub/token.yaml".to_string(),
                folder_id: Some("auth/sub".to_string()),
                name: "Token".to_string(),
                display_name: "Token".to_string(),
                method: HttpMethod::Post,
                url: "".to_string(),
                last_status_code: None,
                examples: vec![],
            },
            SidebarReqItem {
                id: "auth/login.yaml".to_string(),
                folder_id: Some("auth".to_string()),
                name: "Login".to_string(),
                display_name: "Login".to_string(),
                method: HttpMethod::Post,
                url: "".to_string(),
                last_status_code: None,
                examples: vec![],
            },
        ];

        let unsaved = HashSet::new();

        // 1. When child is modified, parent folder is modified
        let status_mod = GitStatusResult {
            is_repo: true,
            unstaged_files: vec![GitStatusItem {
                path: "collections/auth/sub/token.yaml".to_string(),
                status: GitFileStatus::Modified,
                staged: false,
            }],
            ..Default::default()
        };
        assert_eq!(
            calculate_folder_git_status("auth", &folders, &requests, Some(&status_mod), &unsaved),
            SidebarGitStatus::Modified
        );

        // 2. When child is new, parent folder is new
        let status_new = GitStatusResult {
            is_repo: true,
            untracked_files: vec![GitStatusItem {
                path: "collections/auth/login.yaml".to_string(),
                status: GitFileStatus::Untracked,
                staged: false,
            }],
            ..Default::default()
        };
        assert_eq!(
            calculate_folder_git_status("auth", &folders, &requests, Some(&status_new), &unsaved),
            SidebarGitStatus::New
        );

        // 3. When all clean, folder is clean
        let status_clean = GitStatusResult {
            is_repo: true,
            ..Default::default()
        };
        assert_eq!(
            calculate_folder_git_status("auth", &folders, &requests, Some(&status_clean), &unsaved),
            SidebarGitStatus::Clean
        );
    }
}
