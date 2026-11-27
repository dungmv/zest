//! Shared GPUI editor diagnostic plumbing for request composer editors
//! (gRPC message JSON and GraphQL variables JSON): red gutter marks always
//! reflect validation state, while hover tooltip diagnostics are gated on
//! pointer hover (the underlying popover latches on mouse-move and cannot
//! be dismissed programmatically).

use gpui_kit::base::input::{Diagnostic, DiagnosticSeverity, Position};
use gpui_kit::component::input::{
    EditorState as CodeEditorState, GutterDecoration, GutterDecorationCollection,
};
use gpui_kit::{App, Entity};
use zest_core::graphql::validate_variables;
use zest_core::grpc::validate_message;

use crate::state::{CollectionState, RequestSessionState};
use crate::theme::theme;

struct EditorDiagnostic {
    line: usize,
    column: usize,
    message: String,
}

/// Revalidates the active gRPC request's message JSON against the session's
/// resolved protobuf schema and mirrors the result into gutter marks and
/// hover-gated editor diagnostics.
pub(crate) fn refresh_grpc_validation(
    editor: &Entity<CodeEditorState>,
    gutter_marks: &GutterDecorationCollection,
    hovered: bool,
    text: &str,
    session_state: &Entity<RequestSessionState>,
    workspace_state: &Entity<CollectionState>,
    cx: &mut App,
) {
    let (req_id, schema) = {
        let session = session_state.read(cx);
        (
            session.active_request_id().to_string(),
            session.grpc_schema.clone().unwrap_or_default(),
        )
    };
    let payload = workspace_state
        .read(cx)
        .find_request(&req_id)
        .and_then(|r| r.grpc.clone())
        .unwrap_or_default();

    let diagnostic = validate_message(&schema.pool, &payload.service, &payload.method, text)
        .err()
        .map(|e| EditorDiagnostic {
            line: e.line,
            column: e.column,
            message: e.message,
        });

    apply_editor_diagnostic(
        editor,
        gutter_marks,
        hovered,
        text,
        diagnostic.as_ref(),
        "zest-grpc",
        cx,
    );
}

/// Revalidates the GraphQL variables JSON editor and mirrors the result
/// into gutter marks and hover-gated editor diagnostics.
pub(crate) fn refresh_graphql_variables_validation(
    editor: &Entity<CodeEditorState>,
    gutter_marks: &GutterDecorationCollection,
    hovered: bool,
    text: &str,
    cx: &mut App,
) {
    let diagnostic = validate_variables(text).err().map(|e| EditorDiagnostic {
        line: e.line,
        column: e.column,
        message: e.message,
    });

    apply_editor_diagnostic(
        editor,
        gutter_marks,
        hovered,
        text,
        diagnostic.as_ref(),
        "zest-graphql",
        cx,
    );
}

fn apply_editor_diagnostic(
    editor: &Entity<CodeEditorState>,
    gutter_marks: &GutterDecorationCollection,
    hovered: bool,
    text: &str,
    error: Option<&EditorDiagnostic>,
    source: &'static str,
    cx: &mut App,
) {
    let error_color = theme().status_5xx;
    let gutter = match error {
        None => Vec::new(),
        Some(e) => {
            let (start, end) = line_byte_range(text, e.line);
            vec![GutterDecoration::new(start..end).with_color(error_color)]
        }
    };
    gutter_marks.set(gutter, cx);

    editor.update(cx, |state, cx| {
        if let Some(set) = state.diagnostics_mut() {
            set.clear();
            if hovered {
                if let Some(e) = error {
                    // Span the diagnostic from the error column to the end
                    // of the line: a zero-width range paints no decoration
                    // and its hover hit-test would only match one pixel
                    // column.
                    let line_chars = text
                        .split('\n')
                        .nth(e.line)
                        .map(|line| line.chars().count() as u32)
                        .unwrap_or(e.column as u32);
                    let start = Position {
                        line: e.line as u32,
                        character: (e.column as u32).min(line_chars),
                    };
                    let end = Position {
                        line: e.line as u32,
                        character: line_chars,
                    };
                    set.push(Diagnostic {
                        range: start..end,
                        severity: DiagnosticSeverity::Error,
                        message: e.message.clone().into(),
                        source: Some(source.into()),
                        ..Default::default()
                    });
                }
            }
        }
        cx.notify();
    });
}

/// UTF-8 byte range of one line (0-based index) within the source text.
fn line_byte_range(text: &str, line: usize) -> (usize, usize) {
    let mut offset = 0;
    for (index, line_text) in text.split('\n').enumerate() {
        if index == line {
            return (offset, offset + line_text.len());
        }
        offset += line_text.len() + 1;
    }
    (text.len(), text.len())
}

#[cfg(test)]
mod tests {
    use super::line_byte_range;

    #[test]
    fn test_line_byte_range_ascii_and_utf8() {
        let text = "first\nsécond\nthird";
        assert_eq!(line_byte_range(text, 0), (0, 5));
        assert_eq!(line_byte_range(text, 1), (6, 13));
        assert_eq!(line_byte_range(text, 2), (14, 19));
        assert_eq!(line_byte_range(text, 9), (text.len(), text.len()));
    }
}
