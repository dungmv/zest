//! GPUI LSP `CompletionProvider` adapter for the gRPC message JSON editor.
//! Pure JSON-prefix walking and protobuf field resolution live in
//! `zest_core::grpc::json_field_candidates`; `{{env}}` spans are delegated
//! to [`EnvCompletionProvider`].

use gpui_kit::component::input::{CompletionProvider, Rope};
use gpui_kit::component::RopeExt as _;
use gpui_kit::{App, Entity, Task, Window};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    Documentation, Range as LspRange, TextEdit,
};
use zest_core::grpc::{
    json_field_candidates, method_descriptors as core_method_descriptors, MessageDescriptor,
};

use crate::env::{unclosed_env_span, EnvCompletionProvider};
use crate::state::{EnvironmentState, RequestSessionState, WorkspaceDataState};

pub(super) struct ProtoJsonCompletionProvider {
    env: EnvCompletionProvider,
    session_state: Entity<RequestSessionState>,
    workspace_state: Entity<WorkspaceDataState>,
}

impl ProtoJsonCompletionProvider {
    pub(super) fn new(
        env_state: Entity<EnvironmentState>,
        session_state: Entity<RequestSessionState>,
        workspace_state: Entity<WorkspaceDataState>,
    ) -> Self {
        Self {
            env: EnvCompletionProvider::new(env_state),
            session_state,
            workspace_state,
        }
    }

    /// Input message descriptor of the active gRPC request's selected
    /// service/method, from the session's resolved schema pool.
    fn current_message(&self, cx: &mut App) -> Option<MessageDescriptor> {
        let session = self.session_state.read(cx);
        let schema = session.grpc_schema.clone()?;
        let req_id = session.active_request_id().to_string();
        let req = self.workspace_state.read(cx).find_request(&req_id)?;
        let payload = req.grpc.as_ref()?;
        let (input, _) = core_method_descriptors(&schema.pool, &payload.service, &payload.method)?;
        Some(input)
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
        let safe_offset = clamp_char_boundary(&full_text, offset);
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

    fn is_completion_trigger(&self, offset: usize, new_text: &str, cx: &mut App) -> bool {
        self.env.is_completion_trigger(offset, new_text, cx)
            || new_text.contains('{')
            || new_text.contains('"')
            || new_text.chars().any(|c| c.is_alphanumeric() || c == '_')
    }
}

fn clamp_char_boundary(text: &str, offset: usize) -> usize {
    let mut end = offset.min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    end
}
