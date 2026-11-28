//! Git CLI integration for local workspaces.

mod branches;
mod clone;
mod command;
mod history;
mod remotes;
mod repository;
mod status;
mod worktree;

pub use branches::{
    git_branches, git_checkout, git_create_branch, git_delete_branch, GitBranchInfo,
};
pub use clone::{clone_workspace, CloneCancellation, CloneWorkspaceError};
pub use command::{
    is_git_available, run_git_cmd, run_git_cmd_timeout, run_git_diff_cmd, run_git_diff_cmd_timeout,
};
pub use history::{
    git_commit, git_commit_detail, git_commit_diff, git_commit_files, git_history, GitCommitDetail,
    GitCommitInfo,
};
pub use remotes::{
    git_add_remote, git_fetch, git_get_remotes, git_pull, git_push, git_remove_remote,
    git_set_remote_url, GitRemote,
};
pub use repository::{detect_git_repo_fast, find_git_root, git_init};
pub use status::{git_status, GitFileStatus, GitStatusItem, GitStatusResult};
pub use worktree::{
    git_diff, git_discard_file, git_stage_all, git_stage_file, git_stage_files, git_unstage_all,
    git_unstage_file, git_unstage_files,
};

#[cfg(test)]
mod tests;
