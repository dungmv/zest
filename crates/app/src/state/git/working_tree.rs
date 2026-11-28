use std::collections::HashMap;
use zest_core::git::{GitFileStatus, GitStatusResult};

/// One row per path, in Git status order. Worktree changes take precedence over
/// staged changes for the badge and diff mode.
#[derive(Debug, Clone)]
pub struct UnifiedChangeItem {
    pub path: String,
    pub status: GitFileStatus,
    pub has_worktree_changes: bool,
}

#[derive(Debug, Clone, Default)]
pub(super) struct GitChanges {
    pub(super) rows: Vec<UnifiedChangeItem>,
    pub(super) by_path: HashMap<String, usize>,
}

impl GitChanges {
    pub(super) fn from_status(status: &GitStatusResult) -> Self {
        let mut changes = Self::default();
        let entries = status.staged_files.iter().map(|file| (file, false)).chain(
            status
                .unstaged_files
                .iter()
                .chain(&status.untracked_files)
                .map(|file| (file, true)),
        );

        for (file, has_worktree_changes) in entries {
            if let Some(&index) = changes.by_path.get(&file.path) {
                changes.rows[index].status = file.status;
                changes.rows[index].has_worktree_changes = has_worktree_changes;
            } else {
                changes
                    .by_path
                    .insert(file.path.clone(), changes.rows.len());
                changes.rows.push(UnifiedChangeItem {
                    path: file.path.clone(),
                    status: file.status,
                    has_worktree_changes,
                });
            }
        }

        changes
    }

    pub(super) fn get(&self, path: &str) -> Option<&UnifiedChangeItem> {
        self.by_path.get(path).map(|&index| &self.rows[index])
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct GitWorkingTree {
    pub(super) branch: Option<String>,
    pub(super) status: Option<GitStatusResult>,
    pub(super) changes: GitChanges,
    pub(super) version: usize,
}

impl GitWorkingTree {
    pub(super) fn refresh(&mut self, status: GitStatusResult, changes: GitChanges) {
        self.branch = (status.is_repo && !status.branch.is_empty()).then(|| status.branch.clone());
        self.status = Some(status);
        self.changes = changes;
        self.version = self.version.wrapping_add(1);
    }

    pub(super) fn reset(&self) -> Self {
        Self {
            version: self.version.wrapping_add(1),
            ..Self::default()
        }
    }
}
