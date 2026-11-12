use crate::state::{EnvironmentState, ModalState, ScopeSettingsTarget, WorkspaceDataState};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::Icon;
use gpui_kit::{div, img, px, Entity, IntoElement, ParentElement, Styled};

use super::app_icon_image;

/// Workspace switcher pill: [app icon + workspace name ▾] with recents.
pub(super) fn render_workspace_switcher(
    ws_state: &Entity<WorkspaceDataState>,
    modal_state: &Entity<ModalState>,
    ws: &WorkspaceDataState,
    ws_name: &str,
) -> impl IntoElement {
    let t = theme();
    let ws_s = ws_state.clone();
    let modal_s = modal_state.clone();
    let recent = ws.recent_workspaces.clone();
    let ws_path = ws.workspace_path.clone();

    Button::new("btn-workspace-switcher")
        .ghost()
        .compact()
        .px(px(8.))
        .py(px(2.))
        .h(px(26.))
        .rounded(px(5.))
        .bg(t.bg_card)
        .border_1()
        .border_color(t.border)
        .child(img(app_icon_image()).w(px(16.)).h(px(16.)).rounded(px(3.)))
        .child(
            div()
                .text_xs()
                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                .text_color(t.text_primary)
                .child(ws_name.to_string()),
        )
        .child(
            Icon::new(IconName::ChevronDown)
                .size(px(12.))
                .text_color(t.text_muted),
        )
        .dropdown_menu(move |menu, _window, _cx| {
            let ws_new = ws_s.clone();
            let ws_open = ws_s.clone();
            let modal_settings = modal_s.clone();
            let ws_folder = ws_s.clone();
            let ws_clean = ws_s.clone();
            let modal_import = modal_s.clone();
            let modal_about = modal_s.clone();

            let mut m = menu
                .item(
                    PopupMenuItem::new("About Zest")
                        .icon(IconName::Info)
                        .on_click(move |_, _, cx| {
                            modal_about.update(cx, |state, cx| {
                                state.open_about_modal(cx);
                            });
                        }),
                )
                .separator()
                .item(
                    PopupMenuItem::new("Workspace Settings...")
                        .icon(IconName::Settings)
                        .on_click(move |_, _, cx| {
                            modal_settings.update(cx, |state, cx| {
                                state.open_scope_settings(ScopeSettingsTarget::Workspace, cx);
                            });
                        }),
                )
                .item(
                    PopupMenuItem::new("Open Folder")
                        .icon(IconName::FolderOpen)
                        .on_click(move |_, _, cx| {
                            ws_folder.update(cx, |state, _| {
                                state.open_workspace_folder();
                            });
                        }),
                )
                .item(
                    PopupMenuItem::new("Clear Recents")
                        .icon(IconName::Trash)
                        .on_click(move |_, _, cx| {
                            ws_clean.update(cx, |state, cx| {
                                state.clean_history(cx);
                            });
                        }),
                )
                .item(
                    PopupMenuItem::new("Import...")
                        .icon(IconName::Download)
                        .on_click(move |_, _, cx| {
                            modal_import.update(cx, |state, cx| {
                                state.open_import_modal(None, cx);
                            });
                        }),
                )
                .separator()
                .item(
                    PopupMenuItem::new("Open Workspace...")
                        .icon(IconName::Folder)
                        .on_click(move |_, _, cx| {
                            let ws_open = ws_open.clone();
                            cx.spawn(async move |cx| {
                                if let Some(folder) =
                                    rfd::AsyncFileDialog::new().pick_folder().await
                                {
                                    let folder = folder.path().to_path_buf();
                                    ws_open.update(cx, |state, cx| {
                                        state.open_workspace(folder, cx);
                                    });
                                }
                            })
                            .detach();
                        }),
                )
                .item(
                    PopupMenuItem::new("New Workspace...")
                        .icon(IconName::FolderPlus)
                        .on_click(move |_, _, cx| {
                            let ws_new = ws_new.clone();
                            cx.spawn(async move |cx| {
                                if let Some(folder) =
                                    rfd::AsyncFileDialog::new().pick_folder().await
                                {
                                    let folder = folder.path().to_path_buf();
                                    let name = folder
                                        .file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or("Workspace")
                                        .to_string();
                                    ws_new.update(cx, |state, cx| {
                                        state.new_workspace(folder, &name, cx);
                                    });
                                }
                            })
                            .detach();
                        }),
                );

            if !recent.is_empty() {
                m = m.separator();
                for r_path_str in &recent {
                    let r_path = std::path::PathBuf::from(r_path_str);
                    let display_name = r_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(r_path_str.as_str())
                        .to_string();
                    let ws_r = ws_s.clone();
                    let is_curr = r_path == ws_path;
                    let r_path_clone = r_path.clone();

                    m = m.item(
                        PopupMenuItem::new(format!("{} ({})", display_name, r_path_str))
                            .checked(is_curr)
                            .on_click(move |_, _, cx| {
                                ws_r.update(cx, |state, cx| {
                                    state.open_workspace(r_path_clone.clone(), cx);
                                });
                            }),
                    );
                }
            }

            m
        })
}

