use super::ScriptEditorKind;
use crate::theme::theme;
use gpui_kit::component::input::{Editor, EditorState as CodeEditorState};
use gpui_kit::component::scroll::{Scrollbar, ScrollbarMode};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, App, Entity, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement as _, Styled, Window,
};
use std::rc::Rc;

#[allow(clippy::type_complexity)]
#[derive(IntoElement)]
pub struct ScriptEditor {
    editor: Entity<CodeEditorState>,
    kind: ScriptEditorKind,
    error_message: Option<String>,
    on_insert_snippet: Rc<dyn Fn(&str, &mut Window, &mut App)>,
    pub(crate) split_layout_vertical: bool,
}

impl ScriptEditor {
    pub fn new(
        editor: Entity<CodeEditorState>,
        kind: ScriptEditorKind,
        error_message: Option<String>,
        on_insert_snippet: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            editor,
            kind,
            error_message,
            on_insert_snippet: Rc::new(on_insert_snippet),
            split_layout_vertical: false,
        }
    }

    pub fn split_layout_vertical(mut self, vertical: bool) -> Self {
        self.split_layout_vertical = vertical;
        self
    }
}

impl RenderOnce for ScriptEditor {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();
        let kind = self.kind;
        let on_insert = self.on_insert_snippet.clone();

        let scroll_handle = window.use_keyed_state(
            SharedString::from(match kind {
                ScriptEditorKind::PreRequest => "snippets-top-scroll-pre",
                ScriptEditorKind::Tests => "snippets-top-scroll-tests",
            }),
            cx,
            |_window, _cx| gpui_kit::ScrollHandle::new(),
        );
        let scr_h = scroll_handle.read(cx).clone();

        let snippets: Vec<(&'static str, &'static str)> = match kind {
            ScriptEditorKind::PreRequest => vec![
                (
                    "Set Timestamp Header",
                    "z.request.headers.add(\"X-Timestamp\", Date.now().toString());\n",
                ),
                (
                    "HMAC-SHA256 Signature",
                    "const sig = z.crypto.hmacSha256(\"my_secret\", Date.now().toString());\nz.request.headers.add(\"X-Signature\", sig);\n",
                ),
                (
                    "Set Env Variable",
                    "z.environment.set(\"custom_key\", \"custom_value\");\n",
                ),
                (
                    "Mutate JSON Body",
                    "const body = z.request.body.json();\nif (body) {\n    body.timestamp = Date.now();\n    z.request.body.setJson(body);\n}\n",
                ),
            ],
            ScriptEditorKind::Tests => vec![
                (
                    "Status code: 200",
                    "z.test(\"Status code is 200\", () => {\n    z.assert(z.response.status === 200, \"Expected 200 OK\");\n});\n",
                ),
                (
                    "Response time < 500ms",
                    "z.test(\"Response time is under 500ms\", () => {\n    z.assert(z.response.time < 500, \"Too slow\");\n});\n",
                ),
                (
                    "JSON value check",
                    "z.test(\"Verify JSON property\", () => {\n    const data = z.response.json();\n    z.assert(data !== null, \"Response should be JSON\");\n});\n",
                ),
                (
                    "Save Token to Env",
                    "// Request Chaining: extract token from response\nconst data = z.response.json();\nif (data && data.token) {\n    z.environment.set(\"access_token\", data.token);\n}\n",
                ),
            ],
        };

        let snippet_buttons: Vec<_> = snippets
            .into_iter()
            .enumerate()
            .map(|(idx, (label, code))| {
                let insert_cb = on_insert.clone();
                let snippet_code = code;
                div()
                    .id(format!(
                        "snippet-btn-{}-{}",
                        match kind {
                            ScriptEditorKind::PreRequest => "pre",
                            ScriptEditorKind::Tests => "test",
                        },
                        idx
                    ))
                    .flex_shrink_0()
                    .px(px(8.))
                    .py(px(if self.split_layout_vertical { 5. } else { 3. }))
                    .rounded(px(4.))
                    .bg(t.bg_editor)
                    .border_1()
                    .border_color(t.border)
                    .text_xs()
                    .text_color(t.text_primary)
                    .hover(|s| s.bg(t.bg_hover).border_color(t.border_focus))
                    .cursor_pointer()
                    .on_click(move |_, window, cx| {
                        insert_cb(snippet_code, window, cx);
                    })
                    .child(label)
            })
            .collect();

        let workspace = if self.split_layout_vertical {
            // Horizontal split (Top: Composer / Bottom: Response): snippets on the RIGHT (vertical column)
            div()
                .flex_1()
                .flex()
                .overflow_hidden()
                .child(
                    // Left: Code Editor Area
                    div().flex_1().h_full().overflow_hidden().child(
                        Editor::new(&self.editor)
                            .appearance(false)
                            .bordered(false)
                            .h(gpui_kit::relative(1.)),
                    ),
                )
                // Right: Snippets Toolbar (vertical column)
                .child(
                    div()
                        .w(px(200.))
                        .h_full()
                        .flex_shrink_0()
                        .border_l_1()
                        .border_color(t.border)
                        .bg(t.bg_panel)
                        .p(px(10.))
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui_kit::FontWeight::BOLD)
                                .text_color(t.text_secondary)
                                .mb(px(4.))
                                .child("SNIPPETS"),
                        )
                        .children(snippet_buttons),
                )
        } else {
            // Vertical split (Left: Composer / Right: Response columns): snippets on TOP in a single horizontal row with scroll
            div()
                .flex_1()
                .flex()
                .flex_col()
                .overflow_hidden()
                .child(
                    // Top: Snippets Toolbar (single horizontal row with scroll)
                    div()
                        .w_full()
                        .flex_shrink_0()
                        .border_b_1()
                        .border_color(t.border)
                        .bg(t.bg_panel)
                        .h(px(38.))
                        .px(px(8.))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.))
                        .overflow_hidden()
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_xs()
                                .font_weight(gpui_kit::FontWeight::BOLD)
                                .text_color(t.text_secondary)
                                .child("SNIPPETS:"),
                        )
                        .child(
                            div()
                                .id("snippets-top-scroll-container")
                                .flex_1()
                                .min_w(px(0.))
                                .h_full()
                                .relative()
                                .child(
                                    div()
                                        .id("snippets-top-scroll-track")
                                        .size_full()
                                        .track_scroll(&scr_h)
                                        .overflow_x_scroll()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(6.))
                                        .pr(px(24.))
                                        .children(snippet_buttons)
                                        .child(div().w(px(24.)).h(px(1.)).flex_shrink_0()),
                                )
                                .child(div().absolute().inset_0().child(
                                    Scrollbar::horizontal(&scr_h).mode(ScrollbarMode::Always),
                                )),
                        ),
                )
                .child(
                    // Bottom: Code Editor Area
                    div().flex_1().w_full().overflow_hidden().child(
                        Editor::new(&self.editor)
                            .appearance(false)
                            .bordered(false)
                            .h(gpui_kit::relative(1.)),
                    ),
                )
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(t.bg_editor)
            // Error banner if any
            .when_some(self.error_message, |this, err| {
                this.child(
                    div()
                        .px(px(12.))
                        .py(px(6.))
                        .bg(gpui_kit::Hsla {
                            a: 0.15,
                            ..t.method_delete
                        })
                        .border_b_1()
                        .border_color(gpui_kit::Hsla {
                            a: 0.35,
                            ..t.method_delete
                        })
                        .text_xs()
                        .text_color(t.method_delete)
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child("! Script Error:")
                        .child(err),
                )
            })
            // Main workspace
            .child(workspace)
    }
}
