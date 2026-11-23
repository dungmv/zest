//! JSON field-name completion for the gRPC message editor, driven by the
//! request's resolved protobuf schema. `{{env}}` completions keep working —
//! they are delegated to [`EnvCompletionProvider`] when the cursor sits
//! inside a `{{ … ` span.

use gpui_kit::component::input::{CompletionProvider, Rope};
use gpui_kit::component::RopeExt as _;
use gpui_kit::{App, Entity, Task, Window};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    Documentation, Range as LspRange, TextEdit,
};
use zest_core::grpc::{method_descriptors as core_method_descriptors, MessageDescriptor};

use crate::env::{unclosed_env_span, EnvCompletionProvider};
use crate::state::{RequestSessionState, WorkspaceDataState};

mod context;
mod proto_fields;

use context::json_field_candidates;

pub struct ProtoJsonCompletionProvider {
    env: EnvCompletionProvider,
    editor_state: Entity<RequestSessionState>,
    workspace_state: Entity<WorkspaceDataState>,
}

impl ProtoJsonCompletionProvider {
    pub fn new(
        env_state: Entity<crate::state::EnvironmentState>,
        editor_state: Entity<RequestSessionState>,
        workspace_state: Entity<WorkspaceDataState>,
    ) -> Self {
        Self {
            env: EnvCompletionProvider::new(env_state),
            editor_state,
            workspace_state,
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
        if unclosed_env_span(prefix).is_some() {
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

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
