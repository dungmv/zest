use crate::services::GitService;
use crate::state::{GitActiveModal, GitState, ToastState};
use crate::theme::theme;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, px, AnyElement, AppContext as _, Entity, InteractiveElement as _, IntoElement,
    ParentElement, StatefulInteractiveElement as _, Styled,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::spinner::Spinner;
use gpui_component::{Icon, Sizable};
use gpui_kit::assets::IconName;
use zest_core::git::GitStatusResult;

pub fn render_sidebar_footer(
    git_branch: &str,
    git_status: Option<&GitStatusResult>,
    is_syncing: bool,
    sync_message: Option<&str>,
    git: &Entity<GitState>,
    toast: &Entity<ToastState>,
) -> AnyElement {
    let t = theme();
    let is_repo = git_status.map(|s| s.is_repo).unwrap_or(false);
    let behind = git_status.map(|s| s.behind).unwrap_or(0);
    let ahead = git_status.map(|s| s.ahead).unwrap_or(0);

    let git_footer = git.clone();
    let git_service = GitService::new(git.clone(), toast.clone());
    let current_branch_name = git_branch.to_string();
    let sync_label = sync_message.unwrap_or("syncing...").to_lowercase();

    div()
        .id("sidebar-footer-bar")
        .h(px(28.))
        .w_full()
        .bg(t.bg_sidebar)
        .border_t_1()
        .border_color(t.border)
        .px(px(8.))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(6.))
        .text_xs()
        // Branch chip & dropdown menu
        .child(if is_repo {
            let git_menu = git_footer.clone();
            let svc_menu = git_service.clone();
            let active_b = current_branch_name.clone();

            Button::new("sidebar-footer-branch-chip")
                .ghost()
                .compact()
                .flex_1()
                .min_w(px(0.))
                .px(px(4.))
                .py(px(2.))
                .h(px(24.))
                .rounded(px(4.))
                .child(
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.))
                                .overflow_hidden()
                                .child(
                                    Icon::new(IconName::GitBranch)
                                        .size(px(12.))
                                        .text_color(t.primary),
                                )
                                .child(
                                    div()
                                        .text_color(t.text_secondary)
                                        .font_family(".SystemMonoFont")
                                        .max_w(px(140.))
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(git_branch.to_string()),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.))
                                .when(ahead > 0, |s| {
                                    s.child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(1.))
                                            .text_color(t.text_muted)
                                            .child(Icon::new(IconName::ArrowUp).size(px(10.)))
                                            .child(div().text_xs().child(format!("{}", ahead))),
                                    )
                                })
                                .when(behind > 0, |s| {
                                    s.child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(1.))
                                            .text_color(t.primary)
                                            .child(Icon::new(IconName::ArrowDown).size(px(10.)))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .child(format!("{}", behind)),
                                            ),
                                    )
                                })
                                .child(
                                    Icon::new(IconName::ChevronDown)
                                        .size(px(10.))
                                        .text_color(t.text_muted),
                                ),
                        ),
                )
                .dropdown_menu(move |menu, window, cx| {
                    let git_commit = git_menu.clone();
                    let git_history = git_menu.clone();
                    let git_new_branch = git_menu.clone();
                    let git_remotes = git_menu.clone();
                    let svc_checkout_parent = svc_menu.clone();
                    let svc_fetch = svc_menu.clone();
                    let svc_pull = svc_menu.clone();
                    let svc_push = svc_menu.clone();

                    let (branches_list, current_b) = {
                        let g = git_menu.read(cx);
                        let branches = g.branches.clone();
                        let current = g.branch.clone().unwrap_or_else(|| active_b.clone());
                        (branches, current)
                    };

                    let mut m = menu;

                    // 1. Commit...
                    m = m.item(
                        PopupMenuItem::new("Commit...")
                            .icon(IconName::GitCommitHorizontal)
                            .on_click(move |_, _, cx| {
                                git_commit.update(cx, |git, cx| {
                                    git.open_modal(GitActiveModal::Commit, cx);
                                });
                            }),
                    );

                    // 2. History
                    m = m.item(
                        PopupMenuItem::new("History")
                            .icon(IconName::Clock)
                            .on_click(move |_, _, cx| {
                                git_history.update(cx, |git, cx| {
                                    git.open_modal(GitActiveModal::History, cx);
                                });
                            }),
                    );

                    // 3. Branch submenu: named after the current branch (only local branches)
                    m = m.submenu(current_b.clone(), window, cx, move |sub, _w, _cx| {
                        let mut s = sub;
                        let local_branches: Vec<_> =
                            branches_list.iter().filter(|b| !b.is_remote).collect();
                        if local_branches.is_empty() {
                            s = s.item(
                                PopupMenuItem::new(if current_b.is_empty() {
                                    "No branches".to_string()
                                } else {
                                    current_b.clone()
                                })
                                .checked(true),
                            );
                        } else {
                            for b in local_branches {
                                let b_name = b.name.clone();
                                let is_curr = b.is_current || b.name == current_b;
                                let svc_b = svc_checkout_parent.clone();
                                let b_target = b_name.clone();

                                s = s.item(PopupMenuItem::new(b_name).checked(is_curr).on_click(
                                    move |_, _, cx| {
                                        if !is_curr {
                                            svc_b.checkout(&b_target, cx);
                                        }
                                    },
                                ));
                            }
                        }
                        s
                    });

                    // 4. New Branch...
                    m = m.item(
                        PopupMenuItem::new("New Branch...")
                            .icon(IconName::GitBranchPlus)
                            .on_click(move |_, _, cx| {
                                git_new_branch.update(cx, |git, cx| {
                                    git.open_modal(GitActiveModal::NewBranch, cx);
                                });
                            }),
                    );

                    m = m.separator();

                    // 5. Fetch
                    m = m.item(
                        PopupMenuItem::new("Fetch")
                            .icon(IconName::CloudDownload)
                            .disabled(is_syncing)
                            .on_click(move |_, _, cx| {
                                svc_fetch.fetch(cx);
                            }),
                    );

                    // 6. Pull
                    m = m.item(
                        PopupMenuItem::new("Pull")
                            .icon(IconName::ArrowDown)
                            .disabled(is_syncing)
                            .on_click(move |_, _, cx| {
                                svc_pull.pull(cx);
                            }),
                    );

                    // 7. Push
                    m = m.item(
                        PopupMenuItem::new("Push")
                            .icon(IconName::ArrowUp)
                            .disabled(is_syncing)
                            .on_click(move |_, _, cx| {
                                svc_push.push(cx);
                            }),
                    );

                    m = m.separator();

                    // 8. Manage Remotes...
                    m = m.item(
                        PopupMenuItem::new("Manage Remotes...")
                            .icon(IconName::Globe)
                            .on_click(move |_, _, cx| {
                                git_remotes.update(cx, |git, cx| {
                                    git.open_modal(GitActiveModal::Remotes, cx);
                                });
                            }),
                    );

                    m
                })
                .into_any_element()
        } else {
            let git_init = git_footer.clone();
            let is_checking = git_status.is_none();
            div()
                .id("sidebar-footer-no-git")
                .flex_1()
                .cursor_pointer()
                .px(px(4.))
                .py(px(2.))
                .rounded(px(4.))
                .hover(|s| s.bg(t.bg_hover))
                .flex()
                .items_center()
                .gap(px(4.))
                .on_click(move |_, _, cx| {
                    git_init.update(cx, |git, cx| {
                        git.open_modal(GitActiveModal::Commit, cx);
                    });
                })
                .child(div().text_color(t.text_muted).child(if is_checking {
                    "Checking Git..."
                } else {
                    "No Git"
                }))
                .into_any_element()
        })
        .when(is_syncing, |bar| {
            bar.child(
                div()
                    .id("sidebar-footer-sync-indicator")
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .flex_shrink_0()
                    .text_xs()
                    .child(
                        Spinner::new()
                            .icon(IconName::RefreshCw)
                            .with_size(px(12.))
                            .color(t.primary),
                    )
                    .child(div().text_xs().text_color(t.text_muted).child(sync_label)),
            )
        })
        .into_any_element()
}
