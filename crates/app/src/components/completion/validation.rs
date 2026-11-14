//! gRPC message JSON validation, mirrored into the message editor: gutter
//! marks always reflect the current state, hover diagnostics gate on the
//! pointer. Purely editor plumbing — completions live in [`super::proto_json`].

use gpui_kit::base::input::{Diagnostic, DiagnosticSeverity};
use gpui_kit::component::input::EditorState as CodeEditorState;
use gpui_kit::component::input::{GutterDecoration, GutterDecorationCollection, Position};
use gpui_kit::{App, Entity, Hsla};
use zest_core::grpc::{
    method_descriptors as core_method_descriptors, DescriptorPool, DynamicMessage,
};

/// A failed JSON message validation, positioned on the source text.
/// `line`/`column` are zero-based character coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageValidationError {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

/// Validates the gRPC message JSON: syntax first, then schema conformance
/// (strict unknown-field checking, matching the send path) when the
/// selection resolves against the fetched pool. An empty editor is the
/// proto3 zero-value message.
pub fn validate_message(
    pool: &DescriptorPool,
    service: &str,
    method: &str,
    text: &str,
) -> Result<(), MessageValidationError> {
    if text.trim().is_empty() {
        return Ok(());
    }
    if let Err(e) = serde_json::from_str::<serde_json::Value>(text) {
        return Err(MessageValidationError {
            line: e.line().saturating_sub(1),
            column: e.column().saturating_sub(1),
            message: format!("Invalid JSON: {e}"),
        });
    }
    let Some((input, _)) = core_method_descriptors(pool, service, method) else {
        return Ok(());
    };
    let mut deserializer = serde_json::Deserializer::from_str(text);
    if let Err(e) = DynamicMessage::deserialize(input, &mut deserializer) {
        return Err(MessageValidationError {
            line: e.line().saturating_sub(1),
            column: e.column().saturating_sub(1),
            message: e.to_string(),
        });
    }
    if let Err(e) = deserializer.end() {
        return Err(MessageValidationError {
            line: e.line().saturating_sub(1),
            column: e.column().saturating_sub(1),
            message: e.to_string(),
        });
    }
    Ok(())
}

/// Revalidates the gRPC message and mirrors the result into the editor:
/// a red gutter mark on the offending line always reflects the state,
/// while the in-editor diagnostic (hover tooltip) is only pushed while
/// the pointer hovers the editor — the underlying popover latches on
/// mouse-move and cannot be dismissed programmatically, so pushing it
/// unconditionally would leave it stuck on screen while typing.
#[allow(clippy::too_many_arguments)]
pub fn refresh_grpc_validation(
    editor: &Entity<CodeEditorState>,
    gutter_marks: &GutterDecorationCollection,
    hovered: bool,
    pool: &DescriptorPool,
    service: &str,
    method: &str,
    text: &str,
    error_color: Hsla,
    cx: &mut App,
) {
    let result = validate_message(pool, service, method, text);
    let gutter = match &result {
        Ok(()) => Vec::new(),
        Err(e) => {
            let (start, end) = line_byte_range(text, e.line);
            vec![GutterDecoration::new(start..end).with_color(error_color)]
        }
    };
    gutter_marks.set(gutter, cx);

    editor.update(cx, |state, cx| {
        if let Some(set) = state.diagnostics_mut() {
            set.clear();
            if hovered {
                if let Err(e) = &result {
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
                        source: Some("zest-grpc".into()),
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
#[path = "validation_tests.rs"]
mod tests;
