use crate::app::AppView;
use crate::components::format_json_editor;
use crate::components::message_toolbar;
use crate::state::RequestSessionState;
use crate::theme::theme;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{
    Editor, EditorState as CodeEditorState, GutterDecorationCollection, Input, InputState,
};
use gpui_kit::component::Sizable as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, AnyElement, App, Context, Entity, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled, Window,
};
use std::rc::Rc;

/// AppView-facing entry: reads the active request and wires the callbacks.
pub(super) fn render_query_tab(
    av: &AppView,
    window: &mut Window,
    cx: &mut Context<AppView>,
) -> AnyElement {
    div()
        .size_full()
        .child({
            let view_fetch = cx.entity().clone();
            render_graphql_query_tab(
                GraphQLQueryTabContext {
                    editor_state: &av.state.session,
                    query_editor: &av.composer_inputs.graphql.query_editor,
                    variables_editor: &av.composer_inputs.graphql.variables_editor,
                    variables_marks: &av.composer_inputs.graphql.variables_marks,
                    diagnostics_hover: &av.composer_inputs.graphql.diagnostics_hover,
                    operation_name_input: &av.composer_inputs.graphql.operation_name_input,
                    on_fetch_schema: Rc::new(move |cx| {
                        view_fetch.update(cx, |view, cx| view.fetch_graphql_schema(cx));
                    }),
                },
                window,
                cx,
            )
        })
        .into_any_element()
}

pub(super) struct GraphQLQueryTabContext<'a> {
    pub editor_state: &'a Entity<RequestSessionState>,
    pub query_editor: &'a Entity<CodeEditorState>,
    pub variables_editor: &'a Entity<CodeEditorState>,
    pub variables_marks: &'a GutterDecorationCollection,
    pub diagnostics_hover: &'a Rc<std::cell::Cell<bool>>,
    pub operation_name_input: &'a Entity<InputState>,
    /// Invokes `Runner::fetch_graphql_schema` for the active request.
    pub on_fetch_schema: Rc<dyn Fn(&mut App)>,
}

/// GraphQL Query tab, styled after the gRPC Message tab: a toolbar with the
/// optional operation name and schema actions, above the dual-pane
/// Query (top) / Variables (bottom) editors.
pub(super) fn render_graphql_query_tab(
    ctx: GraphQLQueryTabContext,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let _ = window;
    let t = theme();
    let schema_loaded = ctx.editor_state.read(cx).graphql_schema.is_some();

    let ed_format = ctx.editor_state.clone();
    let variables_editor = ctx.variables_editor.clone();
    let on_fetch_schema = ctx.on_fetch_schema.clone();

    div()
        .size_full()
        .flex()
        .flex_col()
        .overflow_hidden()
        // Toolbar: [Operation name] ................ [Format JSON] [Fetch Schema]
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
                            div()
                                .flex_1()
                                .max_w(px(280.))
                                .min_w(px(0.))
                                .h(px(24.))
                                .px(px(6.))
                                .rounded(px(4.))
                                .bg(t.bg_input)
                                .border_1()
                                .border_color(t.border)
                                .flex()
                                .items_center()
                                .child(
                                    Input::new(ctx.operation_name_input)
                                        .appearance(false)
                                        .bordered(false),
                                ),
                        ),
                )
                .child(
                    Button::new("btn-format-variables-graphql")
                        .ghost()
                        .xsmall()
                        .label("Format JSON")
                        .on_click(move |_, window, cx: &mut App| {
                            format_json_editor(&variables_editor, &ed_format, window, cx);
                        }),
                )
                .child(
                    Button::new("btn-fetch-graphql-schema")
                        .ghost()
                        .xsmall()
                        .label("Fetch Schema")
                        .on_click(move |_, _, cx| {
                            (on_fetch_schema)(cx);
                        }),
                ),
        )
        // Query editor (top pane)
        .child(
            div()
                .flex_1()
                .min_h(px(0.))
                .flex()
                .flex_col()
                .px(px(12.))
                .pt(px(8.))
                .gap(px(4.))
                .child(section_label("Query", None))
                .child(
                    div()
                        .flex_1()
                        .min_h(px(0.))
                        .overflow_hidden()
                        .border_1()
                        .border_color(t.border)
                        .rounded(px(6.))
                        .child(Editor::new(ctx.query_editor).h(gpui_kit::relative(1.))),
                ),
        )
        // Variables editor (bottom pane) — hovering enables the in-editor
        // JSON diagnostic (hover tooltip on the offending line).
        .child(
            div()
                .h(px(200.))
                .flex()
                .flex_col()
                .px(px(12.))
                .py(px(8.))
                .gap(px(4.))
                .child(section_label("Variables", schema_loaded.then_some("JSON")))
                .child(
                    div()
                        .id("graphql-variables-editor-pane")
                        .flex_1()
                        .min_h(px(0.))
                        .overflow_hidden()
                        .border_1()
                        .border_color(t.border)
                        .rounded(px(6.))
                        .on_hover({
                            let hover_cell = ctx.diagnostics_hover.clone();
                            let editor_hover = ctx.variables_editor.clone();
                            let marks_hover = ctx.variables_marks.clone();
                            move |hovered: &bool, _window, cx| {
                                let hovered = *hovered;
                                hover_cell.set(hovered);
                                let text = editor_hover.read(cx).value().to_string();
                                super::refresh_graphql_variables_validation(
                                    &editor_hover,
                                    &marks_hover,
                                    hovered,
                                    &text,
                                    cx,
                                );
                            }
                        })
                        .child(Editor::new(ctx.variables_editor).h(gpui_kit::relative(1.))),
                ),
        )
}

fn section_label(text: &str, hint: Option<&str>) -> gpui_kit::AnyElement {
    let t = theme();
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .child(
            div()
                .text_xs()
                .font_weight(gpui_kit::FontWeight::MEDIUM)
                .text_color(t.text_secondary)
                .child(text.to_string()),
        )
        .when_some(hint, |this, hint| {
            this.child(
                div()
                    .text_xs()
                    .text_color(t.text_muted)
                    .child(hint.to_string()),
            )
        })
        .into_any_element()
}
