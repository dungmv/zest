use crate::modals::Modal;
use crate::services::GitService;
use crate::state::{ModalKind, ModalState, WorkspaceState};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Disableable as _, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
#[cfg(all(test, debug_assertions))]
use gpui_kit::InteractiveElement as _;
use gpui_kit::{
    div, px, App, AppContext as _, Entity, IntoElement, ParentElement, RenderOnce, Styled, Window,
};
use std::path::PathBuf;

#[derive(IntoElement)]
pub struct OpenGitWorkspaceModal {
    workspace: Entity<WorkspaceState>,
    modal: Entity<ModalState>,
}

impl OpenGitWorkspaceModal {
    pub fn new(workspace: Entity<WorkspaceState>, modal: Entity<ModalState>) -> Self {
        Self { workspace, modal }
    }
}

impl RenderOnce for OpenGitWorkspaceModal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();
        let inputs = window.use_keyed_state("open-git-workspace-inputs", cx, |window, cx| {
            let url = cx.new(|cx| {
                InputState::new(window, cx).placeholder("https://example.com/team/workspace.git")
            });
            let parent_folder =
                cx.new(|cx| InputState::new(window, cx).placeholder("Choose an existing folder"));
            (url, parent_folder)
        });
        let (url, parent_folder) = inputs.read(cx).clone();
        let ui = &self.modal.read(cx).open_git_workspace;
        let busy = ui.operation.is_some();
        let cancelling = ui.operation.as_ref().is_some_and(|op| op.is_cancelled());
        let error = ui.error.clone();
        let close = self.modal.clone();
        let picker_input = parent_folder.downgrade();
        let picker_modal = self.modal.clone();
        let submit_modal = self.modal.clone();

        Modal::for_kind(
            "open-git-workspace",
            ModalKind::OpenGitWorkspace,
            &self.modal,
        )
        .w(px(520.))
        .title("Open Workspace from Git")
        .icon(IconName::Download)
        .child(
            div()
                .p(px(16.))
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(5.))
                        .child(div().text_sm().child("Git clone URL"))
                        .child(Input::new(&url).disabled(busy)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(5.))
                        .child(div().text_sm().child("Parent folder"))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(Input::new(&parent_folder).disabled(busy)),
                                )
                                .child(
                                    Button::new("git-workspace-browse")
                                        .outline()
                                        .label("Browse…")
                                        .disabled(busy)
                                        .on_click(move |_, window, cx| {
                                            let input = picker_input.clone();
                                            let modal = picker_modal.clone();
                                            window
                                                .spawn(cx, async move |cx| {
                                                    if let Some(folder) =
                                                        rfd::AsyncFileDialog::new()
                                                            .pick_folder()
                                                            .await
                                                    {
                                                        let _ = cx.update(|window, cx| {
                                                            let state = modal.read(cx);
                                                            if state.active_modal
                                                                == Some(ModalKind::OpenGitWorkspace)
                                                                && state
                                                                    .open_git_workspace
                                                                    .operation
                                                                    .is_none()
                                                            {
                                                                let value = folder
                                                                    .path()
                                                                    .to_string_lossy()
                                                                    .into_owned();
                                                                let _ = input.update(
                                                                    cx,
                                                                    |state, cx| {
                                                                        state.set_value(
                                                                            value, window, cx,
                                                                        );
                                                                    },
                                                                );
                                                                cx.refresh_windows();
                                                            }
                                                        });
                                                    }
                                                })
                                                .detach();
                                        }),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(t.text_muted)
                                .child("Zest creates a subfolder named after the repository."),
                        ),
                )
                .when(busy, |content| {
                    content.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .text_sm()
                            .text_color(t.text_secondary)
                            .child(Spinner::new().with_size(px(14.)).color(t.primary))
                            .child(if cancelling {
                                "Cancelling clone…"
                            } else {
                                "Cloning repository…"
                            }),
                    )
                })
                .when_some(error, |content, error| {
                    content.child(Alert::error("git-workspace-error", error).small())
                }),
        )
        .footer(
            DialogFooter::new()
                .px(px(16.))
                .pb(px(12.))
                .child({
                    let button = Button::new("git-workspace-cancel")
                        .ghost()
                        .label(if busy { "Cancel Clone" } else { "Cancel" })
                        .disabled(cancelling)
                        .on_click(move |_, _, cx| {
                            close.update(cx, |state, cx| {
                                state.close(ModalKind::OpenGitWorkspace, cx);
                            });
                        });
                    #[cfg(all(test, debug_assertions))]
                    let button = button.debug_selector(|| "git-workspace-cancel".into());
                    button
                })
                .child({
                    let button = Button::new("git-workspace-submit")
                        .primary()
                        .label("Clone and Open")
                        .disabled(busy)
                        .on_click(move |_, _, cx| {
                            let source = url.read(cx).value().to_string();
                            let path = parent_folder.read(cx).value();
                            GitService::clone_and_open_workspace(
                                self.workspace.clone(),
                                submit_modal.clone(),
                                source,
                                PathBuf::from(path.trim()),
                                cx,
                            );
                        });
                    #[cfg(all(test, debug_assertions))]
                    let button = button.debug_selector(|| "git-workspace-submit".into());
                    button
                }),
        )
    }
}

#[cfg(all(test, debug_assertions))]
mod tests {
    use super::*;
    use gpui_kit::{Context, Modifiers, Render, TestAppContext, VisualTestContext, WindowOptions};
    use std::sync::Arc;
    use zest_core::git::CloneCancellation;
    use zest_core::storage::FsWorkspaceStore;

    struct ModalHost {
        workspace: Entity<WorkspaceState>,
        modal: Entity<ModalState>,
    }

    impl Render for ModalHost {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(OpenGitWorkspaceModal::new(
                self.workspace.clone(),
                self.modal.clone(),
            ))
        }
    }

    #[gpui_kit::test]
    fn clone_dialog_shows_validation_errors_and_cancels_a_running_operation(
        cx: &mut TestAppContext,
    ) {
        let workspace_dir = tempfile::tempdir().unwrap();
        let original_path = workspace_dir.path().to_path_buf();
        let workspace = cx.new(|_| {
            WorkspaceState::new_for_path(
                original_path.clone(),
                Arc::new(FsWorkspaceStore::new(&original_path)),
            )
        });
        let modal = cx.new(|cx| {
            let mut modal = ModalState::default();
            modal.open(ModalKind::OpenGitWorkspace, None, cx);
            modal
        });
        let window = cx.update(|cx| {
            gpui_kit::component::init(cx);
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| {
                    cx.observe(&modal, |_, _, cx| cx.notify()).detach();
                    ModalHost {
                        workspace: workspace.clone(),
                        modal: modal.clone(),
                    }
                })
            })
            .unwrap()
        });
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        cx.run_until_parked();

        let submit = cx.debug_bounds("git-workspace-submit").unwrap().center();
        cx.simulate_click(submit, Modifiers::none());
        cx.run_until_parked();
        cx.update(|_, cx| {
            let state = modal.read(cx);
            assert_eq!(state.active_modal, Some(ModalKind::OpenGitWorkspace));
            assert!(state.open_git_workspace.error.is_some());
            assert_eq!(workspace.read(cx).workspace_path, original_path);
        });

        let cancel = CloneCancellation::default();
        modal.update(&mut cx, |state, cx| {
            state.open_git_workspace.operation = Some(cancel.clone());
            cx.notify();
        });
        cx.run_until_parked();
        let button = cx.debug_bounds("git-workspace-cancel").unwrap().center();
        cx.simulate_click(button, Modifiers::none());
        assert!(cancel.is_cancelled());
    }
}
