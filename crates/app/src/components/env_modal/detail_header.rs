use crate::state::{EnvironmentState, PersistenceState};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, Div, Entity, InteractiveElement as _, ParentElement, StatefulInteractiveElement as _,
    Styled,
};
use zest_core::models::Environment;

pub(super) struct DetailHeaderProps {
    pub env_state: Entity<EnvironmentState>,
    pub persistence_state: Entity<PersistenceState>,
    pub selected_id: String,
    pub is_global_selected: bool,
    pub selected_custom_env: Option<Environment>,
    pub active_conflict: Option<String>,
}

pub(super) fn render_detail_header(props: DetailHeaderProps) -> Div {
    let t = theme();
    let ws_clone = props.env_state;
    let per_clone = props.persistence_state;
    let selected_id = props.selected_id;
    let is_global_selected = props.is_global_selected;
    let selected_custom_env = props.selected_custom_env;
    let active_conflict = props.active_conflict;

    let conflict_key = if is_global_selected {
        "globals".to_string()
    } else {
        selected_custom_env
            .as_ref()
            .map(|e| e.name.clone())
            .unwrap_or_else(|| selected_id.clone())
    };

    div()
        .flex()
        .flex_col()
        .when_some(active_conflict, |this, msg| {
            let per_rel = per_clone.clone();
            let per_keep = per_clone.clone();
            let env_rel = ws_clone.clone();
            let env_keep = ws_clone.clone();
            let key_rel = conflict_key.clone();
            let key_keep = conflict_key.clone();
            this.child(
                div()
                    .px(px(12.))
                    .py(px(6.))
                    .bg(gpui_kit::Hsla {
                        a: 0.15,
                        ..t.status_4xx
                    })
                    .border_b_1()
                    .border_color(t.status_4xx)
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_xs()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .font_weight(gpui_kit::FontWeight::BOLD)
                                    .text_color(t.status_4xx)
                                    .child("! Conflict:"),
                            )
                            .child(div().text_color(t.text_primary).child(msg)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                Button::new("btn-env-conflict-reload")
                                    .primary()
                                    .small()
                                    .label("Reload")
                                    .on_click(move |_, _, cx| {
                                        per_rel.update(cx, |per, cx| {
                                            per.resolve_env_conflict_reload(
                                                &key_rel,
                                                env_rel.clone(),
                                                cx,
                                            );
                                        });
                                    }),
                            )
                            .child(
                                Button::new("btn-env-conflict-keep")
                                    .secondary()
                                    .small()
                                    .label("Keep mine")
                                    .on_click(move |_, _, cx| {
                                        per_keep.update(cx, |per, cx| {
                                            per.resolve_env_conflict_keep(
                                                &key_keep,
                                                env_keep.clone(),
                                                cx,
                                            );
                                        });
                                    }),
                            ),
                    ),
            )
        })
        .child(
            div()
                .h(px(54.))
                .px(px(16.))
                .border_b_1()
                .border_color(t.border)
                .bg(t.bg_card)
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .when(is_global_selected, |this| {
                            this.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(1.))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui_kit::FontWeight::BOLD)
                                            .text_color(t.text_primary)
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(4.))
                                                    .child(Icon::new(IconName::Globe).size(px(14.)))
                                                    .child("Global Variables"),
                                            ),
                                    )
                                    .child(
                                        div().text_xs().text_color(t.text_muted).child(
                                            "Shared across all requests. Active environment overrides matching keys.",
                                        ),
                                    ),
                            )
                        })
                        .when(!is_global_selected, |this| {
                            if let Some(custom) = &selected_custom_env {
                                this.child(
                                    div().flex().items_center().gap(px(8.)).child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui_kit::FontWeight::BOLD)
                                            .text_color(t.text_primary)
                                            .child(custom.name.clone()),
                                    ),
                                )
                            } else {
                                this.child(div().text_sm().child("Select an environment"))
                            }
                        }),
                )
                .child(
                    div().flex().items_center().gap(px(8.)).child({
                        let ws_add = ws_clone.clone();
                        let is_glob = is_global_selected;
                        let cur_id = selected_id.clone();

                        div()
                            .id("btn-detail-add-variable")
                            .cursor_pointer()
                            .px(px(10.))
                            .py(px(4.))
                            .rounded(px(4.))
                            .bg(t.primary)
                            .text_color(t.primary_foreground)
                            .hover(|s| s.bg(t.primary_hover))
                            .text_xs()
                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                            .on_click(move |_, _, cx| {
                                ws_add.update(cx, |state, cx| {
                                    state.add_variable(
                                        if is_glob { "globals" } else { &cur_id },
                                        cx,
                                    );
                                });
                            })
                            .child("+ Add Variable")
                    }),
                ),
        )
}
