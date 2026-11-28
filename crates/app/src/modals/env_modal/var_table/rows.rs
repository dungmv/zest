use super::EnvVarTable;
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::input::Input;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, Context, InteractiveElement as _, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement as _, Styled,
};

/// Render body of [`EnvVarTable`]: table header, scrollable row list, and
/// per-row secret/delete actions. Row input-pool management and state
/// synchronization live in the parent module; this function only builds the
/// element tree.
pub(super) fn render_var_table(
    table: &EnvVarTable,
    cx: &mut Context<EnvVarTable>,
) -> impl IntoElement {
    let t = theme();
    let selected_id = table.bound_env_id.clone();
    let is_empty = table.rows.is_empty();
    let table_entity = cx.entity();

    div()
        .flex_1()
        .flex()
        .flex_col()
        .overflow_hidden()
        // Table Header
        .child(
            div()
                .h(px(30.))
                .px(px(16.))
                .bg(t.bg_card)
                .border_b_1()
                .border_color(t.border)
                .flex()
                .items_center()
                .gap(px(8.))
                .text_xs()
                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                .text_color(t.text_muted)
                .child(div().w(px(180.)).child("VARIABLE NAME"))
                .child(div().flex_1().child("INITIAL VALUE (GIT)"))
                .child(div().flex_1().child("CURRENT VALUE (LOCAL)"))
                .child(div().w(px(64.)).text_center().child("SECRET"))
                .child(div().w(px(24.)).child("")),
        )
        // Table Rows
        .child(
            div()
                .id("env-var-table-scroll")
                .flex_1()
                .px(px(12.))
                .py(px(6.))
                .track_scroll(&table.scroll_handle)
                .overflow_y_scrollbar()
                .flex()
                .flex_col()
                .gap(px(4.))
                .when(is_empty, |this| {
                    this.child(
                        div()
                            .py(px(60.))
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap(px(8.))
                            .text_xs()
                            .text_color(t.text_muted)
                            .child("No variables yet.")
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(t.accent_text)
                                    .child("Click '+ Add Variable' above to add a new variable."),
                            ),
                    )
                })
                .children(table.rows.iter().enumerate().map(|(idx, row)| {
                    let row_id = format!("{}-row-{}", selected_id, idx);
                    let k_input = row.key_input.clone();
                    let init_input = row.init_input.clone();
                    let curr_input = row.curr_input.clone();
                    let secret = row.secret;

                    let tbl_sec = table_entity.clone();
                    let kin_sec = k_input.clone();
                    let tbl_del = table_entity.clone();
                    let kin_del = k_input.clone();

                    let is_target = table
                        .target_variable
                        .as_deref()
                        .is_some_and(|tv| tv == row.key_input.read(cx).value().as_ref());

                    div()
                        .id(SharedString::from(format!("env-row-item-{}", idx)))
                        .flex()
                        .items_center()
                        .px(px(4.))
                        .py(px(2.))
                        .gap(px(8.))
                        .rounded(px(4.))
                        .hover(|s| s.bg(t.bg_hover))
                        .when(is_target, |s| {
                            s.bg(t.bg_hover).border_1().border_color(t.primary)
                        })
                        // Key Input
                        .child(
                            div()
                                .w(px(180.))
                                .h(px(28.))
                                .px(px(6.))
                                .rounded(px(4.))
                                .bg(t.bg_input)
                                .border_1()
                                .border_color(t.border)
                                .flex()
                                .items_center()
                                .child(
                                    Input::new(&k_input)
                                        .id(SharedString::from(format!("{}-key", row_id)))
                                        .appearance(false)
                                        .bordered(false)
                                        .focus_bordered(false)
                                        .px(px(0.))
                                        .text_sm()
                                        .font_family(".SystemMonoFont"),
                                ),
                        )
                        // Initial Value Input
                        .child(
                            div()
                                .flex_1()
                                .h(px(28.))
                                .px(px(6.))
                                .rounded(px(4.))
                                .bg(t.bg_input)
                                .border_1()
                                .border_color(t.border)
                                .flex()
                                .items_center()
                                .child(
                                    Input::new(&init_input)
                                        .id(SharedString::from(format!("{}-init-val", row_id)))
                                        .appearance(false)
                                        .bordered(false)
                                        .focus_bordered(false)
                                        .px(px(0.))
                                        .text_sm()
                                        .font_family(".SystemMonoFont"),
                                ),
                        )
                        // Current Value Input
                        .child(
                            div()
                                .flex_1()
                                .h(px(28.))
                                .px(px(6.))
                                .rounded(px(4.))
                                .bg(t.bg_input)
                                .border_1()
                                .border_color(t.border)
                                .flex()
                                .items_center()
                                .child(
                                    Input::new(&curr_input)
                                        .id(SharedString::from(format!("{}-curr-val", row_id)))
                                        .appearance(false)
                                        .bordered(false)
                                        .focus_bordered(false)
                                        .px(px(0.))
                                        .text_sm()
                                        .font_family(".SystemMonoFont"),
                                ),
                        )
                        // Secret toggle button
                        .child(
                            div()
                                .id(SharedString::from(format!("{}-secret", row_id)))
                                .w(px(64.))
                                .h(px(24.))
                                .rounded(px(4.))
                                .bg(if secret {
                                    gpui_kit::Hsla {
                                        a: 0.15,
                                        ..t.status_4xx
                                    }
                                } else {
                                    gpui_kit::transparent_black()
                                })
                                .hover(|s| s.bg(t.bg_card))
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_xs()
                                .text_color(if secret { t.status_4xx } else { t.text_muted })
                                .on_click(move |_, _, cx| {
                                    tbl_sec.update(cx, |this, cx| {
                                        this.toggle_row_secret_by_key(&kin_sec, cx);
                                    });
                                })
                                .child(if secret {
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(4.))
                                        .child(Icon::new(IconName::Lock).size(px(14.)))
                                        .child(" Secret")
                                        .into_any_element()
                                } else {
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(4.))
                                        .child(Icon::new(IconName::LockOpen).size(px(14.)))
                                        .child(" Visible")
                                        .into_any_element()
                                }),
                        )
                        // Delete row button
                        .child(
                            div()
                                .id(SharedString::from(format!("{}-del", row_id)))
                                .w(px(24.))
                                .h(px(24.))
                                .rounded(px(4.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| s.bg(t.bg_card).text_color(t.status_5xx))
                                .text_color(t.text_muted)
                                .on_click(move |_, _, cx| {
                                    tbl_del.update(cx, |this, cx| {
                                        this.delete_row_by_key(&kin_del, cx);
                                    });
                                })
                                .child(Icon::new(IconName::Trash).small()),
                        )
                })),
        )
}
