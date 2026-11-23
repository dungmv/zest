//! Autocomplete popup rendering and keyboard navigation for single-line env inputs.

use crate::theme::theme;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, App, Div, Entity, FontWeight, Hsla, InteractiveElement as _, KeyDownEvent,
    ParentElement as _, ScrollHandle, SharedString, StatefulInteractiveElement as _, Styled,
    Window,
};
use zest_core::env::{VariableCandidate, VariableScope};

/// Width of the candidate list column inside autocomplete popups.
pub const CANDIDATE_LIST_WIDTH: f32 = 240.;
/// Width of the detail card shown next to the popup.
pub const CANDIDATE_DETAIL_WIDTH: f32 = 280.;
/// Horizontal gap between the popup and the detail card beside it.
pub const CANDIDATE_DETAIL_GAP: f32 = 8.;

/// Max height of the detail card, kept below the popup's own cap so the card
/// stays shorter than the list; its height otherwise hugs the content.
const CANDIDATE_DETAIL_MAX_HEIGHT: f32 = 160.;

#[derive(Default, Clone, Debug)]
pub struct AutocompleteNavState {
    pub selected_index: usize,
    pub dismissed: bool,
    pub last_query: Option<String>,
}

/// Handles keyboard navigation events (`ArrowDown`, `ArrowUp`, `Enter`, `Tab`, `Escape`)
/// for autocomplete popups. Returns `true` if the event was consumed.
pub fn handle_autocomplete_keydown<F>(
    event: &KeyDownEvent,
    candidates: &[VariableCandidate],
    nav_state: &Entity<AutocompleteNavState>,
    scroll_handle: &ScrollHandle,
    window: &mut Window,
    cx: &mut App,
    on_select: F,
) -> bool
where
    F: FnOnce(&VariableCandidate, &mut Window, &mut App),
{
    match event.keystroke.key.as_str() {
        "down" | "arrowdown" => {
            cx.stop_propagation();
            let len = candidates.len();
            if len > 0 {
                let next_idx = nav_state.update(cx, |st, cx| {
                    st.selected_index = (st.selected_index + 1) % len;
                    cx.notify();
                    st.selected_index
                });
                scroll_handle.scroll_to_item(next_idx);
            }
            true
        }
        "up" | "arrowup" => {
            cx.stop_propagation();
            let len = candidates.len();
            if len > 0 {
                let prev_idx = nav_state.update(cx, |st, cx| {
                    if st.selected_index == 0 {
                        st.selected_index = len - 1;
                    } else {
                        st.selected_index -= 1;
                    }
                    cx.notify();
                    st.selected_index
                });
                scroll_handle.scroll_to_item(prev_idx);
            }
            true
        }
        "enter" | "tab" => {
            cx.stop_propagation();
            let sel_idx = nav_state.read(cx).selected_index;
            if let Some(cand) = candidates.get(sel_idx) {
                let cand_clone = cand.clone();
                nav_state.update(cx, |st, cx| {
                    st.dismissed = true;
                    cx.notify();
                });
                on_select(&cand_clone, window, cx);
            }
            true
        }
        "escape" => {
            cx.stop_propagation();
            nav_state.update(cx, |st, cx| {
                st.dismissed = true;
                cx.notify();
            });
            true
        }
        _ => false,
    }
}

/// Two-category type display: all environment variables collapse into a single
/// "Variable" type, dynamic fake vars keep their own "Dynamic" type.
/// Returns `(badge label, text/border color, background tint)`.
pub fn candidate_type_badge(scope: &VariableScope) -> (&'static str, Hsla, Hsla) {
    let t = theme();
    match scope {
        VariableScope::Active(_) | VariableScope::Global => (
            "Variable",
            t.tag_variable,
            Hsla {
                a: 0.14,
                ..t.tag_variable
            },
        ),
        VariableScope::Dynamic => (
            "Dynamic",
            t.tag_dynamic,
            Hsla {
                a: 0.14,
                ..t.tag_dynamic
            },
        ),
        VariableScope::Missing => (
            "Unresolved",
            t.var_unresolved,
            Hsla {
                a: 0.14,
                ..t.var_unresolved
            },
        ),
    }
}

