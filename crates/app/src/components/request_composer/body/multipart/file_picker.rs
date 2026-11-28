use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, InteractiveElement as _, IntoElement, ParentElement, StatefulInteractiveElement as _,
    Styled,
};

use super::row::RowHandles;

/// Read-only "choose file" button shown for `File` rows; spawns the native
/// file dialog off the UI thread and writes the picked path back.
pub(super) fn render_file_picker(
    handles: &RowHandles,
    idx: usize,
    file_path: &Option<String>,
) -> impl IntoElement {
    let t = theme();
    let has_file = file_path.is_some();
    let display_text = file_path
        .as_deref()
        .and_then(|p| std::path::Path::new(p).file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("Choose file...")
        .to_string();
    let tip_path = file_path.clone();

    let handles_for_dialog = handles.clone();

    div()
        .id(format!("btn-select-file-{}", idx))
        .h(px(28.))
        .w_full()
        .min_w(px(0.))
        .px(px(8.))
        .rounded(px(4.))
        .bg(if has_file { t.bg_card } else { t.bg_input })
        .border_1()
        .border_color(t.border)
        .hover(|s| s.bg(t.bg_hover).border_color(t.border_focus))
        .cursor_pointer()
        .flex()
        .items_center()
        .gap(px(6.))
        .when_some(tip_path, |this, path| {
            this.tooltip(move |window, cx| {
                gpui_kit::component::tooltip::Tooltip::new(path.clone()).build(window, cx)
            })
        })
        .on_click(move |_, _, cx| {
            let h = handles_for_dialog.clone();
            cx.spawn(async move |cx| {
                if let Some(file) = rfd::AsyncFileDialog::new().pick_file().await {
                    let p = file.path().to_string_lossy().to_string();
                    cx.update(|cx| {
                        h.mutate(cx, |st, pos, cx| {
                            st.set_file_path(pos, Some(p), cx);
                        });
                    });
                }
            })
            .detach();
        })
        .child(
            Icon::new(if has_file {
                IconName::File
            } else {
                IconName::Folder
            })
            .size(px(14.))
            .flex_shrink_0()
            .text_color(if has_file {
                t.text_primary
            } else {
                t.text_muted
            }),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_xs()
                .font_family(if has_file { ".SystemMonoFont" } else { "" })
                .text_color(if has_file {
                    t.text_primary
                } else {
                    t.text_muted
                })
                .child(display_text),
        )
}
