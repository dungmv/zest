//! Environment variable completion detection and LSP `CompletionProvider` binding.

use super::style::list_candidates_in_state;
use crate::state::EnvironmentState;
use anyhow::Result;
use gpui_kit::component::input::{CompletionProvider, Rope};
use gpui_kit::component::RopeExt;
use gpui_kit::{App, Entity, Task, Window};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    Documentation, Range as LspRange, TextEdit,
};

/// Detects if `text` contains an unclosed `{{` or `{{prefix` (at the end, middle, or cursor).
/// Returns `Some((start_pos, end_pos_to_replace, query_after_braces))`.
pub fn detect_unclosed_env_var(text: &str) -> Option<(usize, usize, String)> {
    detect_unclosed_env_var_at(text, None)
}

/// Detects an unclosed `{{` in `text`, optionally taking `cursor_pos` into account.
pub fn detect_unclosed_env_var_at(
    text: &str,
    cursor_pos: Option<usize>,
) -> Option<(usize, usize, String)> {
    let mut matches = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;

    while i + 1 < text.len() {
        if bytes[i] == b'{' && bytes[i + 1] == b'{' {
            let start = i;
            let after_start = i + 2;
            let mut query_end = after_start;
            let mut is_closed = false;

            while query_end < text.len() {
                if query_end + 1 < text.len()
                    && bytes[query_end] == b'}'
                    && bytes[query_end + 1] == b'}'
                {
                    is_closed = true;
                    break;
                }
                let c = text[query_end..].chars().next().unwrap();
                if c == '/'
                    || c == '?'
                    || c == '&'
                    || c == '='
                    || c == ' '
                    || c == '\n'
                    || c == '\t'
                    || c == '"'
                    || c == '\''
                    || c == '{'
                {
                    break;
                }
                query_end += c.len_utf8();
            }

            if !is_closed {
                let query = text[after_start..query_end].to_string();
                matches.push((start, query_end, query));
            }

            i = query_end;
        } else {
            i += 1;
        }
    }

    if matches.is_empty() {
        return None;
    }

    if let Some(cursor) = cursor_pos {
        for (start, end, query) in matches.iter().rev() {
            if cursor >= *start && cursor <= *end + 1 {
                return Some((*start, *end, query.clone()));
            }
        }
    }

    matches.pop()
}

/// Byte offset of an unclosed `{{` span (start of the braces plus the query
/// typed so far) under JSON-editor semantics: the span terminates only at
/// `}}`, a newline, or a quote.
pub fn unclosed_env_span(prefix: &str) -> Option<(usize, &str)> {
    let brace_pos = prefix.rfind("{{")?;
    let after = &prefix[brace_pos + 2..];
    if after.contains("}}") || after.contains('\n') || after.contains('"') {
        None
    } else {
        Some((brace_pos, after))
    }
}

pub struct EnvCompletionProvider {
    env_state: Entity<EnvironmentState>,
}

impl EnvCompletionProvider {
    pub fn new(env_state: Entity<EnvironmentState>) -> Self {
        Self { env_state }
    }
}

impl CompletionProvider for EnvCompletionProvider {
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

        if let Some((brace_pos, query)) = unclosed_env_span(slice) {
            let env = self.env_state.read(cx);
            let candidates = list_candidates_in_state(env, query);

            let start_pos = rope.offset_to_position(brace_pos);
            let end_pos = rope.offset_to_position(safe_offset);

            let items: Vec<CompletionItem> = candidates
                .into_iter()
                .map(|c| {
                    let label = format!("{{{{{}}}}}", c.name);
                    let insert_text = format!("{{{{{}}}}}", c.name);
                    let scope_label = c.scope_label();
                    CompletionItem {
                        label,
                        filter_text: Some(c.name.clone()),
                        kind: Some(CompletionItemKind::VARIABLE),
                        detail: Some(format!("{} • {}", c.detail_text(), scope_label)),
                        documentation: Some(Documentation::String(format!(
                            "Environment Variable: {}\nValue: {}\nScope: {}",
                            c.name,
                            c.detail_text(),
                            scope_label
                        ))),
                        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                            range: LspRange {
                                start: start_pos,
                                end: end_pos,
                            },
                            new_text: insert_text,
                        })),
                        ..Default::default()
                    }
                })
                .collect();

            return Task::ready(Ok(CompletionResponse::Array(items)));
        }

        Task::ready(Ok(CompletionResponse::Array(vec![])))
    }

    fn is_completion_trigger(&self, _offset: usize, new_text: &str, _cx: &mut App) -> bool {
        new_text.contains('{')
            || new_text
                .chars()
                .any(|c| c.is_alphanumeric() || c == '_' || c == '$')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_unclosed_env_var() {
        assert_eq!(
            detect_unclosed_env_var("https://api.com/{{"),
            Some((16, 18, "".to_string()))
        );
        assert_eq!(
            detect_unclosed_env_var("https://api.com/{{host"),
            Some((16, 22, "host".to_string()))
        );
        assert_eq!(detect_unclosed_env_var("https://api.com/{{host}}"), None);
        assert_eq!(
            detect_unclosed_env_var("https://api.com/{{host}}/{{user"),
            Some((25, 31, "user".to_string()))
        );
        assert_eq!(detect_unclosed_env_var("no braces"), None);
        assert_eq!(
            detect_unclosed_env_var("https://api.com/{{host/users"),
            Some((16, 22, "host".to_string()))
        );
        assert_eq!(
            detect_unclosed_env_var("https://api.com/{{host/users/{{id}}"),
            Some((16, 22, "host".to_string()))
        );
        assert_eq!(
            detect_unclosed_env_var("{{base/api"),
            Some((0, 6, "base".to_string()))
        );
        assert_eq!(detect_unclosed_env_var("{{base_url}}/api"), None);
        assert_eq!(
            detect_unclosed_env_var("{{base_url"),
            Some((0, 10, "base_url".to_string()))
        );
        assert_eq!(
            detect_unclosed_env_var("https://api.com/{{$guid"),
            Some((16, 23, "$guid".to_string()))
        );

        let text = "https://api.com/{{host/users";
        let (start, end, _) = detect_unclosed_env_var(text).unwrap();
        let mut replaced = text.to_string();
        replaced.replace_range(start..end, "{{base_url}}");
        assert_eq!(replaced, "https://api.com/{{base_url}}/users");
    }

    #[test]
    fn test_unclosed_env_span() {
        assert_eq!(unclosed_env_span("{"), None);
        assert_eq!(unclosed_env_span("{{a"), Some((0, "a")));
        assert_eq!(unclosed_env_span("x{{b"), Some((1, "b")));
        assert_eq!(unclosed_env_span("{{a}}"), None);
        assert_eq!(unclosed_env_span("{{a}}b"), None);
        assert_eq!(unclosed_env_span("{{a\""), None);
        assert_eq!(unclosed_env_span("{{a\nb"), None);
        assert_eq!(unclosed_env_span("no braces"), None);
    }
}
