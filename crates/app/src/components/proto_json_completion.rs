//! JSON field-name completion for the gRPC message editor, driven by the
//! request's resolved protobuf schema. `{{env}}` completions keep working —
//! they are delegated to [`EnvCompletionProvider`] when the cursor sits
//! inside a `{{ … ` span.

use std::rc::Rc;

use gpui_kit::base::input::{Diagnostic, DiagnosticSeverity};
use gpui_kit::component::input::EditorState as CodeEditorState;
use gpui_kit::component::input::{
    CompletionProvider, GutterDecoration, GutterDecorationCollection, Position, Rope,
};
use gpui_kit::component::RopeExt as _;
use gpui_kit::{App, Entity, Hsla, Task, Window};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    Documentation, Range as LspRange, TextEdit,
};
use zest_core::grpc::{
    method_descriptors as core_method_descriptors, DescriptorPool, DynamicMessage, Kind,
    MessageDescriptor,
};

use crate::components::env_autocomplete::EnvCompletionProvider;
use crate::state::{RequestSessionState, WorkspaceDataState};

pub struct ProtoJsonCompletionProvider {
    env: EnvCompletionProvider,
    editor_state: Rc<Entity<RequestSessionState>>,
    workspace_state: Rc<Entity<WorkspaceDataState>>,
}

impl ProtoJsonCompletionProvider {
    pub fn new(
        env_state: Entity<crate::state::EnvironmentState>,
        editor_state: Entity<RequestSessionState>,
        workspace_state: Entity<WorkspaceDataState>,
    ) -> Self {
        Self {
            env: EnvCompletionProvider::new(env_state),
            editor_state: Rc::new(editor_state),
            workspace_state: Rc::new(workspace_state),
        }
    }
}

impl CompletionProvider for ProtoJsonCompletionProvider {
    fn completions(
        &self,
        rope: &Rope,
        offset: usize,
        trigger: CompletionContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<CompletionResponse>> {
        let full_text = rope.to_string();
        let safe_offset = offset.min(full_text.len());
        let prefix = &full_text[..safe_offset];

        // Inside a `{{ … ` span the env provider stays in charge.
        if env_trigger_context(prefix).is_some() {
            return self.env.completions(rope, offset, trigger, window, cx);
        }

        let Some(message) = self.current_message(cx) else {
            return Task::ready(Ok(CompletionResponse::Array(vec![])));
        };
        let Some((span, candidates)) = json_field_candidates(&message, prefix) else {
            return Task::ready(Ok(CompletionResponse::Array(vec![])));
        };

        let start_pos = rope.offset_to_position(span.replace_start);
        let end_pos = rope.offset_to_position(span.replace_end);

        let items: Vec<CompletionItem> = candidates
            .into_iter()
            .map(|suggestion| {
                // Without a typed opening quote the inserted name must carry
                // its own quotes to keep the JSON valid.
                let new_text = if span.needs_quote {
                    format!("\"{}\"", suggestion.name)
                } else {
                    suggestion.name.clone()
                };
                CompletionItem {
                    label: suggestion.name.clone(),
                    filter_text: Some(suggestion.name.clone()),
                    kind: Some(CompletionItemKind::FIELD),
                    detail: Some(suggestion.type_label.clone()),
                    documentation: Some(Documentation::String(format!(
                        "{} — {}",
                        suggestion.name, suggestion.type_label
                    ))),
                    text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                        range: LspRange {
                            start: start_pos,
                            end: end_pos,
                        },
                        new_text,
                    })),
                    ..Default::default()
                }
            })
            .collect();

        Task::ready(Ok(CompletionResponse::Array(items)))
    }

    fn is_completion_trigger(&self, _offset: usize, new_text: &str, _cx: &mut App) -> bool {
        new_text.contains('{')
            || new_text.contains('"')
            || new_text.chars().any(|c| c.is_alphanumeric() || c == '_')
    }
}

