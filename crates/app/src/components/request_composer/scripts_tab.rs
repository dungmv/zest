use crate::app::AppView;
use crate::components::script_editor::{ScriptEditor, ScriptEditorKind};
use gpui_kit::component::input::EditorState as CodeEditorState;
use gpui_kit::{div, AnyElement, App, Context, Entity, IntoElement, ParentElement, Styled, Window};

pub(super) fn render_script_tab(
    av: &AppView,
    editor: &Entity<CodeEditorState>,
    kind: ScriptEditorKind,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let script_err = av.state.session.read(cx).script_error.clone();
    let is_vert = crate::settings::AppPreferences::read(cx).split_layout_vertical;
    div()
        .size_full()
        .child(
            ScriptEditor::new(
                editor.clone(),
                kind,
                script_err,
                snippet_appender(editor.clone()),
            )
            .split_layout_vertical(is_vert),
        )
        .into_any_element()
}

/// Appends a snippet-catalog selection to the script editor, separated by a
/// newline when the editor already has content (shared by the pre-request and
/// tests tabs).
fn snippet_appender(
    editor: Entity<CodeEditorState>,
) -> impl Fn(&str, &mut Window, &mut App) + 'static {
    move |snippet, window, cx| {
        editor.update(cx, |t, cx| {
            let cur = t.value().to_string();
            let new_val = if cur.is_empty() {
                snippet.to_string()
            } else if cur.ends_with('\n') {
                format!("{}{}", cur, snippet)
            } else {
                format!("{}\n{}", cur, snippet)
            };
            t.set_value(new_val, window, cx);
        });
    }
}
