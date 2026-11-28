mod binary;
mod form;
mod multipart;
mod tab;
mod type_menu;

pub(super) use tab::render_body_tab;

use self::{
    binary::render_binary_editor,
    form::render_form_editor,
    multipart::{render_multipart_editor, MultipartContext},
};
use crate::components::common::{editor::format_xml, format_jsonc_editor, message_toolbar};
use crate::editor_scheme::editor_scheme;
use crate::state::{CollectionState, EnvironmentState, ModalState, RequestSessionState};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::empty::{
    Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyMediaVariant, EmptyTitle,
};
use gpui_kit::component::input::{Editor, EditorState as CodeEditorState, Textarea, TextareaState};
use gpui_kit::component::Icon;
use gpui_kit::component::Sizable as _;
use gpui_kit::{div, px, App, Entity, IntoElement, ParentElement, RenderOnce, Styled, Window};
use zest_core::models::{KeyValuePair, MultipartItem, RequestBody};

#[derive(IntoElement)]
struct BodyEditor {
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
        let t = theme();
        let scheme = editor_scheme();
        match body_ref {
            None | Some(BodyView::None) => Empty::new()
                .header(
                    EmptyHeader::new()
                        .media(
                            EmptyMedia::new()
                                .with_variant(EmptyMediaVariant::Icon)
                                .child(
                                    Icon::new(IconName::FileText)
                                        .size(px(24.))
                                        .text_color(t.icon_muted),
                                ),
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
                        .text_color(scheme.foreground)
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
                        .text_color(scheme.foreground)
                        .appearance(false)
                        .bordered(false)
                        .h(gpui_kit::relative(1.)),
                )
                .into_any_element(),
            Some(BodyView::Raw { is_xml: false }) => div()
                .size_full()
                .child(
                    Textarea::new(&self.raw_textarea)
                        .text_color(scheme.foreground)
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
