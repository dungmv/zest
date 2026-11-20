use crate::state::RequestSessionState;
use gpui_kit::component::input::EditorState as CodeEditorState;
use gpui_kit::{App, Entity, Window};

/// Pretty-prints the JSON currently in `editor`, saves it back into the
/// editor and flags the active request dirty; notifies when the content is
/// not valid JSON. Shared by the HTTP Body (JSON) editor and the gRPC
/// message editor.
///
/// Uses `replace_all` rather than `set_value`: `set_value` suppresses
/// `InputEvent::Change`, so the formatted text would only live in the editor
/// buffer and be lost on the next request switch (the subscriptions in
/// `subscriptions.rs` persist the value into the request on Change).
/// `replace_all` emits the event and keeps undo history intact.
pub fn format_json_editor(
    editor: &Entity<CodeEditorState>,
    session_state: &Entity<RequestSessionState>,
    window: &mut Window,
    cx: &mut App,
) {
    let raw = editor.read(cx).value().to_string();
    match serde_json::from_str::<serde_json::Value>(&raw) {
        Ok(val) => {
            if let Ok(pretty) = serde_json::to_string_pretty(&val) {
                // Skip the no-op rewrite (already formatted) to avoid a
                // pointless dirty flag and empty undo entry.
                if pretty != raw {
                    editor.update(cx, |txt, cx| {
                        txt.replace_all(pretty, window, cx);
                    });
                    session_state.update(cx, |ed, cx| ed.mark_dirty(cx));
                }
            }
        }
        Err(e) => {
            crate::notify::error_in("Invalid JSON", e.to_string(), window, cx);
        }
    }
}
