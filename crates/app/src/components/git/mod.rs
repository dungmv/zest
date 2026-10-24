pub mod branch_modal;
pub mod commit_modal;
pub mod common;
pub mod diff_view;
pub mod history_modal;
pub mod remotes_modal;

pub use branch_modal::*;
pub use commit_modal::*;
pub use common::*;
pub use diff_view::*;
pub use history_modal::*;
pub use remotes_modal::*;

use crate::services::GitService;
use crate::state::{GitActiveModal, GitState, WorkspaceDataState};
use gpui::{div, App, AppContext as _, Entity, IntoElement, RenderOnce, Window};

#[derive(IntoElement)]
pub struct GitModal {
    git_state: Entity<GitState>,
    workspace_state: Entity<WorkspaceDataState>,
    git_service: GitService,
}

impl GitModal {
    pub fn new(
        git_state: Entity<GitState>,
        workspace_state: Entity<WorkspaceDataState>,
        git_service: GitService,
    ) -> Self {
        Self {
            git_state,
            workspace_state,
            git_service,
        }
    }
}

impl RenderOnce for GitModal {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let active_modal = self.git_state.read(cx).active_modal;

        match active_modal {
            Some(GitActiveModal::Commit) => GitCommitModal::new(
                self.git_state.clone(),
                self.workspace_state.clone(),
                self.git_service.clone(),
            )
            .into_any_element(),
            Some(GitActiveModal::History) => {
                GitHistoryModal::new(self.git_state.clone(), self.git_service.clone())
                    .into_any_element()
            }
            Some(GitActiveModal::NewBranch) => {
                GitBranchModal::new(self.git_state.clone(), self.git_service.clone())
                    .into_any_element()
            }
            Some(GitActiveModal::Remotes) => {
                GitRemotesModal::new(self.git_state.clone(), self.git_service.clone())
                    .into_any_element()
            }
            None => div().into_any_element(),
        }
    }
}
