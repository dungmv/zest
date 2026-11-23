//! Deep GraphQL query completion driven by the session's introspected
//! schema: field/argument/enum-value/type/fragment/directive suggestions at
//! the caret, resolved by [`zest_core::graphql::completions_at`].
//! `{{env}}` spans are delegated to [`EnvCompletionProvider`]. Also carries
//! the variables editor's JSON validation (hover-gated diagnostics + gutter
//! marks, mirroring the gRPC validation plumbing).

use gpui_kit::base::input::{Diagnostic, DiagnosticSeverity, Position};
use gpui_kit::component::input::{
    CompletionProvider, EditorState as CodeEditorState, GutterDecoration,
    GutterDecorationCollection, Rope,
};
use gpui_kit::component::RopeExt as _;
use gpui_kit::{App, Entity, Hsla, Task, Window};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    Documentation, Range as LspRange, TextEdit,
};
use zest_core::graphql::{completions_at, CompletionKind};

use crate::env::{unclosed_env_span, EnvCompletionProvider};
use crate::state::RequestSessionState;

pub struct GraphQLQueryCompletionProvider {
    env: EnvCompletionProvider,
    editor_state: Entity<RequestSessionState>,
}

impl GraphQLQueryCompletionProvider {
    pub fn new(
        env_state: Entity<crate::state::EnvironmentState>,
        editor_state: Entity<RequestSessionState>,
    ) -> Self {
        Self {
            env: EnvCompletionProvider::new(env_state),
            editor_state,
        }
    }
}

fn is_graphql_name_byte(b: u8) -> bool {
    b == b'_' || b.is_ascii_alphanumeric()
}

impl CompletionProvider for GraphQLQueryCompletionProvider {
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
        let prefix_text = &full_text[..safe_offset];

        // Inside a `{{ … ` span the env provider stays in charge.
        if unclosed_env_span(prefix_text).is_some() {
            return self.env.completions(rope, offset, trigger, window, cx);
        }

        let Some(schema) = self.editor_state.read(cx).graphql_schema.clone() else {
            return Task::ready(Ok(CompletionResponse::Array(vec![])));
        };
        let suggestions = completions_at(&full_text, safe_offset, &schema);
        if suggestions.is_empty() {
            return Task::ready(Ok(CompletionResponse::Array(vec![])));
        }

        // Replace the typed name run directly before the caret (GraphQL
        // names are ASCII; the resolver filters by the same prefix).
        let typed_bytes = prefix_text
            .bytes()
            .rev()
            .take_while(|b| is_graphql_name_byte(*b))
            .count();
        let replace_start = safe_offset - typed_bytes;
        let start_pos = rope.offset_to_position(replace_start);
        let end_pos = rope.offset_to_position(safe_offset);

        let items: Vec<CompletionItem> = suggestions
            .into_iter()
            .map(|suggestion| CompletionItem {
                label: suggestion.label.clone(),
                filter_text: Some(suggestion.label.clone()),
                kind: Some(map_kind(suggestion.kind)),
                detail: Some(suggestion.detail.clone()),
                documentation: Some(Documentation::String(format!(
                    "{} — {}",
                    suggestion.label, suggestion.detail
                ))),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range: LspRange {
                        start: start_pos,
                        end: end_pos,
                    },
                    new_text: suggestion.label,
                })),
                ..Default::default()
            })
            .collect();

        Task::ready(Ok(CompletionResponse::Array(items)))
    }

    fn is_completion_trigger(&self, _offset: usize, new_text: &str, _cx: &mut App) -> bool {
        // Sibling-field completion also fires on whitespace: positions that
        // admit nothing (strings, top level) resolve to an empty list and
        // show no popup.
        new_text
            .chars()
            .any(|c| c.is_alphanumeric() || matches!(c, '_' | '@' | ':' | '(' | '{' | ' ' | '\n'))
    }
}

fn map_kind(kind: CompletionKind) -> CompletionItemKind {
    match kind {
        CompletionKind::Field | CompletionKind::InputField => CompletionItemKind::FIELD,
        CompletionKind::Argument => CompletionItemKind::VARIABLE,
        CompletionKind::EnumValue => CompletionItemKind::ENUM_MEMBER,
        CompletionKind::TypeName => CompletionItemKind::CLASS,
        CompletionKind::Fragment => CompletionItemKind::MODULE,
        CompletionKind::Directive => CompletionItemKind::FUNCTION,
    }
}

/// Variables validation: syntax plus "must be an object". Errors carry
/// zero-based line/column for the editor plumbing.
fn validate_variables(text: &str) -> Result<(), (usize, usize, String)> {
    if text.trim().is_empty() {
        return Ok(());
    }
    match serde_json::from_str::<serde_json::Value>(text) {
        Err(e) => Err((
            e.line().saturating_sub(1),
            e.column().saturating_sub(1),
            format!("Invalid JSON: {e}"),
        )),
        Ok(value) if !value.is_object() => {
            Err((0, 0, "Variables must be a JSON object.".to_string()))
        }
        Ok(_) => Ok(()),
    }
}

/// Revalidates the GraphQL variables editor: a red gutter mark always
/// reflects the state; the hover tooltip diagnostic is only pushed while
/// the pointer is over the editor (the diagnostic popover latches on
/// mouse-move and cannot be dismissed programmatically).
pub fn refresh_variables_validation(
    editor: &Entity<CodeEditorState>,
    gutter_marks: &GutterDecorationCollection,
    hovered: bool,
    text: &str,
    error_color: Hsla,
    cx: &mut App,
) {
    let result = validate_variables(text);
    let gutter = match &result {
        Ok(()) => Vec::new(),
        Err((line, _column, _message)) => {
            let (start, end) = line_byte_range(text, *line);
            vec![GutterDecoration::new(start..end).with_color(error_color)]
        }
    };
    gutter_marks.set(gutter, cx);

    editor.update(cx, |state, cx| {
        if let Some(set) = state.diagnostics_mut() {
            set.clear();
            if hovered {
                if let Err((line, column, message)) = &result {
                    let line_chars = text
                        .split('\n')
                        .nth(*line)
                        .map(|l| l.chars().count() as u32)
                        .unwrap_or(*column as u32);
                    let start = Position {
                        line: *line as u32,
                        character: (*column as u32).min(line_chars),
                    };
                    let end = Position {
                        line: *line as u32,
                        character: line_chars,
                    };
                    set.push(Diagnostic {
                        range: start..end,
                        severity: DiagnosticSeverity::Error,
                        message: message.clone().into(),
                        source: Some("zest-graphql".into()),
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
#[path = "graphql_query_tests.rs"]
mod tests;
