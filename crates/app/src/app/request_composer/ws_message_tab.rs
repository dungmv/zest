use crate::components::message_toolbar;
use crate::state::{RequestSessionState, WorkspaceDataState};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Textarea, TextareaState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::{div, px, App, Entity, IntoElement, ParentElement, Styled};
use std::rc::Rc;
use zest_core::models::WsMessageFormat;

/// View callback routed through `AppView`; send needs `Window` to clear the
/// textarea after a successful send.
type SendMessageCallback = Rc<dyn Fn(&mut gpui_kit::Window, &mut App)>;

pub(super) struct WsMessageTabContext<'a> {
    pub workspace_state: &'a Entity<WorkspaceDataState>,
    pub editor_state: &'a Entity<RequestSessionState>,
    /// Session-only outgoing message box (never persisted).
    pub message_textarea: &'a Entity<TextareaState>,
    pub on_send_message: SendMessageCallback,
    pub on_ping: Rc<dyn Fn(&mut App)>,
}

/// WebSocket Message tab, styled after the HTTP Body tab: a Format
/// (Text/JSON) dropdown on the left and Ping/Send on the right, above the
/// outgoing message textarea.
pub(super) fn render_ws_message_tab(
    msg_format: WsMessageFormat,
    ctx: WsMessageTabContext,
    cx: &mut App,
) -> impl IntoElement {
    let t = theme();
    let ws_menu = ctx.workspace_state.clone();
    let ed_menu = ctx.editor_state.clone();
    let req_id_menu = ctx.editor_state.read(cx).active_request_id().to_string();
    let on_ping = ctx.on_ping.clone();
    let on_send = ctx.on_send_message.clone();

    div()
        .size_full()
        .flex()
        .flex_col()
        .overflow_hidden()
        // Toolbar: [Format ▾] .................. [Ping] [Send]
        .child(
            message_toolbar()
                .gap(px(6.))
                .child(
                    div().flex().items_center().flex_1().min_w(px(0.)).child(
                        Button::new("btn-ws-format-dropdown")
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
                                            .child(msg_format.as_str()),
                                    )
                                    .child(
                                        Icon::new(IconName::ChevronDown)
                                            .size(px(12.))
                                            .text_color(t.text_muted),
                                    ),
                            )
                            .dropdown_menu(move |menu, _window, _cx| {
                                let mut m = menu;
                                let ws_text = ws_menu.clone();
                                let ed_text = ed_menu.clone();
                                let id_text = req_id_menu.clone();
                                m = m.item(
                                    PopupMenuItem::new("Text")
                                        .checked(msg_format.is_text())
                                        .on_click(move |_, _, cx| {
                                            crate::state::edit_active_request(
                                                &ws_text,
                                                &ed_text,
                                                &id_text,
                                                cx,
                                                |r| {
                                                    r.set_ws_message_format(WsMessageFormat::Text);
                                                },
                                            );
                                        }),
                                );
                                let ws_json = ws_menu.clone();
                                let ed_json = ed_menu.clone();
                                let id_json = req_id_menu.clone();
                                m = m.item(
                                    PopupMenuItem::new("JSON")
                                        .checked(msg_format.is_json())
                                        .on_click(move |_, _, cx| {
                                            crate::state::edit_active_request(
                                                &ws_json,
                                                &ed_json,
                                                &id_json,
                                                cx,
                                                |r| {
                                                    r.set_ws_message_format(WsMessageFormat::Json);
                                                },
                                            );
                                        }),
                                );
                                m
                            }),
                    ),
                )
                .child(
                    Button::new("btn-ws-ping")
                        .ghost()
                        .xsmall()
                        .label("Ping")
                        .on_click(move |_, _, cx| {
                            (on_ping)(cx);
                        }),
                )
                .child(
                    Button::new("btn-ws-send")
                        .primary()
                        .xsmall()
                        .label("Send")
                        .on_click(move |_, window, cx| {
                            (on_send)(window, cx);
                        }),
                ),
        )
        // Outgoing message editor. Content is session-only, never persisted.
        .child(
            div().flex_1().min_h(px(0.)).overflow_hidden().child(
                Textarea::new(ctx.message_textarea)
                    .appearance(false)
                    .bordered(false)
                    .h(gpui_kit::relative(1.)),
            ),
        )
}
