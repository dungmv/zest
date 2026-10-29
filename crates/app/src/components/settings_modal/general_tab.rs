use crate::state::ModalState;
use crate::theme::{theme, ThemePreference};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::Icon;
use gpui_kit::{div, px, App, Entity, IntoElement, ParentElement, Styled};

pub fn render_general_tab(modal_state: &Entity<ModalState>, cx: &App) -> impl IntoElement {
    let t = theme();
    let cur_pref = modal_state.read(cx).theme_preference;
    let is_vertical_split = modal_state.read(cx).split_layout_vertical;

    let modal_clone = modal_state.clone();

    let (theme_icon, theme_label) = match cur_pref {
        ThemePreference::System => (IconName::Monitor, "System"),
        ThemePreference::Dark => (IconName::Moon, "Dark"),
        ThemePreference::Light => (IconName::Sun, "Light"),
    };

    let (layout_icon, layout_label) = if is_vertical_split {
        (IconName::PanelBottom, "Vertical Split")
    } else {
        (IconName::PanelRight, "Horizontal Split")
    };

    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(20.))
        // Section: Appearance & Layout
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::BOLD)
                        .text_color(t.text_secondary)
                        .child("APPEARANCE & LAYOUT"),
                )
                // Row 1: Theme Mode Dropdown
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .p(px(12.))
                        .rounded(px(6.))
                        .bg(t.bg_app)
                        .border_1()
                        .border_color(t.border)
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(16.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(3.))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                        .text_color(t.text_primary)
                                        .child("Theme Mode"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(t.text_muted)
                                        .child("Follow macOS system appearance or stay in Dark / Light mode."),
                                ),
                        )
                        .child({
                            let modal_theme = modal_clone.clone();
                            div().flex_shrink_0().child(
                                Button::new("btn-theme-mode-dropdown")
                                    .ghost()
                                    .compact()
                                    .px(px(10.))
                                    .py(px(4.))
                                    .h(px(28.))
                                    .rounded(px(5.))
                                    .bg(t.bg_input)
                                    .border_1()
                                    .border_color(t.border)
                                    .child(
                                        Icon::new(theme_icon)
                                            .size(px(13.))
                                            .text_color(t.text_secondary),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                                            .text_color(t.text_primary)
                                            .child(theme_label),
                                    )
                                    .child(
                                        Icon::new(IconName::ChevronDown)
                                            .size(px(12.))
                                            .text_color(t.text_muted),
                                    )
                                    .dropdown_menu(move |menu, _window, _cx| {
                                        let m_sys = modal_theme.clone();
                                        let m_dark = modal_theme.clone();
                                        let m_light = modal_theme.clone();
                                        menu.item(
                                            PopupMenuItem::new("System (Follow OS)")
                                                .icon(IconName::Monitor)
                                                .checked(cur_pref == ThemePreference::System)
                                                .on_click(move |_, _, cx| {
                                                    m_sys.update(cx, |st, cx| {
                                                        st.set_theme_preference(
                                                            ThemePreference::System,
                                                            cx,
                                                        );
                                                    });
                                                }),
                                        )
                                        .item(
                                            PopupMenuItem::new("Dark")
                                                .icon(IconName::Moon)
                                                .checked(cur_pref == ThemePreference::Dark)
                                                .on_click(move |_, _, cx| {
                                                    m_dark.update(cx, |st, cx| {
                                                        st.set_theme_preference(
                                                            ThemePreference::Dark,
                                                            cx,
                                                        );
                                                    });
                                                }),
                                        )
                                        .item(
                                            PopupMenuItem::new("Light")
                                                .icon(IconName::Sun)
                                                .checked(cur_pref == ThemePreference::Light)
                                                .on_click(move |_, _, cx| {
                                                    m_light.update(cx, |st, cx| {
                                                        st.set_theme_preference(
                                                            ThemePreference::Light,
                                                            cx,
                                                        );
                                                    });
                                                }),
                                        )
                                    }),
                            )
                        }),
                )
                // Row 2: Split Layout Dropdown
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .p(px(12.))
                        .rounded(px(6.))
                        .bg(t.bg_app)
                        .border_1()
                        .border_color(t.border)
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(16.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(3.))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                        .text_color(t.text_primary)
                                        .child("Editor & Inspector Layout"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(t.text_muted)
                                        .child("Split the Request Composer and Response Inspector panes horizontally or vertically."),
                                ),
                        )
                        .child({
                            let modal_layout = modal_clone;
                            div().flex_shrink_0().child(
                                Button::new("btn-layout-split-dropdown")
                                    .ghost()
                                    .compact()
                                    .px(px(10.))
                                    .py(px(4.))
                                    .h(px(28.))
                                    .rounded(px(5.))
                                    .bg(t.bg_input)
                                    .border_1()
                                    .border_color(t.border)
                                    .child(
                                        Icon::new(layout_icon)
                                            .size(px(13.))
                                            .text_color(t.text_secondary),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                                            .text_color(t.text_primary)
                                            .child(layout_label),
                                    )
                                    .child(
                                        Icon::new(IconName::ChevronDown)
                                            .size(px(12.))
                                            .text_color(t.text_muted),
                                    )
                                    .dropdown_menu(move |menu, _window, _cx| {
                                        let m_horiz = modal_layout.clone();
                                        let m_vert = modal_layout.clone();
                                        menu.item(
                                            PopupMenuItem::new("Horizontal Split (Left / Right)")
                                                .icon(IconName::PanelRight)
                                                .checked(!is_vertical_split)
                                                .on_click(move |_, _, cx| {
                                                    m_horiz.update(cx, |st, cx| {
                                                        st.set_split_layout_vertical(false, cx);
                                                    });
                                                }),
                                        )
                                        .item(
                                            PopupMenuItem::new("Vertical Split (Top / Bottom)")
                                                .icon(IconName::PanelBottom)
                                                .checked(is_vertical_split)
                                                .on_click(move |_, _, cx| {
                                                    m_vert.update(cx, |st, cx| {
                                                        st.set_split_layout_vertical(true, cx);
                                                    });
                                                }),
                                        )
                                    }),
                            )
                        }),
                ),
        )
}
