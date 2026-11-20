use crate::theme::AppTheme;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, App, Entity, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled, Window,
};
use std::path::PathBuf;
use zest_core::importers::ImportResult;
use zest_core::models::HttpMethod;

use super::ImportModalState;

pub(super) fn render_file_picker_view(
    selected_file: Option<&PathBuf>,
    modal_state: Entity<ImportModalState>,
    t: &AppTheme,
) -> impl IntoElement {
    let st_pick = modal_state.clone();

    div().flex().flex_col().gap(px(8.)).child(
        div()
            .p(px(16.))
            .rounded(px(8.))
            .border_1()
            .border_color(t.border)
            .bg(t.bg_panel)
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                            .text_color(t.text_primary)
                            .child(match selected_file {
                                Some(f) => f
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("Selected File")
                                    .to_string(),
                                None => "No file selected".to_string(),
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(t.text_muted)
                            .child(match selected_file {
                                Some(f) => f.to_string_lossy().to_string(),
                                None => "Supports .json, .yaml, .yml files".to_string(),
                            }),
                    ),
            )
            .child(
                div()
                    .id("btn-browse-import-file")
                    .px(px(12.))
                    .py(px(6.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(t.border)
                    .bg(t.bg_card)
                    .cursor_pointer()
                    .hover(|s| s.bg(t.bg_hover).border_color(t.primary))
                    .text_xs()
                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                    .text_color(t.text_primary)
                    .child("Browse File...")
                    .on_click(move |_, _, cx| {
                        let st_pick = st_pick.clone();
                        cx.spawn(async move |cx| {
                            if let Some(file) = rfd::AsyncFileDialog::new()
                                .add_filter(
                                    "API Specification & Collections",
                                    &["json", "yaml", "yml"],
                                )
                                .pick_file()
                                .await
                            {
                                let path = file.path().to_path_buf();
                                let read_res = cx
                                    .background_executor()
                                    .spawn(async move {
                                        std::fs::read_to_string(&path).map(|c| (path, c))
                                    })
                                    .await;
                                st_pick.update(cx, |s, cx| match read_res {
                                    Ok((path, content)) => s.set_loaded_file(path, content, cx),
                                    Err(e) => s.set_file_read_error(e.to_string(), cx),
                                });
                            }
                        })
                        .detach();
                    }),
            ),
    )
}

pub(super) fn render_preview_card(res: &ImportResult, t: &AppTheme) -> impl IntoElement {
    div()
        .p(px(12.))
        .rounded(px(8.))
        .bg(t.bg_panel)
        .border_1()
        .border_color(t.border)
        .flex()
        .flex_col()
        .gap(px(10.))
        // Summary row
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(gpui_kit::FontWeight::BOLD)
                                .text_color(t.text_primary)
                                .child(res.title.clone()),
                        )
                        .when_some(res.description.as_ref(), |this, desc| {
                            this.child(
                                div()
                                    .text_xs()
                                    .text_color(t.text_muted)
                                    .child(format!("— {}", desc)),
                            )
                        }),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(6.))
                        .child(render_badge(
                            &format!("{} requests", res.requests.len()),
                            t.primary,
                        ))
                        .child(render_badge(
                            &format!("{} folders", res.folders.len()),
                            t.text_secondary,
                        ))
                        .when(!res.environments.is_empty(), |this| {
                            this.child(render_badge(
                                &format!("{} env", res.environments.len()),
                                t.status_3xx,
                            ))
                        }),
                ),
        )
        // Requests preview list (scrollable)
        .child(
            div()
                .max_h(px(140.))
                .overflow_y_scrollbar()
                .flex()
                .flex_col()
                .gap(px(4.))
                .children(res.requests.iter().take(20).map(|r| {
                    div()
                        .px(px(8.))
                        .py(px(4.))
                        .rounded(px(4.))
                        .bg(t.bg_card)
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .overflow_hidden()
                                .child(render_method_pill(r.method, t))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                                        .text_color(t.text_primary)
                                        .child(r.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(t.text_muted)
                                        .truncate()
                                        .child(r.url.clone()),
                                ),
                        )
                        .when(!r.folder_path.is_empty(), |this| {
                            this.child(
                                div()
                                    .text_xs()
                                    .text_color(t.text_muted)
                                    .child(r.folder_path.join(" / ")),
                            )
                        })
                }))
                .when(res.requests.len() > 20, |this| {
                    this.child(
                        div()
                            .text_xs()
                            .text_color(t.text_muted)
                            .py(px(4.))
                            .child(format!("... and {} more requests", res.requests.len() - 20)),
                    )
                }),
        )
}

fn render_badge(label: &str, color: gpui_kit::Hsla) -> impl IntoElement {
    div()
        .px(px(6.))
        .py(px(2.))
        .rounded(px(4.))
        .bg(color.opacity(0.12))
        .text_xs()
        .font_weight(gpui_kit::FontWeight::MEDIUM)
        .text_color(color)
        .child(label.to_string())
}

fn render_method_pill(method: HttpMethod, t: &AppTheme) -> impl IntoElement {
    let color = match method {
        HttpMethod::Get => t.method_get,
        HttpMethod::Post => t.method_post,
        HttpMethod::Put => t.method_put,
        HttpMethod::Delete => t.method_delete,
        HttpMethod::Patch => t.method_patch,
        HttpMethod::Head => t.method_head,
        HttpMethod::Options => t.method_options,
    };

    div()
        .w(px(44.))
        .flex()
        .items_center()
        .justify_center()
        .py(px(1.))
        .text_xs()
        .font_weight(gpui_kit::FontWeight::NORMAL)
        .text_color(color)
        .child(method.as_str())
}

pub(super) fn render_checkbox<F>(
    id: &'static str,
    label: &'static str,
    checked: bool,
    on_click: F,
) -> impl IntoElement
where
    F: Fn(&bool, &mut Window, &mut App) + 'static,
{
    Checkbox::new(id)
        .label(label)
        .checked(checked)
        .on_click(on_click)
}
