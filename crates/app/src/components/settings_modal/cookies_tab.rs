use crate::state::{ModalState, WorkspaceDataState};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::empty::{
    Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyMediaVariant, EmptyTitle,
};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, App, AppContext as _, Entity, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled, Window,
};

pub fn render_cookies_tab(
    workspace_state: &Entity<WorkspaceDataState>,
    modal_state: &Entity<ModalState>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let t = theme();
    let ws_clone = workspace_state.clone();
    let modal_clone = modal_state.clone();

    let all_cookies = ws_clone.read(cx).cookies.clone();
    let total_count = all_cookies.len();

    let filter_holder = window.use_keyed_state(
        gpui_kit::SharedString::from("settings-cookie-filter-input"),
        cx,
        |window, cx| {
            let init_q = modal_clone.read(cx).cookie_filter_query.clone();
            let inp = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Filter cookies by domain, name, or value...")
            });
            if !init_q.is_empty() {
                inp.update(cx, |s, cx| s.set_value(init_q, window, cx));
            }
            let modal_sub = modal_clone.clone();
            let sub = cx.subscribe_in(&inp, window, move |_, input, event, _, cx| {
                if let InputEvent::Change = event {
                    let q = input.read(cx).value().to_string();
                    modal_sub.update(cx, |st, cx| {
                        st.set_cookie_filter_query(q, cx);
                    });
                }
            });
            (inp, sub)
        },
    );

    let filter_input = filter_holder.read(cx).0.clone();
    let query = modal_clone
        .read(cx)
        .cookie_filter_query
        .trim()
        .to_lowercase();

    let filtered_cookies: Vec<_> = all_cookies
        .into_iter()
        .filter(|c| {
            query.is_empty()
                || c.domain.to_lowercase().contains(&query)
                || c.name.to_lowercase().contains(&query)
                || c.value.to_lowercase().contains(&query)
        })
        .collect();

    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(14.))
        // Header & Clear All button
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
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
                                .font_weight(gpui_kit::FontWeight::BOLD)
                                .text_color(t.text_secondary)
                                .child(format!("WORKSPACE COOKIE JAR ({})", total_count)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(t.text_muted)
                                .child("Inspect session cookies captured from responses or remove individual cookies."),
                        ),
                )
                .when(total_count > 0, |this| {
                    let ws_clear = ws_clone.clone();
                    this.child(
                        div()
                            .id("btn-settings-clear-all-cookies")
                            .flex_shrink_0()
                            .px(px(10.))
                            .py(px(5.))
                            .rounded(px(4.))
                            .bg(t.bg_input)
                            .border_1()
                            .border_color(t.border)
                            .text_color(t.status_5xx)
                            .hover(|s| s.bg(t.bg_hover))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .gap(px(5.))
                            .text_xs()
                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                            .on_click(move |_, _, cx| {
                                ws_clear.update(cx, |ws, cx| {
                                    ws.clear_cookies(cx);
                                });
                            })
                            .child(Icon::new(IconName::Trash).size(px(12.)))
                            .child("Clear All"),
                    )
                }),
        )
        // Search / Filter bar
        .child(
            div()
                .w_full()
                .min_w_0()
                .h(px(32.))
                .px(px(10.))
                .rounded(px(6.))
                .bg(t.bg_input)
                .border_1()
                .border_color(t.border)
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    Icon::new(IconName::Search)
                        .size(px(14.))
                        .text_color(t.text_muted),
                )
                .child(
                    div().flex_1().min_w_0().child(
                        Input::new(&filter_input)
                            .id("settings-cookie-filter")
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false)
                            .cleanable(true),
                    ),
                ),
        )
        // Cookie List or Empty State
        .child(if filtered_cookies.is_empty() {
            Empty::new()
                .header(
                    EmptyHeader::new()
                        .media(
                            EmptyMedia::new()
                                .with_variant(EmptyMediaVariant::Icon)
                                .child(Icon::new(IconName::Cookie).size(px(24.))),
                        )
                        .title(
                            EmptyTitle::new().child(if total_count == 0 {
                                "No cookies stored in this workspace"
                            } else {
                                "No cookies match your filter"
                            }),
                        )
                        .description(
                            EmptyDescription::new().child(if total_count == 0 {
                                "Cookies received via Set-Cookie response headers will automatically appear here."
                            } else {
                                "Try searching by a different domain, cookie name, or value."
                            }),
                        ),
                )
                .into_any_element()
        } else {
            div()
                .w_full()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(8.))
                .children(filtered_cookies.into_iter().enumerate().map(|(idx, cookie)| {
                    let ws_del = ws_clone.clone();
                    let c_domain = cookie.domain.clone();
                    let c_path = cookie.path.clone();
                    let c_name = cookie.name.clone();
                    let val_copy = cookie.value.clone();

                    div()
                        .id(format!("cookie-row-{}", idx))
                        .w_full()
                        .min_w_0()
                        .p(px(10.))
                        .rounded(px(6.))
                        .bg(t.bg_app)
                        .border_1()
                        .border_color(t.border)
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        // Top row: Name + Badges + Actions (Copy / Delete)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(6.))
                                        .min_w_0()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_family(".SystemMonoFont")
                                                .font_weight(gpui_kit::FontWeight::BOLD)
                                                .text_color(t.primary)
                                                .child(cookie.name.clone()),
                                        )
                                        .when(!cookie.domain.is_empty(), |this| {
                                            this.child(
                                                Tag::secondary()
                                                    .rounded(px(3.))
                                                    .px(px(6.))
                                                    .py(px(0.5))
                                                    .child(
                                                        div()
                                                            .text_size(px(10.))
                                                            .font_family(".SystemMonoFont")
                                                            .child(cookie.domain.clone()),
                                                    ),
                                            )
                                        })
                                        .child(
                                            Tag::secondary()
                                                .rounded(px(3.))
                                                .px(px(5.))
                                                .py(px(0.5))
                                                .child(
                                                    div()
                                                        .text_size(px(10.))
                                                        .font_family(".SystemMonoFont")
                                                        .child(cookie.path.clone()),
                                                ),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(4.))
                                        .when(cookie.secure, |this| {
                                            this.child(
                                                Tag::success()
                                                    .rounded(px(3.))
                                                    .px(px(5.))
                                                    .py(px(0.5))
                                                    .child(
                                                        div()
                                                            .text_size(px(10.))
                                                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                            .child("Secure"),
                                                    ),
                                            )
                                        })
                                        .when(cookie.http_only, |this| {
                                            this.child(
                                                Tag::info()
                                                    .rounded(px(3.))
                                                    .px(px(5.))
                                                    .py(px(0.5))
                                                    .child(
                                                        div()
                                                            .text_size(px(10.))
                                                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                            .child("HttpOnly"),
                                                    ),
                                            )
                                        })
                                        .when_some(cookie.same_site.clone(), |this, ss| {
                                            this.child(
                                                Tag::secondary()
                                                    .rounded(px(3.))
                                                    .px(px(5.))
                                                    .py(px(0.5))
                                                    .child(
                                                        div()
                                                            .text_size(px(10.))
                                                            .child(format!("SameSite={}", ss)),
                                                    ),
                                            )
                                        })
                                        // Copy cookie value
                                        .child(
                                            div()
                                                .id(format!("btn-copy-cookie-{}", idx))
                                                .cursor_pointer()
                                                .p(px(4.))
                                                .rounded(px(4.))
                                                .text_color(t.text_muted)
                                                .hover(|s| {
                                                    s.bg(t.bg_hover).text_color(t.text_primary)
                                                })
                                                .on_click(move |_, _, cx| {
                                                    cx.write_to_clipboard(
                                                        gpui_kit::ClipboardItem::new_string(
                                                            val_copy.clone(),
                                                        ),
                                                    );
                                                })
                                                .child(Icon::new(IconName::Copy).size(px(13.))),
                                        )
                                        // Delete single cookie
                                        .child(
                                            div()
                                                .id(format!("btn-del-cookie-{}", idx))
                                                .cursor_pointer()
                                                .p(px(4.))
                                                .rounded(px(4.))
                                                .text_color(t.text_muted)
                                                .hover(|s| {
                                                    s.bg(t.bg_hover).text_color(t.status_5xx)
                                                })
                                                .on_click(move |_, _, cx| {
                                                    ws_del.update(cx, |ws, cx| {
                                                        ws.remove_cookie(
                                                            &c_domain, &c_path, &c_name, cx,
                                                        );
                                                    });
                                                })
                                                .child(Icon::new(IconName::Trash).size(px(13.))),
                                        ),
                                ),
                        )
                        // Value row
                        .child(
                            div()
                                .px(px(8.))
                                .py(px(5.))
                                .rounded(px(4.))
                                .bg(t.bg_card)
                                .border_1()
                                .border_color(t.border_subtle)
                                .font_family(".SystemMonoFont")
                                .text_xs()
                                .text_color(t.text_primary)
                                .truncate()
                                .child(cookie.value.clone()),
                        )
                        // Optional expires footer
                        .when_some(cookie.expires, |this, exp| {
                            this.child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(t.text_muted)
                                    .child(format!("Expires: {}", exp)),
                            )
                        })
                }))
                .into_any_element()
        })
}
