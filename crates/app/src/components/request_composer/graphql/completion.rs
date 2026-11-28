//! GPUI LSP `CompletionProvider` adapter for the GraphQL query editor.
//! Pure schema-aware completion lives in `zest_core::graphql::completions_at`;
//! `{{env}}` spans are delegated to [`EnvCompletionProvider`].

use gpui_kit::component::input::{CompletionProvider, Rope};
use gpui_kit::component::RopeExt as _;
use gpui_kit::{App, Entity, Task, Window};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    Documentation, Range as LspRange, TextEdit,
};
use zest_core::graphql::{completions_at, CompletionKind};

use crate::env::{unclosed_env_span, EnvCompletionProvider};
use crate::state::{EnvironmentState, RequestSessionState};

pub(in crate::components::request_composer) struct GraphQLQueryCompletionProvider {
    env: EnvCompletionProvider,
    session_state: Entity<RequestSessionState>,
}

impl GraphQLQueryCompletionProvider {
    pub(in crate::components::request_composer) fn new(
        env_state: Entity<EnvironmentState>,
        session_state: Entity<RequestSessionState>,
    ) -> Self {
        Self {
            env: EnvCompletionProvider::new(env_state),
            session_state,
        }
    }
}

fn is_graphql_name_byte(b: u8) -> bool {
    b == b'_' || b.is_ascii_alphanumeric()
}

fn clamp_char_boundary(text: &str, offset: usize) -> usize {
    let mut end = offset.min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    end
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
        let safe_offset = clamp_char_boundary(&full_text, offset);
        let prefix_text = &full_text[..safe_offset];

        // Inside a `{{ … ` span the env provider stays in charge.
        if unclosed_env_span(prefix_text).is_some() {
            return self.env.completions(rope, offset, trigger, window, cx);
        }

        let Some(schema) = self.session_state.read(cx).graphql_schema.clone() else {
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

    fn is_completion_trigger(&self, offset: usize, new_text: &str, cx: &mut App) -> bool {
        // Sibling-field completion also fires on whitespace: positions that
        // admit nothing (strings, top level) resolve to an empty list and
        // show no popup.
        self.env.is_completion_trigger(offset, new_text, cx)
            || new_text.chars().any(|c| {
                c.is_alphanumeric() || matches!(c, '_' | '@' | ':' | '(' | '{' | ' ' | '\n')
            })
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

#[cfg(test)]
mod tests {
    use super::clamp_char_boundary;

    #[test]
    fn test_clamp_char_boundary_handles_multibyte_utf8() {
        let s = "aé"; // 'a' (0..1), 'é' (1..3)
        assert_eq!(clamp_char_boundary(s, 0), 0);
        assert_eq!(clamp_char_boundary(s, 1), 1);
        assert_eq!(clamp_char_boundary(s, 2), 1);
        assert_eq!(clamp_char_boundary(s, 3), 3);
        assert_eq!(clamp_char_boundary(s, 99), 3);
    }
}
