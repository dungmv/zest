use crate::state::{CollectionState, RequestSessionState};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, AnyElement, Entity, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled,
};

/// Formats a byte count into a human-readable size string (pure, no disk I/O).
pub fn format_byte_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

pub fn render_binary_editor(
    req_id_str: &str,
    file_path: &str,
    size_label: &str,
    editor_state: &Entity<RequestSessionState>,
    workspace_state: Option<&Entity<CollectionState>>,
) -> AnyElement {
    let t = theme();
    let file_path = file_path.to_string();
    let ed_c = editor_state.clone();
    let has_file = !file_path.is_empty();
    let size_str = size_label.to_string();
    let req_id_str = req_id_str.to_string();
    let ws_state = workspace_state.cloned();

    div()
        .size_full()
        .p(px(20.))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .child(
            div()
                .w(px(460.))
                .p(px(20.))
                .rounded(px(8.))
                .bg(t.bg_card)
                .border_1()
                .border_color(t.border)
                .flex()
                .flex_col()
                .gap(px(14.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(
                            Icon::new(IconName::Package)
                                .size(px(20.))
                                .text_color(t.text_secondary),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(gpui_kit::FontWeight::BOLD)
                                        .text_color(t.text_primary)
                                        .child("Binary Request Payload"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(t.text_secondary)
                                        .child("Select a file from your system to send as the raw binary body."),
                                ),
                        ),
                )
                .child(
                    div()
                        .p(px(10.))
                        .rounded(px(6.))
                        .bg(t.bg_input)
                        .border_1()
                        .border_color(t.border)
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
                                        .font_family(".SystemMonoFont")
                                        .text_color(if has_file {
                                            t.text_primary
                                        } else {
                                            t.text_muted
                                        })
                                        .child(if has_file {
                                            file_path.clone()
                                        } else {
                                            "No file chosen yet".to_string()
                                        }),
                                )
                                .when(has_file, |this| {
                                    this.child(
                                        div()
                                            .text_xs()
                                            .text_color(t.text_muted)
                                            .child(format!("Size: {}", size_str)),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child({
                                    let ed_pick = ed_c.clone();
                                    let ws_pick = ws_state.clone();
                                    let id_pick = req_id_str.clone();
                                    div()
                                        .id("btn-browse-binary-file")
                                        .px(px(10.))
                                        .py(px(4.))
                                        .rounded(px(4.))
                                        .bg(t.primary)
                                        .text_color(t.primary_foreground)
                                        .hover(|s| s.bg(t.primary_hover))
                                        .cursor_pointer()
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                        .on_click(move |_, _, cx| {
                                            let ed_bg = ed_pick.clone();
                                            let ws_bg = ws_pick.clone();
                                            let id_bg = id_pick.clone();
                                            cx.spawn(async move |cx| {
                                                if let Some(file) =
                                                    rfd::AsyncFileDialog::new().pick_file().await
                                                {
                                                    let p =
                                                        file.path().to_string_lossy().to_string();
                                                    cx.update(|cx| {
                                                        if let Some(ws) = &ws_bg {
                                                            let path_for_req = p.clone();
                                                            ws.update(cx, |ws, _| {
                                                                ws.update_active_request(
                                                                    &id_bg,
                                                                    |r| {
                                                                        r.set_binary_file(
                                                                            path_for_req,
                                                                        );
                                                                    },
                                                                );
                                                            });
                                                        }
                                                        ed_bg.update(cx, |state, cx| {
                                                            state.refresh_binary_file_size(p, cx);
                                                            state.mark_dirty(cx);
                                                        });
                                                    });
                                                }
                                            })
                                            .detach();
                                        })
                                        .child("Browse File...")
                                })
                                .when(has_file, |this| {
                                    let ed_clr = ed_c.clone();
                                    let ws_clr = ws_state.clone();
                                    let id_clr = req_id_str.clone();
                                    this.child(
                                        div()
                                            .id("btn-clear-binary-file")
                                            .px(px(8.))
                                            .py(px(4.))
                                            .rounded(px(4.))
                                            .bg(t.bg_card)
                                            .border_1()
                                            .border_color(t.border)
                                            .hover(|s| s.bg(t.bg_hover))
                                            .cursor_pointer()
                                            .text_xs()
                                            .text_color(t.text_secondary)
                                            .on_click(move |_, _, cx| {
                                                if let Some(ws) = &ws_clr {
                                                    ws.update(cx, |ws, _| {
                                                        ws.update_active_request(&id_clr, |r| {
                                                            r.set_binary_file("".to_string());
                                                        });
                                                    });
                                                }
                                                ed_clr.update(cx, |state, cx| {
                                                    state.refresh_binary_file_size(
                                                        String::new(),
                                                        cx,
                                                    );
                                                    state.mark_dirty(cx);
                                                });
                                            })
                                            .child("Clear"),
                                    )
                                }),
                        ),
                ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::format_byte_size;

    #[test]
    fn formats_byte_sizes_across_units() {
        assert_eq!(format_byte_size(0), "0 B");
        assert_eq!(format_byte_size(512), "512 B");
        assert_eq!(format_byte_size(1536), "1.5 KB");
        assert_eq!(format_byte_size(2 * 1024 * 1024), "2.0 MB");
    }
}
