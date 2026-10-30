mod cookies_tab;
mod general_tab;
mod network_tab;
mod shortcuts_tab;

use crate::components::ModalDialog;
use crate::state::{ModalState, SettingsTab, WorkspaceDataState};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::separator::Separator;
use gpui_kit::component::Icon;
use gpui_kit::{
    div, px, App, Entity, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement as _, Styled, Window,
};

#[derive(IntoElement)]
pub struct SettingsModal {
    workspace_state: Entity<WorkspaceDataState>,
    modal_state: Entity<ModalState>,
}

impl SettingsModal {
    pub fn new(
        workspace_state: Entity<WorkspaceDataState>,
        modal_state: Entity<ModalState>,
    ) -> Self {
        Self {
            workspace_state,
            modal_state,
        }
    }
}

impl RenderOnce for SettingsModal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();
        let ws_clone = self.workspace_state.clone();
        let modal_clone = self.modal_state.clone();

        let active_tab = modal_clone.read(cx).settings_tab;
        let cookie_count = ws_clone.read(cx).cookies.len();

        let nav_items: [(SettingsTab, IconName, &'static str, Option<usize>); 4] = [
            (
                SettingsTab::General,
                IconName::SlidersHorizontal,
                "General & Appearance",
                None,
            ),
            (
                SettingsTab::Network,
                IconName::Globe,
                "Network & Execution",
                None,
            ),
            (
                SettingsTab::Cookies,
                IconName::Cookie,
                "Cookies",
                Some(cookie_count),
            ),
            (
                SettingsTab::Shortcuts,
                IconName::Keyboard,
                "Shortcuts",
                None,
            ),
        ];

        ModalDialog::new("settings-modal")
            .w(px(740.))
            .h(px(540.))
            .title("Settings")
            .icon(IconName::Settings)
            .on_close({
                let modal_s = modal_clone.clone();
                move |_, cx| {
                    modal_s.update(cx, |state, cx| {
                        state.close_settings_modal(cx);
                    });
                }
            })
            // Main Split Body: Left Sidebar + Right Content
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .w_full()
                    .flex()
                    .overflow_hidden()
                    // Left Vertical Sidebar Navigation
                    .child(
                        div()
                            .w(px(200.))
                            .min_w(px(200.))
                            .max_w(px(200.))
                            .flex_shrink_0()
                            .h_full()
                            .bg(t.bg_sidebar)
                            .p(px(10.))
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .children(nav_items.into_iter().map(
                                |(tab, icon, label, badge_opt)| {
                                    let is_active = active_tab == tab;
                                    let modal_s = modal_clone.clone();
                                    div()
                                        .id(format!("settings-nav-{:?}", tab))
                                        .px(px(10.))
                                        .py(px(8.))
                                        .rounded(px(6.))
                                        .cursor_pointer()
                                        .bg(if is_active {
                                            t.bg_selected
                                        } else {
                                            t.transparent
                                        })
                                        .border_1()
                                        .border_color(if is_active {
                                            t.border
                                        } else {
                                            t.transparent
                                        })
                                        .hover(|s| if is_active { s } else { s.bg(t.bg_hover) })
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .gap(px(8.))
                                        .on_click(move |_, _, cx| {
                                            modal_s.update(cx, |st, cx| {
                                                st.set_settings_tab(tab, cx);
                                            });
                                        })
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(8.))
                                                .min_w_0()
                                                .child(Icon::new(icon).size(px(14.)).text_color(
                                                    if is_active {
                                                        t.primary
                                                    } else {
                                                        t.text_secondary
                                                    },
                                                ))
                                                .child(
                                                    div()
                                                        .truncate()
                                                        .text_xs()
                                                        .font_weight(if is_active {
                                                            gpui_kit::FontWeight::SEMIBOLD
                                                        } else {
                                                            gpui_kit::FontWeight::MEDIUM
                                                        })
                                                        .text_color(if is_active {
                                                            t.text_primary
                                                        } else {
                                                            t.text_secondary
                                                        })
                                                        .child(label),
                                                ),
                                        )
                                        .children(badge_opt.map(|count| {
                                            div()
                                                .flex_shrink_0()
                                                .px(px(6.))
                                                .py(px(1.))
                                                .rounded_full()
                                                .bg(if is_active { t.primary } else { t.bg_input })
                                                .text_color(if is_active {
                                                    t.primary_foreground
                                                } else {
                                                    t.text_muted
                                                })
                                                .text_size(px(10.))
                                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                .child(format!("{}", count))
                                        }))
                                },
                            )),
                    )
                    // Separator between sidebar and content
                    .child(Separator::vertical())
                    // Right Content Panel
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .h_full()
                            .overflow_hidden()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .flex_1()
                                    .w_full()
                                    .min_h(px(0.))
                                    .overflow_y_scrollbar()
                                    .p(px(20.))
                                    .child(match active_tab {
                                        SettingsTab::General => {
                                            general_tab::render_general_tab(&modal_clone, cx)
                                                .into_any_element()
                                        }
                                        SettingsTab::Network => {
                                            network_tab::render_network_tab(&modal_clone, cx)
                                                .into_any_element()
                                        }
                                        SettingsTab::Cookies => cookies_tab::render_cookies_tab(
                                            &ws_clone,
                                            &modal_clone,
                                            window,
                                            cx,
                                        )
                                        .into_any_element(),
                                        SettingsTab::Shortcuts => {
                                            shortcuts_tab::render_shortcuts_tab().into_any_element()
                                        }
                                    }),
                            ),
                    ),
            )
    }
}