impl ProtoJsonCompletionProvider {
    /// Input message descriptor of the active gRPC request's selected
    /// service/method, from the session's resolved schema pool.
    fn current_message(&self, cx: &mut App) -> Option<MessageDescriptor> {
        let schema = self.editor_state.read(cx).grpc_schema.clone()?;
        let req_id = self.editor_state.read(cx).active_request_id().to_string();
        let req = self.workspace_state.read(cx).find_request(&req_id)?;
        let payload = req.grpc.as_ref()?;
        let (input, _) = core_method_descriptors(&schema.pool, &payload.service, &payload.method)?;
        Some(input)
    }
}

/// Start of an unterminated `{{ … ` span, if the cursor is inside one.
fn env_trigger_context(prefix: &str) -> Option<()> {
    let brace_pos = prefix.rfind("{{")?;
    let after = &prefix[brace_pos + 2..];
    if after.contains("}}") || after.contains('\n') || after.contains('"') {
        None
    } else {
        Some(())
    }
}

/// One suggested JSON key (owned so suggestions survive the context walk).
pub struct KeySuggestion {
    pub name: String,
    pub type_label: String,
}

/// Where a key completion applies and how the inserted text must look.
pub struct KeyCompletionSpan {
    pub replace_start: usize,
    pub replace_end: usize,
    /// True when no opening quote was typed: accepting a suggestion inserts
    /// the name wrapped in quotes.
    pub needs_quote: bool,
}

/// Suggests the JSON keys available at the cursor position of a partially
/// typed message, tracking nested objects through the typed JSON. Returns
/// the span the completion replaces (the partially typed key — with or
/// without its opening quote) plus the matching fields.
fn json_field_candidates(
    root: &MessageDescriptor,
    prefix: &str,
) -> Option<(KeyCompletionSpan, Vec<KeySuggestion>)> {
    let (current, span) = locate_message_context(root, prefix)?;
    let query: String = prefix[span.replace_start..span.replace_end]
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    let mut candidates: Vec<KeySuggestion> = current
        .fields()
        .map(|field| KeySuggestion {
            name: field.name().to_string(),
            type_label: type_label(&field),
        })
        .collect();
    if !query.is_empty() {
        let lower = query.to_lowercase();
        candidates.retain(|s| s.name.to_lowercase().starts_with(&lower));
    }
    if candidates.is_empty() {
        None
    } else {
        Some((span, candidates))
    }
}

/// Walks the typed JSON prefix to find which message the cursor is inside
/// and where the current (partial) key begins.
fn locate_message_context(
    root: &MessageDescriptor,
    prefix: &str,
) -> Option<(MessageDescriptor, KeyCompletionSpan)> {
    let bytes = prefix.as_bytes();
    let mut stack: Vec<Option<MessageDescriptor>> = Vec::new();
    // `None` = inside an object whose schema is unknown (e.g. a map literal).
    let mut current: Option<MessageDescriptor> = Some(root.clone());
    let mut last_key: Option<String> = None;
    // True right after a key's `:` — the next string is a value, not a key.
    let mut expecting_value = false;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                // Capture the string span, then decide if it is a key.
                let start = i;
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\\' {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == b'"' {
                        break;
                    }
                    i += 1;
                }
                if i >= bytes.len() {
                    // Unterminated string at the cursor. Right after a key's
                    // `:` it is a value being typed — no key suggestions.
                    if expecting_value {
                        return None;
                    }
                    let current = current?;
                    return Some((
                        current,
                        KeyCompletionSpan {
                            replace_start: start + 1,
                            replace_end: prefix.len(),
                            needs_quote: false,
                        },
                    ));
                }
                let content = &prefix[start + 1..i];
                i += 1;
                let mut j = i;
                while j < bytes.len() && (bytes[j] as char).is_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] == b':' {
                    last_key = Some(content.to_string());
                    expecting_value = true;
                } else if i == bytes.len() && !expecting_value {
                    // String closed exactly at the cursor (e.g. auto-paired
                    // quotes) while a key is expected here.
                    let current = current?;
                    return Some((
                        current,
                        KeyCompletionSpan {
                            replace_start: start + 1,
                            replace_end: i,
                            needs_quote: false,
                        },
                    ));
                } else {
                    expecting_value = false;
                }
                // The capture loop already positioned `i` past the closing
                // quote; skip the loop's trailing increment.
                continue;
            }
            b'{' => {
                // A keyed `{` descends into that field's message (None for
                // maps/scalars = unknown scope); an unkeyed `{` (the root
                // object, or a bare object in an array) keeps the scope.
                let new_current = match &last_key {
                    Some(key) => current
                        .as_ref()
                        .and_then(|c| find_field(c, key))
                        .and_then(|f| field_as_message(&f)),
                    None => current.clone(),
                };
                stack.push(current.take());
                current = new_current;
                last_key = None;
                // Whatever precedes the brace, the next token is a key.
                expecting_value = false;
            }
            b'}' => {
                current = stack.pop().flatten();
                last_key = None;
            }
            _ => {}
        }
        i += 1;
    }
    let current = current?;
    // No string under the cursor: suggestions still apply while an
    // identifier is being typed (or the cursor sits where a key may start)
    // right after `{` or `,` — the inserted name is then wrapped in quotes.
    // Value positions (right after `":` or a completed value) yield nothing.
    let ident_len: usize = prefix
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .map(char::len_utf8)
        .sum();
    let before_ident = &prefix[..prefix.len() - ident_len];
    let opener = before_ident
        .bytes()
        .rev()
        .find(|b| !b.is_ascii_whitespace());
    let key_position = matches!(opener, Some(b'{') | Some(b','));
    if !key_position {
        return None;
    }
    Some((
        current,
        KeyCompletionSpan {
            replace_start: prefix.len() - ident_len,
            replace_end: prefix.len(),
            needs_quote: true,
        },
    ))
}

