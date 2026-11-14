use crate::state::{RequestSessionState, ToastState};
use gpui_kit::component::input::EditorState as CodeEditorState;
use gpui_kit::{App, Entity, Window};

/// Pretty-prints the JSON currently in `editor`, saves it back into the
/// editor and flags the active request dirty; toasts when the content is
/// not valid JSON. Shared by the HTTP Body (JSON) editor and the gRPC
/// message editor.
pub fn format_json_editor(
    editor: &Entity<CodeEditorState>,
    session_state: &Entity<RequestSessionState>,
    toast_state: Option<&Entity<ToastState>>,
    window: &mut Window,
    cx: &mut App,
) {
    let raw = editor.read(cx).value().to_string();
    match serde_json::from_str::<serde_json::Value>(&raw) {
        Ok(val) => {
            if let Ok(pretty) = serde_json::to_string_pretty(&val) {
                editor.update(cx, |txt, cx| {
                    txt.set_value(pretty, window, cx);
                });
                session_state.update(cx, |ed, cx| ed.mark_dirty(cx));
            }
        }
        Err(e) => {
            if let Some(ts) = toast_state {
                ts.update(cx, |toast, cx| {
                    toast.push_error("Invalid JSON", e.to_string(), cx);
                });
            }
        }
    }
}