/// Environment switcher pill: [● env name ▾] with all environments.
pub(super) fn render_environment_switcher(
    env_state: &Entity<EnvironmentState>,
    modal_state: &Entity<ModalState>,
    env_read: &EnvironmentState,
    env_name: &str,
) -> impl IntoElement {
    let t = theme();
    let env_s = env_state.clone();
    let modal_settings_clone = modal_state.clone();
    let envs = env_read.environments.clone();
    let active_env_id = env_read.active_environment_id.clone();
    let has_active = env_read.active_environment().is_some();

    Button::new("btn-env-breadcrumb")
        .ghost()
        .compact()
        .px(px(8.))
        .py(px(2.))
        .h(px(26.))
        .rounded(px(5.))
        .bg(t.bg_card)
        .border_1()
        .border_color(t.border)
        .child(div().w(px(7.)).h(px(7.)).rounded_full().bg(if has_active {
            t.status_2xx
        } else {
            t.text_muted
        }))
        .child(
            div()
                .text_xs()
                .font_weight(gpui_kit::FontWeight::MEDIUM)
                .text_color(t.text_primary)
                .child(env_name.to_string()),
        )
        .child(
            Icon::new(IconName::ChevronDown)
                .size(px(12.))
                .text_color(t.text_muted),
        )
        .dropdown_menu(move |menu, _window, _cx| {
            let mut m = menu;

            // 1. "No Environment" option
            let env_none = env_s.clone();
            let is_none_active = active_env_id.is_none();
            m = m.item(
                PopupMenuItem::new("No Environment")
                    .checked(is_none_active)
                    .on_click(move |_, _, cx| {
                        env_none.update(cx, |state, cx| {
                            state.set_active_environment(None, cx);
                        });
                    }),
            );

            // 2. All workspace environments
            for env in &envs {
                let env_id = env.id.clone();
                let is_act = active_env_id.as_deref() == Some(&env_id);
                let env_item = env_s.clone();
                m = m.item(
                    PopupMenuItem::new(env.name.clone())
                        .checked(is_act)
                        .on_click(move |_, _, cx| {
                            env_item.update(cx, |state, cx| {
                                state.set_active_environment(Some(env_id.clone()), cx);
                            });
                        }),
                );
            }

            // 3. Separator and "Environment Settings..."
            let modal_settings = modal_settings_clone.clone();
            m = m.separator().item(
                PopupMenuItem::new("Environment Settings...")
                    .icon(IconName::Settings)
                    .on_click(move |_, _, cx| {
                        modal_settings.update(cx, |state, cx| {
                            state.open_env_modal(cx);
                        });
                    }),
            );

            m
        })
}
