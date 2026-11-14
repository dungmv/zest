use super::{AutocompleteNavState, EnvVarCandidate, EnvVarKind};
use crate::theme::theme;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, App, Div, Entity, FontWeight, InteractiveElement as _, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled, Window,
};

/// Width of the candidate list column inside autocomplete popups.
pub const CANDIDATE_LIST_WIDTH: f32 = 240.;
/// Width of the detail card shown next to the popup.
pub const CANDIDATE_DETAIL_WIDTH: f32 = 280.;
/// Horizontal gap between the popup and the detail card beside it.
pub const CANDIDATE_DETAIL_GAP: f32 = 8.;

/// Max height of the detail card, kept below the popup's own cap so the card
/// stays shorter than the list; its height otherwise hugs the content.
const CANDIDATE_DETAIL_MAX_HEIGHT: f32 = 160.;

/// Two-category type display: all environment variables collapse into a single
/// "Variable" type, dynamic fake vars keep their own "Dynamic" type.
/// Returns `(badge label, text/border color, background tint)`.
pub fn candidate_type_badge(kind: EnvVarKind) -> (&'static str, gpui_kit::Hsla, gpui_kit::Hsla) {
    let t = theme();
    match kind {
        EnvVarKind::Active | EnvVarKind::Global => (
            "Variable",
            t.tag_variable,
            gpui_kit::Hsla {
                a: 0.14,
                ..t.tag_variable
            },
        ),
        EnvVarKind::Dynamic => (
            "Dynamic",
            t.tag_dynamic,
            gpui_kit::Hsla {
                a: 0.14,
                ..t.tag_dynamic
            },
        ),
        // Never produced by `get_env_completions`; kept for exhaustiveness.
        EnvVarKind::Unresolved => (
            "Unresolved",
            t.var_unresolved,
            gpui_kit::Hsla {
                a: 0.14,
                ..t.var_unresolved
            },
        ),
    }
}

/// Renders a single autocomplete row: variable key plus its two-category type
/// badge. Hovering the row moves the keyboard selection to it so the detail
/// card follows the mouse; clicking dismisses the popup and runs `on_select`.
pub fn render_candidate_row<F>(
    id_prefix: &str,
    idx: usize,
    cand: &EnvVarCandidate,
    is_selected: bool,
    nav_state: &Entity<AutocompleteNavState>,
    on_select: F,
) -> gpui_kit::Stateful<Div>
where
    F: Fn(&EnvVarCandidate, &mut Window, &mut App) + 'static,
{
    let t = theme();
    let (badge_label, cand_color, cand_bg_tint) = candidate_type_badge(cand.kind);
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

/// Renders the compact detail card for the selected candidate, placed next to
/// the popup: key, source scope (environment variables only), and the wrapped
/// value or description. The card hugs its content instead of stretching to
/// the height of the popup list.
pub fn render_candidate_detail_card(cand: Option<&EnvVarCandidate>) -> Div {
    let t = theme();
    let Some(cand) = cand else {
        return div();
    };

    let (_, cand_color, _) = candidate_type_badge(cand.kind);

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
        .when(cand.kind != EnvVarKind::Dynamic, |this| {
            this.child(
                div()
                    .text_xs()
                    .text_color(t.text_muted)
                    .child(cand.scope.clone()),
            )
        })
        .child(if cand.kind == EnvVarKind::Dynamic {
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
