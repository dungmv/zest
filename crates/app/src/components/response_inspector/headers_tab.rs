use super::ResponseInspector;
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::{
    div, px, AnyElement, App, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled,
};
use std::sync::Arc;
use zest_core::models::HttpResponseRecord;

pub(super) fn render_headers_tab(
    ri: &ResponseInspector,
    cx: &App,
    is_sending_this_request: bool,
    current_resp: &Option<Arc<HttpResponseRecord>>,
) -> AnyElement {
    let t = theme();
    let resp_s = ri.response_state.read(cx);
    let ed_s = ri.editor_state.read(cx);
    let _active_id = ed_s.active_request_id.clone();
    let _is_viewing_example = resp_s.active_example_name.is_some();
    let _view_mode = resp_s.view_mode;
    let _resp_state_clone = ri.response_state.clone();
    let current_resp = current_resp.clone();
    if is_sending_this_request {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .child(Spinner::new().with_size(px(20.)).color(t.primary))
            .child(
                div()
                    .text_xs()
                    .text_color(t.text_secondary)
                    .child("Sending request... Response headers will appear here."),
            )
            .into_any_element()
    } else if let Some(resp) = &current_resp {
        div()
            .size_full()
            .overflow_y_scrollbar()
            .overflow_x_hidden()
            .px(px(8.))
            .py(px(4.))
            .flex()
            .flex_col()
            .children(resp.headers.iter().enumerate().map(|(idx, (k, v))| {
                let val_copy = v.clone();
                let val_tip = v.clone();
                let key_tip = k.clone();
                div()
                    .id(format!("resp-hdr-row-{}", idx))
                    .w_full()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(6.))
                    .py(px(3.))
                    .rounded(px(4.))
                    .border_b_1()
                    .border_color(t.border_subtle)
                    .hover(|s| s.bg(t.bg_hover))
                    .text_xs()
                    .font_family(".SystemMonoFont")
                    .child(
                        div()
                            .id(format!("hdr-key-{}", idx))
                            .w(px(180.))
                            .flex_shrink_0()
                            .truncate()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .text_color(t.primary)
                            .tooltip(move |window, cx| {
                                gpui_kit::component::tooltip::Tooltip::new(key_tip.clone())
                                    .build(window, cx)
                            })
                            .child(k.clone()),
                    )
                    .child(
                        div()
                            .id(format!("hdr-val-{}", idx))
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .text_color(t.text_primary)
                            .tooltip(move |window, cx| {
                                gpui_kit::component::tooltip::Tooltip::new(val_tip.clone())
                                    .build(window, cx)
                            })
                            .child(v.clone()),
                    )
                    .child(
                        div()
                            .id(format!("copy-hdr-{}", idx))
                            .flex_shrink_0()
                            .cursor_pointer()
                            .p(px(2.))
                            .rounded(px(4.))
                            .text_color(t.text_muted)
                            .hover(|s| s.text_color(t.text_primary).bg(t.bg_card))
                            .tooltip(move |window, cx| {
                                gpui_kit::component::tooltip::Tooltip::new("Copy value")
                                    .build(window, cx)
                            })
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(
                                    val_copy.clone(),
                                ));
                            })
                            .child(Icon::new(IconName::Copy).size(px(14.))),
                    )
            }))
            .into_any_element()
    } else {
        div()
            .p(px(12.))
            .text_xs()
            .text_color(t.text_muted)
            .child("No headers received.")
            .into_any_element()
    }
}
