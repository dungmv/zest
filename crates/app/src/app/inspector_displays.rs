use crate::state::ResponseState;
use gpui::{App, AppContext as _, Entity, Window};
use gpui_component::input::EditorState as CodeEditorState;
use std::rc::Rc;

type HighlighterFactory =
    Rc<dyn Fn(&str) -> Option<Box<dyn gpui_component::input::InputHighlighter>>>;

pub struct InspectorDisplays {
    pub response_editor: Entity<CodeEditorState>,
    pub request_body_editor: Entity<CodeEditorState>,
}

impl InspectorDisplays {
    pub fn new(window: &mut Window, highlighter_factory: HighlighterFactory, cx: &mut App) -> Self {
        let response_editor = cx.new(|cx| {
            let mut state = CodeEditorState::new(window, cx);
            state.set_highlighter_factory(highlighter_factory.clone(), cx);
            state
                .language("json")
                .line_number(true)
                .folding(true)
                .soft_wrap(false)
        });

        let request_body_editor = cx.new(|cx| {
            let mut state = CodeEditorState::new(window, cx);
            state.set_highlighter_factory(highlighter_factory, cx);
            state.language("json").line_number(true).folding(false)
        });

        Self {
            response_editor,
            request_body_editor,
        }
    }

    pub fn sync_response_editor(
        &mut self,
        response_state: &Entity<ResponseState>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (display_text, is_json, is_xml) = {
            let resp_s = response_state.read(cx);
            if let Some(fmt) = &resp_s.formatted_body {
                (fmt.display_text.to_string(), fmt.is_json, fmt.is_xml)
            } else if let Some(resp) = &resp_s.current_response {
                let text = String::from_utf8_lossy(&resp.body).to_string();
                let is_j = resp.is_json()
                    || text.trim_start().starts_with('{')
                    || text.trim_start().starts_with('[');
                let is_x = !is_j
                    && (resp.is_xml()
                        || text.trim_start().starts_with("<?xml")
                        || (text.trim_start().starts_with('<') && text.trim_end().ends_with('>')));
                (text, is_j, is_x)
            } else {
                (String::new(), false, false)
            }
        };

        let lang = if is_json {
            "json"
        } else if is_xml {
            "xml"
        } else {
            "plaintext"
        };

        self.response_editor.update(cx, |txt, cx| {
            txt.set_value(display_text, window, cx);
            txt.set_highlighter(lang, cx);
            txt.set_readonly(true, cx);
        });
    }

    pub fn sync_sent_request_preview(
        &mut self,
        response_state: &Entity<ResponseState>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (body_text, lang) = {
            let resp_s = response_state.read(cx);
            if let Some(resp) = &resp_s.current_response {
                if let Some(sent) = &resp.request_sent {
                    let (body_text, lang) = if let Some(preview) = &sent.body_preview {
                        let is_j = preview.trim_start().starts_with('{')
                            || preview.trim_start().starts_with('[');
                        let text = if is_j {
                            serde_json::from_str::<serde_json::Value>(preview)
                                .map(|v| {
                                    serde_json::to_string_pretty(&v).unwrap_or(preview.clone())
                                })
                                .unwrap_or_else(|_| preview.clone())
                        } else {
                            preview.clone()
                        };
                        (text, if is_j { "json" } else { "plaintext" })
                    } else {
                        (String::new(), "plaintext")
                    };
                    (Some(body_text), Some(lang))
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            }
        };

        if let (Some(body_text), Some(lang)) = (body_text, lang) {
            self.request_body_editor.update(cx, |txt, cx| {
                if txt.value() != body_text {
                    txt.set_value(body_text, window, cx);
                    txt.set_highlighter(lang, cx);
                    txt.set_readonly(true, cx);
                }
            });
        }
    }
}
