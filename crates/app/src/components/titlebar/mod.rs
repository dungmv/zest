mod breadcrumb;
mod menus;

use crate::state::{
    EnvironmentState, ModalState, RequestSessionState, ResponseState, SidebarState,
    WorkspaceDataState,
};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Icon, InteractiveElementExt as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, App, Context, Entity, Image, ImageFormat, InteractiveElement as _, IntoElement,
    MouseButton, ParentElement, Render, RenderOnce, StatefulInteractiveElement as _, Styled,
    Window, WindowControlArea,
};
use std::sync::{Arc, OnceLock};

struct TitleBarState {
    should_move: bool,
}

impl Render for TitleBarState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

pub(super) fn app_icon_image() -> Arc<Image> {
    static ICON: OnceLock<Arc<Image>> = OnceLock::new();
    ICON.get_or_init(|| {
        Arc::new(Image {
            format: ImageFormat::Png,
            bytes: include_bytes!("../../../resources/app_icon_64.png").to_vec(),
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
        let is_linux = cfg!(target_os = "linux");
        let is_macos = cfg!(target_os = "macos");
        let t = theme();
        let ws = self.workspace_state.read(cx);
        let env_read = self.env_state.read(cx);
        let modal = self.modal_state.read(cx);
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

        let active_id = self.editor_state.read(cx).active_request_id();

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
            .id("titlebar-left-section")
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
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
                        gpui_kit::transparent_black()
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
            .child(menus::render_workspace_switcher(
                &ws_clone,
                &modal_clone,
                ws,
                &ws_name,
            ))
            // Environment pill dropdown: [● local ▾]
            .child(menus::render_environment_switcher(
                &self.env_state,
                &modal_clone,
                env_read,
                &env_name,
            ));

        // Center section: search button + current request name
        let center_section = breadcrumb::render_center_section(
            ws,
            active_id,
            self.response_state.read(cx).active_example_name.as_ref(),
            &modal_clone,
        );

        // Right toolbar buttons: split toggle, settings
        let right_toolbar = div()
            .id("titlebar-right-toolbar")
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .flex()
            .items_center()
            .flex_shrink_0()
            .gap(px(4.))
            .pr(px(8.))
            // Split layout toggle button
            .child({
                let modal_s = modal_clone.clone();
                let is_vert = modal.split_layout_vertical;
                Button::new("btn-split-toggle")
                    .ghost()
                    .compact()
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .w(px(28.))
                    .h(px(26.))
                    .tooltip(if is_vert {
                        "Split Layout: Stacked (Vertical)"
                    } else {
                        "Split Layout: Side-by-Side (Horizontal)"
                    })
                    .on_click(move |_, _, cx| {
                        modal_s.update(cx, |state, cx| state.toggle_split_layout(cx));
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
                Button::new("btn-settings")
                    .ghost()
                    .compact()
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .w(px(28.))
                    .h(px(26.))
                    .on_click(move |_, _, cx| {
                        modal_s.update(cx, |state, cx| state.toggle_settings_modal(cx));
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
            bar = bar.child(render_caption_buttons(is_max));
        }

        bar
    }
}

#[cfg(not(target_os = "macos"))]
fn render_caption_buttons(is_max: bool) -> impl IntoElement {
    use crate::theme::hex_to_hsla;

    let t = theme();
    let icon_color = t.text_primary;

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
        )
}
