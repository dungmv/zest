use crate::state::{
    EnvironmentState, ModalState, RequestSessionState, ResponseState, ScopeSettingsTarget,
    SidebarState, WorkspaceDataState,
};
use crate::theme::{hex_to_hsla, theme};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, img, px, App, Context, Entity, Image, ImageFormat, InteractiveElement as _, IntoElement,
    MouseButton, ParentElement, Render, RenderOnce, StatefulInteractiveElement as _, Styled,
    Window, WindowControlArea,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::{Icon, InteractiveElementExt as _};
use gpui_kit::assets::IconName;
use std::sync::{Arc, OnceLock};

struct TitleBarState {
    should_move: bool,
}

impl Render for TitleBarState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn app_icon_image() -> Arc<Image> {
    static ICON: OnceLock<Arc<Image>> = OnceLock::new();
    ICON.get_or_init(|| {
        Arc::new(Image {
            format: ImageFormat::Png,
            bytes: include_bytes!("../../resources/app_icon_64.png").to_vec(),
            id: 1,
        })
    })
    .clone()
}

#[derive(IntoElement)]
pub struct AppTitleBar {
    workspace_state: Entity<WorkspaceDataState>,
    env_state: Entity<EnvironmentState>,
    modal_state: Entity<ModalState>,
    sidebar_state: Entity<SidebarState>,
    editor_state: Entity<RequestSessionState>,
    response_state: Entity<ResponseState>,
}

impl AppTitleBar {
    pub fn new(
        workspace_state: Entity<WorkspaceDataState>,
        env_state: Entity<EnvironmentState>,
        modal_state: Entity<ModalState>,
        sidebar_state: Entity<SidebarState>,
        editor_state: Entity<RequestSessionState>,
        response_state: Entity<ResponseState>,
    ) -> Self {
        Self {
            workspace_state,
            env_state,
            modal_state,
            sidebar_state,
            editor_state,
            response_state,
        }
    }
}

