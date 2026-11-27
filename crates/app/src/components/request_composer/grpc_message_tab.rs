use crate::components::format_json_editor;
use crate::components::message_toolbar;
use crate::state::{edit_active_request, CollectionState, RequestSessionState};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{
    Editor, EditorState as CodeEditorState, GutterDecorationCollection,
};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::{
    div, px, App, Entity, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled, Window,
};
use zest_core::models::GrpcPayload;

pub(super) struct GrpcMessageTabContext<'a> {
    pub workspace_state: &'a Entity<CollectionState>,
    pub editor_state: &'a Entity<RequestSessionState>,
    pub message_editor: &'a Entity<CodeEditorState>,
    pub error_marks: &'a GutterDecorationCollection,
    pub diagnostics_hover: &'a std::rc::Rc<std::cell::Cell<bool>>,
}

/// gRPC Message tab, styled after the HTTP Body tab: a toolbar with the
/// Service/Method picker on the left and a JSON formatter on the right,
/// above the JSON message editor.
pub(super) fn render_grpc_message_tab(
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

    div()
        .size_full()
        .flex()
        .flex_col()
        .overflow_hidden()
        // Toolbar: [Service/Method ▾] ................. [Format JSON]
        .child(
            message_toolbar()
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
                                                        edit_active_request(
                                                            &ws_set,
                                                            &ed_set,
                                                            &id,
                                                            cx,
                                                            |r| {
                                                                let payload = r.grpc_mut();
                                                                payload.service = svc_name.clone();
                                                                payload.method =
                                                                    method_name.clone();
                                                            },
                                                        );
                                                    }),
                                            );
                                        }
                                    }
                                    m
                                }),
                        ),
                )
                .child(
                    Button::new("btn-format-json-grpc")
                        .ghost()
                        .xsmall()
                        .label("Format JSON")
                        .on_click(move |_, window, cx: &mut App| {
                            format_json_editor(&message_editor, &ed_format, window, cx);
                        }),
                ),
        )
        // Message JSON editor. Hovering the pane enables the in-editor
        // error diagnostic (hover tooltip on the offending line); leaving
        // clears it so nothing floats over the content while typing.
        .child(
            div()
                .id("grpc-message-editor-pane")
                .flex_1()
                .min_h(px(0.))
                .overflow_hidden()
                .on_hover({
                    let hover_cell = ctx.diagnostics_hover.clone();
                    let ws_hover = ctx.workspace_state.clone();
                    let ed_hover = ctx.editor_state.clone();
                    let editor_hover = ctx.message_editor.clone();
                    let marks_hover = ctx.error_marks.clone();
                    move |hovered: &bool, _window, cx| {
                        let hovered = *hovered;
                        hover_cell.set(hovered);
                        let text = editor_hover.read(cx).value().to_string();
                        super::refresh_grpc_validation(
                            &editor_hover,
                            &marks_hover,
                            hovered,
                            &text,
                            &ed_hover,
                            &ws_hover,
                            cx,
                        );
                    }
                })
                .child(Editor::new(ctx.message_editor).h(gpui_kit::relative(1.))),
        )
}
