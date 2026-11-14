use crate::state::{RequestSessionState, ToastState, WorkspaceDataState};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Editor, EditorState as CodeEditorState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::{div, px, App, Entity, IntoElement, ParentElement, Styled, Window};
use zest_core::models::GrpcPayload;

pub struct GrpcMessageTabContext<'a> {
    pub workspace_state: &'a Entity<WorkspaceDataState>,
    pub editor_state: &'a Entity<RequestSessionState>,
    pub toast_state: &'a Entity<ToastState>,
    pub message_editor: &'a Entity<CodeEditorState>,
}

/// gRPC Message tab, styled after the HTTP Body tab: a toolbar with the
/// Service/Method picker on the left and a JSON formatter on the right,
/// above the JSON message editor.
pub fn render_grpc_message_tab(
    payload: &GrpcPayload,
    ctx: GrpcMessageTabContext,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let _ = window;
    let t = theme();
    let schema = ctx
        .editor_state
        .read(cx)
        .grpc_schema
        .clone()
        .unwrap_or_default();

    let selection = if payload.service.is_empty() && payload.method.is_empty() {
        "Select service/method…".to_string()
    } else if payload.method.is_empty() {
        payload.service.clone()
    } else {
        format!("{}/{}", payload.service, payload.method)
    };

    let ws_menu = ctx.workspace_state.clone();
    let ed_menu = ctx.editor_state.clone();
    let req_id_menu = ctx.editor_state.read(cx).active_request_id().to_string();
    let current_service = payload.service.clone();
    let current_method = payload.method.clone();

    let ed_format = ctx.editor_state.clone();
    let message_editor = ctx.message_editor.clone();
    let toast_state = ctx.toast_state.clone();

    // Live validity for the toolbar check: JSON syntax first, then schema
    // conformance when the selection resolves against the fetched pool.
    // Error details are surfaced as editor diagnostics on the offending
    // line, not here.
    let raw = ctx.message_editor.read(cx).value().to_string();
    let is_valid = raw.trim().is_empty()
        || crate::components::proto_json_completion::validate_message(
            &schema.pool,
            &payload.service,
            &payload.method,
            &raw,
        )
        .is_ok();

    div()
        .size_full()
        .flex()
        .flex_col()
        .overflow_hidden()
        // Toolbar: [Service/Method ▾] ................. [Format JSON]
        .child(
            div()
                .h(px(32.))
                .px(px(8.))
                .border_b_1()
                .border_color(t.border)
                .bg(t.bg_editor)
                .flex()
                .items_center()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .flex_1()
                        .min_w(px(0.))
                        .child(
                            Button::new("btn-grpc-target-dropdown")
                                .secondary()
                                .compact()
                                .px(px(8.))
                                .py(px(2.))
                                .h(px(24.))
                                .rounded(px(4.))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(6.))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui_kit::FontWeight::MEDIUM)
                                                .text_color(t.text_primary)
                                                .child(selection),
                                        )
                                        .child(
                                            Icon::new(IconName::ChevronDown)
                                                .size(px(12.))
                                                .text_color(t.text_muted),
                                        ),
                                )
                                .dropdown_menu(move |menu, _window, _cx| {
                                    let mut m = menu;
                                    if schema.services.is_empty() {
                                        return m.item(
                                    PopupMenuItem::new(
                                        "No schema loaded — use Fetch Schema in the Proto tab",
                                    )
                                    .on_click(|_, _, _| {}),
                                );
                                    }
                                    for (service, methods) in &schema.services {
                                        m = m.separator().label(service.clone());
                                        for info in methods {
                                            let selected = *service == current_service
                                                && info.name == current_method;
                                            let ws_set = ws_menu.clone();
                                            let ed_set = ed_menu.clone();
                                            let id = req_id_menu.clone();
                                            let svc_name = service.clone();
                                            let method_name = info.name.clone();
                                            m = m.item(
                                                PopupMenuItem::new(info.label())
                                                    .checked(selected)
                                                    .on_click(move |_, _, cx| {
                                                        ws_set.update(cx, |ws, _| {
                                                            ws.update_active_request(&id, |r| {
                                                                let payload = r.grpc_mut();
                                                                payload.service = svc_name.clone();
                                                                payload.method =
                                                                    method_name.clone();
                                                            });
                                                        });
                                                        ed_set
                                                            .update(cx, |ed, cx| ed.mark_dirty(cx));
                                                    }),
                                            );
                                        }
                                    }
                                    m
                                }),
                        ),
                )
                // Validity check: a green check when the message parses and
                // conforms to the schema (details live on the editor line).
                .child(if is_valid {
                    div()
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .text_color(t.status_2xx)
                        .child("✓")
                } else {
                    div()
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .text_color(t.status_5xx)
                        .child("✗")
                })
                .child(
                    Button::new("btn-format-json-grpc")
                        .ghost()
                        .xsmall()
                        .label("Format JSON")
                        .on_click(move |_, window, cx: &mut App| {
                            let raw = message_editor.read(cx).value().to_string();
                            match serde_json::from_str::<serde_json::Value>(&raw) {
                                Ok(val) => {
                                    if let Ok(pretty) = serde_json::to_string_pretty(&val) {
                                        message_editor.update(cx, |txt, cx| {
                                            txt.set_value(pretty, window, cx);
                                        });
                                        ed_format.update(cx, |ed, cx| ed.mark_dirty(cx));
                                    }
                                }
                                Err(e) => {
                                    toast_state.update(cx, |toast, cx| {
                                        toast.push_error("Invalid JSON", e.to_string(), cx);
                                    });
                                }
                            }
                        }),
                ),
        )
        // Message JSON editor
        .child(
            div()
                .flex_1()
                .min_h(px(0.))
                .overflow_hidden()
                .child(Editor::new(ctx.message_editor).h(gpui_kit::relative(1.))),
        )
}