/// Renders a single autocomplete row: variable key plus its two-category type badge.
pub fn render_candidate_row<F>(
    id_prefix: &str,
    idx: usize,
    cand: &VariableCandidate,
    is_selected: bool,
    nav_state: &Entity<AutocompleteNavState>,
    on_select: F,
) -> gpui_kit::Stateful<Div>
where
    F: Fn(&VariableCandidate, &mut Window, &mut App) + 'static,
{
    let t = theme();
    let (badge_label, cand_color, cand_bg_tint) = candidate_type_badge(&cand.scope);
    let nav_for_hover = nav_state.clone();
    let nav_for_click = nav_state.clone();
    let cand_for_click = cand.clone();

    div()
        .id(SharedString::from(format!(
            "{}-cand-{}",
            id_prefix, cand.name
        )))
        .cursor_pointer()
        .px(px(8.))
        .py(px(5.))
        .rounded(px(4.))
        .bg(if is_selected {
            t.bg_selected
        } else {
            gpui_kit::transparent_black()
        })
        .hover(|s| s.bg(t.bg_hover))
        .on_hover(move |hovered, _, cx| {
            if *hovered {
                nav_for_hover.update(cx, |st, cx| {
                    st.selected_index = idx;
                    cx.notify();
                });
            }
        })
        .flex()
        .items_center()
        .justify_between()
        .gap(px(8.))
        .on_click(move |_, window, cx| {
            nav_for_click.update(cx, |st, cx| {
                st.dismissed = true;
                cx.notify();
            });
            on_select(&cand_for_click, window, cx);
        })
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .truncate()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .font_family(".SystemMonoFont")
                .text_color(cand_color)
                .child(format!("{{{{{}}}}}", cand.name)),
        )
        .child(
            div()
                .flex_none()
                .px(px(6.))
                .py(px(1.))
                .rounded(px(3.))
                .bg(cand_bg_tint)
                .border_1()
                .border_color(cand_color)
                .text_xs()
                .text_color(cand_color)
                .child(badge_label),
        )
}

/// Renders the compact detail card for the selected candidate, placed next to the popup.
pub fn render_candidate_detail_card(cand: Option<&VariableCandidate>) -> Div {
    let t = theme();
    let Some(cand) = cand else {
        return div();
    };

    let (_, cand_color, _) = candidate_type_badge(&cand.scope);
    let is_dynamic = matches!(cand.scope, VariableScope::Dynamic);

    div()
        .w(px(CANDIDATE_DETAIL_WIDTH))
        .max_h(px(CANDIDATE_DETAIL_MAX_HEIGHT))
        .px(px(10.))
        .py(px(8.))
        .rounded(px(6.))
        .bg(t.bg_card)
        .border_1()
        .border_color(t.border)
        .shadow_2xl()
        .occlude()
        .flex()
        .flex_col()
        .gap(px(4.))
        .overflow_hidden()
        .child(
            div()
                .truncate()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .font_family(".SystemMonoFont")
                .text_color(cand_color)
                .child(format!("{{{{{}}}}}", cand.name)),
        )
        .when(!is_dynamic, |this| {
            this.child(
                div()
                    .text_xs()
                    .text_color(t.text_muted)
                    .child(cand.scope_label()),
            )
        })
        .child(if is_dynamic {
            div()
                .id(SharedString::from(format!(
                    "cand-detail-desc-{}",
                    cand.name
                )))
                .min_h(px(0.))
                .overflow_y_scroll()
                .text_xs()
                .text_color(t.text_secondary)
                .child(cand.description.clone().unwrap_or_default())
        } else {
            div()
                .id(SharedString::from(format!("cand-detail-val-{}", cand.name)))
                .min_h(px(0.))
                .overflow_y_scroll()
                .text_xs()
                .font_family(".SystemMonoFont")
                .text_color(t.text_primary)
                .child(if cand.value.is_empty() {
                    "\"\"".to_string()
                } else {
                    cand.value.clone()
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autocomplete_nav_state() {
        let mut state = AutocompleteNavState::default();
        assert_eq!(state.selected_index, 0);
        assert!(!state.dismissed);
        assert_eq!(state.last_query, None);

        state.last_query = Some("foo".to_string());
        state.selected_index = 2;
        state.dismissed = true;

        assert_eq!(state.selected_index, 2);
        assert!(state.dismissed);
        assert_eq!(state.last_query, Some("foo".to_string()));
    }
}
