pub mod binary;
pub mod form;
pub mod json;
pub mod multipart;
pub mod xml;

pub use binary::*;
pub use form::*;
pub use json::*;
pub use multipart::*;
pub use xml::*;

use crate::components::format_jsonc_editor;
use crate::components::message_toolbar;
use crate::state::{
    edit_active_request, CollectionState, EnvironmentState, ModalState, RequestSessionState,
};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::empty::{
    Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyMediaVariant, EmptyTitle,
};
use gpui_kit::component::input::{Editor, EditorState as CodeEditorState, Textarea, TextareaState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::Icon;
use gpui_kit::component::Sizable as _;
use gpui_kit::{div, px, App, Entity, IntoElement, ParentElement, RenderOnce, Styled, Window};
use zest_core::models::{KeyValuePair, MultipartItem, RequestBody};

pub(crate) fn clamp_char_boundary_left(s: &str, mut idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

pub(crate) fn clamp_char_boundary_right(s: &str, mut idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    while idx < s.len() && !s.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

#[derive(IntoElement)]
pub struct BodyEditor {
    editor_state: Entity<RequestSessionState>,
    json_editor: Entity<CodeEditorState>,
    xml_editor: Entity<CodeEditorState>,
    raw_textarea: Entity<TextareaState>,
    workspace_state: Option<Entity<CollectionState>>,
    env_state: Option<Entity<EnvironmentState>>,
    modal_state: Option<Entity<ModalState>>,
}

impl BodyEditor {
    pub fn new(
        editor_state: Entity<RequestSessionState>,
        json_editor: Entity<CodeEditorState>,
        xml_editor: Entity<CodeEditorState>,
        raw_textarea: Entity<TextareaState>,
    ) -> Self {
        Self {
            editor_state,
            json_editor,
            xml_editor,
            raw_textarea,
            workspace_state: None,
            env_state: None,
            modal_state: None,
        }
    }

    pub fn workspace_state(mut self, ws: Entity<CollectionState>) -> Self {
        self.workspace_state = Some(ws);
        self
    }

    pub fn env_state(mut self, env: Entity<EnvironmentState>) -> Self {
        self.env_state = Some(env);
        self
    }

    pub fn modal_state(mut self, modal: Entity<ModalState>) -> Self {
        self.modal_state = Some(modal);
        self
    }
}

/// Per-frame snapshot of the request body: the heavy content variants
/// (Json/Raw) distill to a discriminant — their content lives in the editor
/// entities and is never read during render — while the small structured
/// variants are cloned for the renderer.
enum BodyView {
    None,
    Json,
    Raw { is_xml: bool },
    Form(Vec<KeyValuePair>),
    Multipart(Vec<MultipartItem>),
    Binary(String),
}

fn body_view(body: &RequestBody) -> BodyView {
    match body {
        RequestBody::None => BodyView::None,
        RequestBody::Json { .. } => BodyView::Json,
        RequestBody::Raw { content_type, .. } => BodyView::Raw {
            is_xml: content_type == "application/xml" || content_type == "text/xml",
        },
        RequestBody::FormUrlEncoded { params } => BodyView::Form(params.clone()),
        RequestBody::Multipart { items } => BodyView::Multipart(items.clone()),
        RequestBody::Binary { file_path } => BodyView::Binary(file_path.clone()),
    }
}

impl BodyEditor {
    #[allow(clippy::too_many_arguments)]
    fn render_type_dropdown(
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

    fn render_toolbar_action(
        &self,
        req_id_str: &str,
        is_json: bool,
        is_xml: bool,
        is_multipart: bool,
        is_form: bool,
    ) -> gpui_kit::AnyElement {
        if is_json {
            let ed_c = self.editor_state.clone();
            let j_text = self.json_editor.clone();
            Button::new("btn-format-json-body")
                .ghost()
                .xsmall()
                .label("Format JSON")
                .on_click(move |_, window, cx: &mut App| {
                    format_jsonc_editor(&j_text, &ed_c, window, cx);
                })
                .into_any_element()
        } else if is_xml {
            let ed_c = self.editor_state.clone();
            let xml_tx = self.xml_editor.clone();
            Button::new("btn-format-xml-body")
                .ghost()
                .xsmall()
                .label("Format XML")
                .on_click(move |_, window, cx| {
                    let raw = xml_tx.read(cx).value().to_string();
                    match format_xml(&raw) {
                        Ok(pretty) => {
                            xml_tx.update(cx, |txt, cx| {
                                txt.set_value(pretty.clone(), window, cx);
                            });
                            ed_c.update(cx, |state, cx| {
                                state.mark_dirty(cx);
                            });
                        }
                        Err(e) => {
                            crate::notify::error_in("XML Format Error", e, window, cx);
                        }
                    }
                })
                .into_any_element()
        } else if is_multipart {
            let ed_c = self.editor_state.clone();
            let ws_add = self.workspace_state.clone();
            let id_add = req_id_str.to_string();
            Button::new("btn-add-multipart-part")
                .ghost()
                .xsmall()
                .icon(IconName::Plus)
                .label("Add Part")
                .on_click(move |_, _, cx| {
                    if let Some(ws) = &ws_add {
                        ws.update(cx, |ws, _| {
                            ws.update_active_request(&id_add, |r| {
                                r.add_multipart_item();
                            });
                        });
                    }
                    ed_c.update(cx, |state, cx| {
                        state.mark_dirty(cx);
                    });
                })
                .into_any_element()
        } else if is_form {
            let ed_c = self.editor_state.clone();
            let ws_add = self.workspace_state.clone();
            let id_add = req_id_str.to_string();
            Button::new("btn-add-form-row")
                .ghost()
                .xsmall()
                .icon(IconName::Plus)
                .label("Add Row")
                .on_click(move |_, _, cx| {
                    if let Some(ws) = &ws_add {
                        ws.update(cx, |ws, _| {
                            ws.update_active_request(&id_add, |r| {
                                r.add_form_param();
                            });
                        });
                    }
                    ed_c.update(cx, |state, cx| {
                        state.mark_dirty(cx);
                    });
                })
                .into_any_element()
        } else {
            div().into_any_element()
        }
    }

    fn render_body_content(
        &self,
        req_id_str: &str,
        body_ref: Option<&BodyView>,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui_kit::AnyElement {
        match body_ref {
            None | Some(BodyView::None) => Empty::new()
                .header(
                    EmptyHeader::new()
                        .media(
                            EmptyMedia::new()
                                .with_variant(EmptyMediaVariant::Icon)
                                .child(Icon::new(IconName::FileText).size(px(24.))),
                        )
                        .title(EmptyTitle::new().child("This request has no body payload"))
                        .description(EmptyDescription::new().child(
                            "Select a format above (e.g. JSON, Form) to add request body data.",
                        )),
                )
                .into_any_element(),
            Some(BodyView::Json) => div()
                .size_full()
                .child(
                    Editor::new(&self.json_editor)
                        .appearance(false)
                        .bordered(false)
                        .h(gpui_kit::relative(1.)),
                )
                .into_any_element(),
            Some(BodyView::Form(pairs)) => render_form_editor(
                req_id_str,
                pairs,
                &self.editor_state,
                self.workspace_state.as_ref(),
                self.env_state.as_ref(),
                self.modal_state.as_ref(),
            ),
            Some(BodyView::Multipart(items)) => render_multipart_editor(
                MultipartContext {
                    req_id_str,
                    editor_state: &self.editor_state,
                    workspace_state: self.workspace_state.as_ref(),
                    env_state: self.env_state.as_ref(),
                    modal_state: self.modal_state.as_ref(),
                },
                items,
                window,
                cx,
            ),
            Some(BodyView::Raw { is_xml: true }) => div()
                .size_full()
                .child(
                    Editor::new(&self.xml_editor)
                        .appearance(false)
                        .bordered(false)
                        .h(gpui_kit::relative(1.)),
                )
                .into_any_element(),
            Some(BodyView::Raw { is_xml: false }) => div()
                .size_full()
                .child(
                    Textarea::new(&self.raw_textarea)
                        .appearance(false)
                        .bordered(false)
                        .h(gpui_kit::relative(1.)),
                )
                .into_any_element(),
            Some(BodyView::Binary(file_path)) => {
                let size_label = self.editor_state.read(cx).binary_file_size_label(file_path);
                render_binary_editor(
                    req_id_str,
                    file_path,
                    size_label,
                    &self.editor_state,
                    self.workspace_state.as_ref(),
                )
            }
        }
    }
}

impl RenderOnce for BodyEditor {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let req_id_str = self.editor_state.read(cx).active_request_id().to_string();
        let body_view = self.workspace_state.as_ref().and_then(|ws| {
            ws.read(cx)
                .find_request(&req_id_str)
                .map(|r| body_view(&r.body))
        });
        let body_ref = body_view.as_ref();

        let (is_none, is_json, is_xml, is_text, is_multipart, is_form, is_binary) = match body_ref {
            None | Some(BodyView::None) => (true, false, false, false, false, false, false),
            Some(BodyView::Json) => (false, true, false, false, false, false, false),
            Some(BodyView::Raw { is_xml: true }) => {
                (false, false, true, false, false, false, false)
            }
            Some(BodyView::Raw { is_xml: false }) => {
                (false, false, false, true, false, false, false)
            }
            Some(BodyView::Multipart(_)) => (false, false, false, false, true, false, false),
            Some(BodyView::Form(_)) => (false, false, false, false, false, true, false),
            Some(BodyView::Binary(_)) => (false, false, false, false, false, false, true),
        };

        let current_label = if is_none {
            "None"
        } else if is_json {
            "JSON"
        } else if is_xml {
            "XML"
        } else if is_text {
            "Text"
        } else if is_multipart {
            "Multipart"
        } else if is_form {
            "Form URL-Encoded"
        } else if is_binary {
            "Binary Stream"
        } else {
            "None"
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                message_toolbar()
                    .justify_between()
                    .child(self.render_type_dropdown(
                        &req_id_str,
                        current_label,
                        is_none,
                        is_json,
                        is_xml,
                        is_text,
                        is_multipart,
                        is_form,
                        is_binary,
                    ))
                    .child(self.render_toolbar_action(
                        &req_id_str,
                        is_json,
                        is_xml,
                        is_multipart,
                        is_form,
                    )),
            )
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    .child(self.render_body_content(&req_id_str, body_ref, window, cx)),
            )
    }
}
