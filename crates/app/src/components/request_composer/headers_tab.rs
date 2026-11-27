use crate::app::AppView;
use crate::components::common::mask_secret;
use crate::components::kv_table::KvReadonlyRow;
use crate::components::*;
use crate::state::*;
use gpui_kit::{div, AnyElement, Context, IntoElement, ParentElement, Styled};
use zest_core::models::RequestItem;

/// Builds the read-only rows appended to the Headers `KvTable`:
/// - ancestor headers, using the exact send-time merge semantics. Rows the request
///   overrides (case-insensitive key match) are excluded — they are not sent, so
///   showing them as inherited values would be misleading.
/// - a preview of the header contributed by inherited auth (e.g. Bearer ->
///   `Authorization: Bearer ***`), when the request auth is `Inherit`.
pub(crate) fn inherited_readonly_rows(
    ws: &CollectionState,
    req: &RequestItem,
) -> Vec<KvReadonlyRow> {
    let mut rows: Vec<KvReadonlyRow> = ws
        .inherited_headers_for_request(req)
        .into_iter()
        .filter(|h| !h.overridden_by_request)
        .map(|h| KvReadonlyRow {
            key: h.pair.key,
            value: h.pair.value,
            enabled: h.pair.enabled,
            source_detail: format!("Inherited from {}", h.source),
        })
        .collect();

    if req.auth.is_inherit() {
        if let Some(info) = ws.inherited_auth_source(req.folder_id.as_deref()) {
            let scope = if info.source_folder_id.is_some() {
                format!("folder \"{}\"", info.source_name)
            } else {
                format!("workspace \"{}\"", info.source_name)
            };
            let source_detail = format!("Added by inherited auth from {scope}");
            if let Some((key, value)) = info.auth.contributed_header_preview(mask_secret) {
                rows.push(KvReadonlyRow {
                    key,
                    value,
                    enabled: true,
                    source_detail,
                });
            }
        }
    }

    rows
}

pub(super) fn render_headers_tab(av: &AppView, cx: &mut Context<AppView>) -> AnyElement {
    let req_id_str = av.state.session.read(cx).active_request_id().to_string();
    let ws_read = av.state.collection.read(cx);
    let active_r = ws_read.find_request(&req_id_str);
    let headers = active_r.map(|r| r.headers.clone()).unwrap_or_default();
    let readonly_rows = active_r
        .map(|r| inherited_readonly_rows(ws_read, r))
        .unwrap_or_default();
    let ed_up = av.state.session.clone();
    let ws_up = av.state.collection.clone();
    let ed_add = av.state.session.clone();
    let ws_add = av.state.collection.clone();
    let ed_del = av.state.session.clone();
    let ws_del = av.state.collection.clone();
    let req_id_k = req_id_str.clone();
    let req_id_a = req_id_str.clone();
    let req_id_d = req_id_str.clone();
    div()
        .size_full()
        .flex()
        .flex_col()
        .child(
            div().flex_1().child(
                KvTable::new(
                    format!("headers-kv-{}", req_id_str),
                    headers,
                    move |idx, k, v, enabled, _, cx| {
                        edit_active_request(&ws_up, &ed_up, &req_id_k, cx, |r| {
                            r.update_header(idx, k, v, enabled);
                        });
                    },
                    move |_, cx| {
                        edit_active_request(&ws_add, &ed_add, &req_id_a, cx, |r| {
                            r.add_header();
                        });
                    },
                    move |idx, _, cx| {
                        edit_active_request(&ws_del, &ed_del, &req_id_d, cx, |r| {
                            r.remove_header(idx);
                        });
                    },
                )
                .readonly_rows(readonly_rows)
                .env_state(av.state.env.clone())
                .modal_state(av.state.modal.clone()),
            ),
        )
        .into_any_element()
}
