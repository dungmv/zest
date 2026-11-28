use super::BodyEditor;
use crate::state::edit_active_request;
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::Icon;
use gpui_kit::{div, px, IntoElement, ParentElement, Styled};
use zest_core::models::{KeyValuePair, MultipartItem, RequestBody};

impl BodyEditor {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_type_dropdown(
        &self,
        req_id_str: &str,
        current_label: &'static str,
        is_none: bool,
        is_json: bool,
        is_xml: bool,
        is_text: bool,
        is_multipart: bool,
        is_form: bool,
        is_binary: bool,
    ) -> impl IntoElement {
        let t = theme();
        let ed_menu = self.editor_state.clone();
        let ws_menu = self.workspace_state.clone();
        let id_menu = req_id_str.to_string();
        Button::new("btn-body-type-dropdown")
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
                            .child(current_label),
                    )
                    .child(
                        Icon::new(IconName::ChevronDown)
                            .size(px(12.))
                            .text_color(t.text_muted),
                    ),
            )
            .dropdown_menu(move |menu, _window, _cx| {
                let mut m = menu;

                // 1. None
                let ed_none = ed_menu.clone();
                let ws_none = ws_menu.clone();
                let id_none = id_menu.clone();
                m = m.item(
                    PopupMenuItem::new("None")
                        .checked(is_none)
                        .on_click(move |_, _, cx| {
                            if let Some(ws) = &ws_none {
                                edit_active_request(ws, &ed_none, &id_none, cx, |r| {
                                    r.set_body_type(RequestBody::None);
                                    r.update_content_type_header(None);
                                });
                            }
                        }),
                );

                // 2. Text group: JSON, XML, Text
                m = m.separator().label("Text");

                let ed_json = ed_menu.clone();
                let id_json = id_menu.clone();
                let ws_json = ws_menu.clone();
                m = m.item(
                    PopupMenuItem::new("JSON")
                        .checked(is_json)
                        .on_click(move |_, _, cx| {
                            if let Some(ws) = &ws_json {
                                ws.update(cx, |ws, _| {
                                    ws.update_active_request(&id_json, |r| {
                                        let current_str = match &r.body {
                                            RequestBody::Json { content }
                                                if !content.trim().is_empty() =>
                                            {
                                                content.clone()
                                            }
                                            RequestBody::Raw { content, .. }
                                                if !content.trim().is_empty() =>
                                            {
                                                content.clone()
                                            }
                                            _ => "{\n  \n}".to_string(),
                                        };
                                        r.set_body_json(current_str);
                                        r.update_content_type_header(Some("application/json"));
                                    });
                                });
                            }
                            ed_json.update(cx, |state, cx| {
                                state.notify_body_changed(cx);
                            });
                        }),
                );

                let ed_xml = ed_menu.clone();
                let id_xml = id_menu.clone();
                let ws_xml = ws_menu.clone();
                m = m.item(
                    PopupMenuItem::new("XML")
                        .checked(is_xml)
                        .on_click(move |_, _, cx| {
                            if let Some(ws) = &ws_xml {
                                ws.update(cx, |ws, _| {
                                    ws.update_active_request(&id_xml, |r| {
                                        let current_str = match &r.body {
                                            RequestBody::Raw { content, .. }
                                                if !content.trim().is_empty() =>
                                            {
                                                content.clone()
                                            }
                                            _ => "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<root>\n  \n</root>".to_string(),
                                        };
                                        r.set_raw_body(current_str, "application/xml".to_string());
                                        r.update_content_type_header(Some("application/xml"));
                                    });
                                });
                            }
                            ed_xml.update(cx, |state, cx| {
                                state.notify_body_changed(cx);
                            });
                        }),
                );

                let ed_text = ed_menu.clone();
                let id_text = id_menu.clone();
                let ws_text = ws_menu.clone();
                m = m.item(
                    PopupMenuItem::new("Text")
                        .checked(is_text)
                        .on_click(move |_, _, cx| {
                            if let Some(ws) = &ws_text {
                                ws.update(cx, |ws, _| {
                                    ws.update_active_request(&id_text, |r| {
                                        let current_str = match &r.body {
                                            RequestBody::Raw { content, .. } => content.clone(),
                                            RequestBody::Json { content } => content.clone(),
                                            _ => "".to_string(),
                                        };
                                        r.set_raw_body(current_str, "text/plain".to_string());
                                        r.update_content_type_header(Some("text/plain"));
                                    });
                                });
                            }
                            ed_text.update(cx, |state, cx| {
                                state.notify_body_changed(cx);
                            });
                        }),
                );

                // 3. Form Data group: Multipart, Form URL-Encoded
                m = m.separator().label("Form Data");

                let ed_mp = ed_menu.clone();
                let id_mp = id_menu.clone();
                let ws_mp = ws_menu.clone();
                m = m.item(
                    PopupMenuItem::new("Multipart")
                        .checked(is_multipart)
                        .on_click(move |_, _, cx| {
                            if let Some(ws) = &ws_mp {
                                ws.update(cx, |ws, _| {
                                    ws.update_active_request(&id_mp, |r| {
                                        let current_items = match &r.body {
                                            RequestBody::Multipart { items }
                                                if !items.is_empty() =>
                                            {
                                                items.clone()
                                            }
                                            _ => vec![MultipartItem::default()],
                                        };
                                        r.set_body_type(RequestBody::multipart(current_items));
                                        r.update_content_type_header(Some("multipart/form-data"));
                                    });
                                });
                            }
                            ed_mp.update(cx, |state, cx| {
                                state.notify_body_changed(cx);
                            });
                        }),
                );

                let ed_form = ed_menu.clone();
                let id_form = id_menu.clone();
                let ws_form = ws_menu.clone();
                m = m.item(
                    PopupMenuItem::new("Form URL-Encoded")
                        .checked(is_form)
                        .on_click(move |_, _, cx| {
                            if let Some(ws) = &ws_form {
                                ws.update(cx, |ws, _| {
                                    ws.update_active_request(&id_form, |r| {
                                        let current_pairs = match &r.body {
                                            RequestBody::FormUrlEncoded { params }
                                                if !params.is_empty() =>
                                            {
                                                params.clone()
                                            }
                                            _ => vec![KeyValuePair::new("", "")],
                                        };
                                        r.set_body_type(RequestBody::form(current_pairs));
                                        r.update_content_type_header(Some(
                                            "application/x-www-form-urlencoded",
                                        ));
                                    });
                                });
                            }
                            ed_form.update(cx, |state, cx| {
                                state.notify_body_changed(cx);
                            });
                        }),
                );

                // 4. Binary group: Binary Stream
                m = m.separator().label("Binary");

                let ed_bin = ed_menu.clone();
                let id_bin = id_menu.clone();
                let ws_bin = ws_menu.clone();
                m = m.item(
                    PopupMenuItem::new("Binary Stream")
                        .checked(is_binary)
                        .on_click(move |_, _, cx| {
                            if let Some(ws) = &ws_bin {
                                ws.update(cx, |ws, _| {
                                    ws.update_active_request(&id_bin, |r| {
                                        let current_path = match &r.body {
                                            RequestBody::Binary { file_path } => file_path.clone(),
                                            _ => "".to_string(),
                                        };
                                        r.set_body_type(RequestBody::Binary {
                                            file_path: current_path,
                                        });
                                        r.update_content_type_header(Some(
                                            "application/octet-stream",
                                        ));
                                    });
                                });
                            }
                            ed_bin.update(cx, |state, cx| {
                                state.notify_body_changed(cx);
                            });
                        }),
                );

                m
            })
    }
}
