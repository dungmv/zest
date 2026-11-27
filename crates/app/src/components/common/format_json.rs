use crate::state::RequestSessionState;
use gpui_kit::component::input::EditorState as CodeEditorState;
use gpui_kit::{App, Entity, Window};

pub fn format_json_editor(
    editor: &Entity<CodeEditorState>,
    session_state: &Entity<RequestSessionState>,
    window: &mut Window,
    cx: &mut App,
) {
    format_editor(editor, session_state, window, cx, |raw| {
        let value = serde_json::from_str::<serde_json::Value>(raw)?;
        serde_json::to_string_pretty(&value)
    });
}

pub fn format_jsonc_editor(
    editor: &Entity<CodeEditorState>,
    session_state: &Entity<RequestSessionState>,
    window: &mut Window,
    cx: &mut App,
) {
    format_editor(editor, session_state, window, cx, zest_core::jsonc::format);
}

fn format_editor(
    editor: &Entity<CodeEditorState>,
    session_state: &Entity<RequestSessionState>,
    window: &mut Window,
    cx: &mut App,
    format: impl FnOnce(&str) -> serde_json::Result<String>,
) {
    let raw = editor.read(cx).value().to_string();
    match format(&raw) {
        Ok(pretty) if pretty != raw => {
            // replace_all emits Change for persistence and preserves undo history.
            editor.update(cx, |txt, cx| txt.replace_all(pretty, window, cx));
            session_state.update(cx, |ed, cx| ed.mark_dirty(cx));
        }
        Ok(_) => {}
        Err(e) => crate::notify::error_in("Invalid JSON", e.to_string(), window, cx),
    }
}
