use super::ResponseInspector;
use crate::theme::theme;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::Sizable as _;
use gpui_kit::{div, px, AnyElement, App, IntoElement, ParentElement, Styled};
use std::sync::Arc;
use zest_core::models::HttpResponseRecord;

pub(super) fn render_console_tab(
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
    let resp_state_clone = ri.response_state.clone();
    let _current_resp = current_resp.clone();
    if is_sending_this_request {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .child(Spinner::new().with_size(px(20.)).color(t.primary))
            .child(div().text_xs().text_color(t.text_secondary).child(
                "Sending request... Scripts and tests will execute after response is received.",
            ))
            .into_any_element()
    } else {
        crate::components::ConsolePanel::new(resp_state_clone.clone()).into_any_element()
    }
}