fn find_field(desc: &MessageDescriptor, name: &str) -> Option<FieldRef> {
    desc.fields().find(|f| f.name() == name)
}

type FieldRef = zest_core::grpc::FieldDescriptor;

/// The message type a field points at, if any (map entries excluded —
/// key/value suggestions inside a map literal are noise).
fn field_as_message(field: &FieldRef) -> Option<MessageDescriptor> {
    if field.is_map() {
        return None;
    }
    match field.kind() {
        Kind::Message(m) => Some(m),
        _ => None,
    }
}

/// Human-readable type of a field for the completion detail line.
fn type_label(field: &FieldRef) -> String {
    let cardinality = if field.is_list() {
        "repeated "
    } else if field.is_map() {
        "map "
    } else {
        ""
    };
    let kind = match field.kind() {
        Kind::Message(m) => m.full_name().to_string(),
        Kind::Enum(e) => e.full_name().to_string(),
        other => scalar_label(&other).to_string(),
    };
    format!("{cardinality}{kind}")
}

fn scalar_label(kind: &Kind) -> &'static str {
    match kind {
        Kind::Double => "double",
        Kind::Float => "float",
        Kind::Int32 => "int32",
        Kind::Int64 => "int64",
        Kind::Uint32 => "uint32",
        Kind::Uint64 => "uint64",
        Kind::Sint32 => "sint32",
        Kind::Sint64 => "sint64",
        Kind::Fixed32 => "fixed32",
        Kind::Fixed64 => "fixed64",
        Kind::Sfixed32 => "sfixed32",
        Kind::Sfixed64 => "sfixed64",
        Kind::Bool => "bool",
        Kind::String => "string",
        Kind::Bytes => "bytes",
        Kind::Message(_) | Kind::Enum(_) => "",
    }
}

#[cfg(test)]
#[path = "proto_json_completion_tests.rs"]
mod tests;

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

/// Pushes a validation result into the editor as editor diagnostics (hover
/// tooltip on the offending line) and a red gutter mark on that line, like
/// a breakpoint. Clears both when the message is valid.
pub fn update_grpc_diagnostics(
    editor: &Entity<CodeEditorState>,
    gutter_marks: &GutterDecorationCollection,
    result: &Result<(), MessageValidationError>,
    text: &str,
    error_color: Hsla,
    cx: &mut App,
) {
    let gutter = match result {
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
            if let Err(e) = result {
                // Span the diagnostic from the error column to the end of
                // the line: a zero-width range paints no decoration and its
                // hover hit-test would only match one pixel column.
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