impl RenderOnce for AppTitleBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_state(cx, |_, _| TitleBarState { should_move: false });
        let is_macos = cfg!(target_os = "macos");
        let is_linux = cfg!(target_os = "linux");
        let t = theme();
        let ws = self.workspace_state.read(cx);
        let env_read = self.env_state.read(cx);
        let modal = self.modal_state.read(cx);
        let ed = self.editor_state.read(cx);
        let resp = self.response_state.read(cx);
        let sidebar_collapsed = self.sidebar_state.read(cx).sidebar_collapsed;

        let folder_name = ws
            .workspace_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Workspace")
            .to_string();

        let ws_name = ws
            .active_workspace
            .as_ref()
            .map(|w| {
                if !w.name.trim().is_empty() {
                    w.name.clone()
                } else {
                    folder_name.clone()
                }
            })
            .unwrap_or(folder_name);

        let env_name = env_read
            .active_environment()
            .map(|e| e.name.clone())
            .unwrap_or_else(|| "No Environment".to_string());

        let active_id = ed.active_request_id();
        let req_title = ws
            .find_request(active_id)
            .map(crate::state::effective_request_name)
            .unwrap_or_default();

        let current_title = if let Some(ex_name) = &resp.active_example_name {
            if !req_title.trim().is_empty() {
                format!("{}: {}", req_title, ex_name)
            } else {
                ex_name.clone()
            }
        } else {
            req_title
        };

        let ws_clone = self.workspace_state.clone();
        let modal_clone = self.modal_state.clone();
        let sb_clone = self.sidebar_state.clone();

        #[cfg(target_os = "macos")]
        let left_padding = px(78.);
        #[cfg(not(target_os = "macos"))]
        let left_padding = px(8.);

        let bar = div()
            .id("app-title-bar")
            .h(px(38.))
            .w_full()
            .bg(t.bg_titlebar)
            .border_b_1()
            .border_color(t.border)
            .flex()
            .items_center()
            .justify_between()
            .overflow_hidden()
            .pl(left_padding)
            .when(is_linux, |this| {
                this.on_double_click(|_, window, _| window.zoom_window())
            })
            .when(is_macos, |this| {
                this.on_double_click(|_, window, _| window.titlebar_double_click())
            })
            .on_mouse_down_out(window.listener_for(&state, |state, _, _, _| {
                state.should_move = false;
            }))
            .on_mouse_down(
                MouseButton::Left,
                window.listener_for(&state, |state, _, _, _| {
                    state.should_move = true;
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                window.listener_for(&state, |state, _, _, _| {
                    state.should_move = false;
                }),
            )
            .on_mouse_move(window.listener_for(&state, |state, _, window, _| {
                if state.should_move {
                    state.should_move = false;
                    window.start_window_move();
                }
            }));

        // Left section
        let left_section = div()
            .flex()
            .items_center()
            .flex_shrink_0()
            .gap(px(8.))
            // Sidebar toggle button
            .child({
                let sb_s = sb_clone.clone();
                div()
                    .id("btn-toggle-sidebar")
                    .occlude()
                    .w(px(28.))
                    .h(px(26.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .bg(if !sidebar_collapsed {
                        t.bg_hover
                    } else {
                        gpui::transparent_black()
                    })
                    .hover(|s| s.bg(t.bg_hover))
                    .text_color(if !sidebar_collapsed {
                        t.text_primary
                    } else {
                        t.text_secondary
                    })
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(move |_, _, cx| {
                        sb_s.update(cx, |state, cx| {
                            state.toggle_sidebar(cx);
                        });
                    })
                    .child(Icon::new(IconName::PanelLeft).size(px(16.)))
            })
            // Workspace switcher pill dropdown: [icon + ws_name ▾]
            .child({
                let ws_s = ws_clone.clone();
                let modal_s = modal_clone.clone();
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
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(t.text_primary)
                            .child(ws_name.clone()),
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

                        let mut m = menu
                            .item(
                                PopupMenuItem::new("Workspace Settings...")
                                    .icon(IconName::Settings)
                                    .on_click(move |_, _, cx| {
                                        modal_settings.update(cx, |state, cx| {
                                            state.open_scope_settings(
                                                ScopeSettingsTarget::Workspace,
                                                cx,
                                            );
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
                                PopupMenuItem::new("Clean History")
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
                                    PopupMenuItem::new(format!(
                                        "{} ({})",
                                        display_name, r_path_str
                                    ))
                                    .checked(is_curr)
                                    .on_click(
                                        move |_, _, cx| {
                                            ws_r.update(cx, |state, cx| {
                                                state.open_workspace(r_path_clone.clone(), cx);
                                            });
                                        },
                                    ),
                                );
                            }
                        }

                        m
                    })
            })
            // Environment pill dropdown: [● local ▾]
            .child({
                let env_s = self.env_state.clone();
                let modal_settings_clone = modal_clone.clone();
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
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(t.text_primary)
                            .child(env_name),
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
            });

        // Center section: search button + current request name
        let modal_center_clone = modal_clone.clone();
        let center_section = div()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .px(px(8.))
            .overflow_hidden()
            .flex()
            .items_center()
            .justify_center()
            .child({
                let _ws_s = ws_clone.clone();
                let has_title = !current_title.trim().is_empty();
                let label = if has_title {
                    current_title.clone()
                } else {
                    "Search requests, commands...".to_string()
                };

                div()
                    .id("btn-titlebar-search")
                    .occlude()
                    .h(px(26.))
                    .w(px(320.))
                    .max_w(px(420.))
                    .px(px(8.))
                    .rounded(px(5.))
                    .bg(t.bg_card)
                    .border_1()
                    .border_color(t.border)
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(6.))
                    .cursor_pointer()
                    .hover(|s| s.bg(t.bg_hover).border_color(t.border_focus))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(move |_, _, cx| {
                        let modal_cp = modal_center_clone.clone();
                        modal_cp.update(cx, |state, cx| {
                            state.toggle_command_palette(cx);
                        });
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .min_w(px(0.))
                            .flex_1()
                            .overflow_hidden()
                            .child(
                                div()
                                    .truncate()
                                    .text_xs()
                                    .font_family(".SystemMonoFont")
                                    .text_color(if has_title {
                                        t.text_primary
                                    } else {
                                        t.text_muted
                                    })
                                    .child(label),
                            ),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .px(px(4.))
                            .py(px(1.))
                            .rounded(px(3.))
                            .bg(t.bg_input)
                            .border_1()
                            .border_color(t.border)
                            .text_color(t.text_muted)
                            .font_family(".SystemMonoFont")
                            .text_size(px(10.))
                            .child(if is_macos { "⌘K" } else { "Ctrl+K" }),
                    )
            });

        // Right toolbar buttons: split toggle, settings
        let right_toolbar = div()
            .flex()
            .items_center()
            .flex_shrink_0()
            .gap(px(4.))
            .pr(px(8.))
            // Split layout toggle button
            .child({
                let modal_s = modal_clone.clone();
                let is_vert = modal.split_layout_vertical;
                div()
                    .id("btn-split-toggle")
                    .occlude()
                    .w(px(28.))
                    .h(px(26.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .hover(|s| s.bg(t.bg_hover))
                    .text_color(t.text_secondary)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(move |_, _, cx| {
                        modal_s.update(cx, |state, cx| state.toggle_split_layout(cx))
                    })
                    .child(if is_vert {
                        Icon::new(IconName::PanelBottom).size(px(16.))
                    } else {
                        Icon::new(IconName::PanelRight).size(px(16.))
                    })
            })
            // Settings button
            .child({
                let modal_s = modal_clone.clone();
                div()
                    .id("btn-settings")
                    .occlude()
                    .w(px(28.))
                    .h(px(26.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .hover(|s| s.bg(t.bg_hover))
                    .text_color(t.text_secondary)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(move |_, _, cx| {
                        modal_s.update(cx, |state, cx| state.toggle_settings_modal(cx))
                    })
                    .child(Icon::new(IconName::Settings).size(px(16.)))
            });

        let drag_region = div()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .overflow_hidden()
            .flex()
            .items_center()
            .justify_between()
            .window_control_area(WindowControlArea::Drag)
            .child(left_section)
            .child(center_section)
            .child(right_toolbar);

        #[allow(unused_mut)]
        let mut bar = bar.child(drag_region);

        #[cfg(not(target_os = "macos"))]
        {
            let is_max = window.is_maximized();
            let icon_color = t.text_primary;

            bar = bar.child(
                div()
                    .h_full()
                    .flex()
                    .items_center()
                    .flex_shrink_0()
                    .child(
                        div()
                            .id("caption-min")
                            .occlude()
                            .w(px(46.))
                            .h_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .window_control_area(WindowControlArea::Min)
                            .hover(|s| s.bg(t.bg_hover))
                            .text_color(icon_color)
                            .on_click(|_, window, cx| {
                                cx.stop_propagation();
                                window.minimize_window();
                            })
                            .child(Icon::new(IconName::WindowMinimize).size(px(16.))),
                    )
                    .child(
                        div()
                            .id("caption-max")
                            .occlude()
                            .w(px(46.))
                            .h_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .window_control_area(WindowControlArea::Max)
                            .hover(|s| s.bg(t.bg_hover))
                            .text_color(icon_color)
                            .on_click(|_, window, cx| {
                                cx.stop_propagation();
                                window.zoom_window();
                            })
                            .child(if is_max {
                                Icon::new(IconName::WindowRestore).size(px(16.))
                            } else {
                                Icon::new(IconName::WindowMaximize).size(px(16.))
                            }),
                    )
                    .child(
                        div()
                            .id("caption-close")
                            .occlude()
                            .w(px(46.))
                            .h_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .window_control_area(WindowControlArea::Close)
                            .hover(|s| {
                                s.bg(hex_to_hsla(0xe81123))
                                    .text_color(hex_to_hsla(0xffffff))
                            })
                            .text_color(icon_color)
                            .on_click(|_, window, cx| {
                                cx.stop_propagation();
                                window.remove_window();
                                cx.quit();
                            })
                            .child(Icon::new(IconName::WindowClose).size(px(16.))),
                    ),
            );
        }

        bar
    }
}
