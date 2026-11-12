use super::ScriptEditorKind;
use crate::components::env_autocomplete::{detect_unclosed_env_var, get_env_completions};
use crate::state::EnvironmentState;
use anyhow::Result;
use gpui_kit::component::input::{CompletionProvider, Rope};
use gpui_kit::component::RopeExt;
use gpui_kit::{App, Entity, Task, Window};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    Documentation, Range as LspRange, TextEdit,
};

pub(crate) struct CandidateDef {
    pub(crate) name: &'static str,
    pub(crate) insert_text: &'static str,
    pub(crate) kind: CompletionItemKind,
    pub(crate) detail: &'static str,
    pub(crate) doc: &'static str,
}

pub struct ScriptCompletionProvider {
    pub(crate) kind: ScriptEditorKind,
    pub(crate) env_state: Option<Entity<EnvironmentState>>,
}

impl ScriptCompletionProvider {
    #[allow(dead_code)]
    pub fn new(kind: ScriptEditorKind) -> Self {
        Self {
            kind,
            env_state: None,
        }
    }

    pub fn with_environment(kind: ScriptEditorKind, env_state: Entity<EnvironmentState>) -> Self {
        Self {
            kind,
            env_state: Some(env_state),
        }
    }

    pub fn should_trigger(new_text: &str) -> bool {
        new_text.contains('{')
            || new_text.contains('"')
            || new_text.contains('\'')
            || new_text
                .chars()
                .any(|c| c.is_alphanumeric() || c == '.' || c == '_' || c == '$')
    }
}

impl CompletionProvider for ScriptCompletionProvider {
    fn completions(
        &self,
        rope: &Rope,
        offset: usize,
        _trigger: CompletionContext,
        _window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<CompletionResponse>> {
        let full_text = rope.to_string();
        let safe_offset = offset.min(full_text.len());
        let slice = &full_text[..safe_offset];

        // 1. Check for unclosed {{ or {{prefix in script
        if let Some((start_pos_byte, _end_pos_byte, q)) = detect_unclosed_env_var(slice) {
            if let Some(env_entity) = &self.env_state {
                let env = env_entity.read(cx);
                let candidates = get_env_completions(env, &q);
                let start_pos = rope.offset_to_position(start_pos_byte);
                let end_pos = rope.offset_to_position(safe_offset);
                let items: Vec<CompletionItem> = candidates
                    .into_iter()
                    .map(|c| CompletionItem {
                        label: format!("{{{{{}}}}}", c.name),
                        filter_text: Some(c.name.clone()),
                        kind: Some(CompletionItemKind::VARIABLE),
                        detail: Some(format!("{} • {}", c.detail_text(), c.scope)),
                        documentation: Some(Documentation::String(format!(
                            "Environment Variable: {}\nValue: {}\nScope: {}",
                            c.name,
                            c.detail_text(),
                            c.scope
                        ))),
                        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                            range: LspRange {
                                start: start_pos,
                                end: end_pos,
                            },
                            new_text: format!("{{{{{}}}}}", c.name),
                        })),
                        ..Default::default()
                    })
                    .collect();
                return Task::ready(Ok(CompletionResponse::Array(items)));
            }
        }

        // 2. Check for variable key inside z.environment.get("...", z.variables.get("...", etc.
        if let Some(quote_idx) = slice.rfind(['"', '\'']) {
            let before_quote = slice[..quote_idx].trim_end();
            if before_quote.ends_with(".get(")
                || before_quote.ends_with(".has(")
                || before_quote.ends_with(".unset(")
                || before_quote.ends_with(".set(")
            {
                let var_query = &slice[quote_idx + 1..];
                if !var_query.contains(['"', '\'', '\n']) {
                    if let Some(env_entity) = &self.env_state {
                        let env = env_entity.read(cx);
                        let candidates = get_env_completions(env, var_query);
                        let start_lsp = rope.offset_to_position(quote_idx + 1);
                        let end_lsp = rope.offset_to_position(safe_offset);
                        let items: Vec<CompletionItem> = candidates
                            .into_iter()
                            .map(|c| CompletionItem {
                                label: c.name.clone(),
                                filter_text: Some(c.name.clone()),
                                kind: Some(CompletionItemKind::VARIABLE),
                                detail: Some(format!("{} • {}", c.detail_text(), c.scope)),
                                documentation: Some(Documentation::String(format!(
                                    "Environment Variable: {}\nValue: {}\nScope: {}",
                                    c.name,
                                    c.detail_text(),
                                    c.scope
                                ))),
                                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                                    range: LspRange {
                                        start: start_lsp,
                                        end: end_lsp,
                                    },
                                    new_text: c.name,
                                })),
                                ..Default::default()
                            })
                            .collect();
                        return Task::ready(Ok(CompletionResponse::Array(items)));
                    }
                }
            }
        }

        let query_start = slice
            .rfind(|c: char| !c.is_alphanumeric() && c != '_' && c != '$' && c != '.')
            .map(|idx| idx + 1)
            .unwrap_or(0);
        let expr = &slice[query_start..];

        let (base_path, query, replace_start) = if let Some(dot_idx) = expr.rfind('.') {
            let base = &expr[..dot_idx];
            let q = &expr[dot_idx + 1..];
            (base.to_string(), q.to_string(), query_start + dot_idx + 1)
        } else {
            ("".to_string(), expr.to_string(), query_start)
        };

        let start_pos = rope.offset_to_position(replace_start);
        let end_pos = rope.offset_to_position(safe_offset);

        let candidates = self.get_candidates(&base_path);
        let query_lower = query.to_lowercase();

        let items: Vec<CompletionItem> = candidates
            .into_iter()
            .filter(|c| c.name.to_lowercase().starts_with(&query_lower))
            .map(|c| CompletionItem {
                label: c.name.to_string(),
                filter_text: Some(c.name.to_string()),
                kind: Some(c.kind),
                detail: Some(c.detail.to_string()),
                documentation: Some(Documentation::String(c.doc.to_string())),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range: LspRange {
                        start: start_pos,
                        end: end_pos,
                    },
                    new_text: c.insert_text.to_string(),
                })),
                ..Default::default()
            })
            .collect();

        Task::ready(Ok(CompletionResponse::Array(items)))
    }

    fn is_completion_trigger(&self, _offset: usize, new_text: &str, _cx: &mut App) -> bool {
        Self::should_trigger(new_text)
    }
}
