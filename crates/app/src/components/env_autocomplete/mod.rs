mod render;

pub use render::{
    candidate_type_badge, render_candidate_detail_card, render_candidate_row, CANDIDATE_DETAIL_GAP,
    CANDIDATE_DETAIL_WIDTH, CANDIDATE_LIST_WIDTH,
};

use crate::state::EnvironmentState;
use anyhow::Result;
use gpui_kit::{
    div, px, App, Div, Entity, InteractiveElement as _, KeyDownEvent, ParentElement as _,
    ScrollHandle, Styled as _, Task, Window,
};
use gpui_kit::component::input::{CompletionProvider, InlineToken, InputContent};
use gpui_kit::component::RopeExt;
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    Documentation, Range as LspRange, TextEdit,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvVarCandidate {
    pub name: String,
    /// Resolved value for environment variables; generated sample for dynamic ones.
    pub value: String,
    pub scope: String,
    pub is_secret: bool,
    /// Pre-computed source kind, so consumers never re-parse the scope string.
    pub kind: EnvVarKind,
    /// Human-readable description (dynamic variables only).
    pub description: Option<String>,
}

impl EnvVarCandidate {
    /// Human-readable summary: description for dynamic variables, resolved value otherwise.
    pub fn detail_text(&self) -> &str {
        self.description.as_deref().unwrap_or(&self.value)
    }
}

/// Detects if `text` contains an unclosed `{{` or `{{prefix` (at the end, middle, or cursor).
/// Returns `Some((start_pos, end_pos_to_replace, query_after_braces))`
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

/// Returns all available environment variables matching `query` (case-insensitive)
pub fn get_env_completions(env: &EnvironmentState, query: &str) -> Vec<EnvVarCandidate> {
    let q = query.to_lowercase();
    let mut candidates = Vec::new();
    let mut seen_keys = std::collections::HashSet::new();

    // 1. Active Environment variables (highest priority)
    if let Some(active_env) = env.active_environment() {
        for v in &active_env.variables {
            if !seen_keys.contains(&v.key) {
                if q.is_empty() || v.key.to_lowercase().contains(&q) {
                    candidates.push(EnvVarCandidate {
                        name: v.key.clone(),
                        value: if v.secret {
                            "••••••••".to_string()
                        } else {
                            v.resolved_value().to_string()
                        },
                        scope: format!("Active: {}", active_env.name),
                        is_secret: v.secret,
                        kind: EnvVarKind::Active,
                        description: None,
                    });
                }
                seen_keys.insert(v.key.clone());
            }
        }
    }

    // 2. Global Environment variables
    for v in &env.global_environment.variables {
        if !seen_keys.contains(&v.key) {
            if q.is_empty() || v.key.to_lowercase().contains(&q) {
                candidates.push(EnvVarCandidate {
                    name: v.key.clone(),
                    value: if v.secret {
                        "••••••••".to_string()
                    } else {
                        v.resolved_value().to_string()
                    },
                    scope: "Global".to_string(),
                    is_secret: v.secret,
                    kind: EnvVarKind::Global,
                    description: None,
                });
            }
            seen_keys.insert(v.key.clone());
        }
    }

    // 3. Dynamic variables ($guid, $timestamp, etc.)
    for (d_name, d_desc) in zest_core::scripting::FAKE_VAR_CATALOG {
        if q.is_empty()
            || d_name.to_lowercase().contains(&q)
            || d_name.trim_start_matches('$').to_lowercase().contains(&q)
        {
            candidates.push(EnvVarCandidate {
                name: d_name.to_string(),
                value: zest_core::scripting::generate_fake_var_value(d_name).unwrap_or_default(),
                scope: "Dynamic".to_string(),
                is_secret: false,
                kind: EnvVarKind::Dynamic,
                description: Some(d_desc.to_string()),
            });
        }
    }

    candidates
}

/// Resolves variable details: `(value, scope, is_resolved)`
pub fn get_variable_details(env: &EnvironmentState, var_name: &str) -> (String, String, bool) {
    // 1. Check Active Env
    if let Some(active_env) = env.active_environment() {
        if let Some(v) = active_env.variables.iter().find(|x| x.key == var_name) {
            let val = if v.secret {
                "••••••••".to_string()
            } else {
                v.resolved_value().to_string()
            };
            return (val, format!("Active: {}", active_env.name), true);
        }
    }

    // 2. Check Global Env
    if let Some(v) = env
        .global_environment
        .variables
        .iter()
        .find(|x| x.key == var_name)
    {
        let val = if v.secret {
            "••••••••".to_string()
        } else {
            v.resolved_value().to_string()
        };
        return (val, "Global".to_string(), true);
    }

    // 3. Check dynamic variables
    if var_name.starts_with('$') {
        if let Some(fake) = zest_core::scripting::generate_fake_var_value(var_name) {
            return (format!("Dynamic: {}", fake), "Dynamic".to_string(), true);
        }
    }

    (
        "Unresolved variable".to_string(),
        "Missing".to_string(),
        false,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvVarKind {
    Active,
    Global,
    Dynamic,
    Unresolved,
}

/// Returns `(EnvVarKind, text_and_border_color, background_tint_color)`
pub fn get_variable_kind_and_color(
    scope: &str,
    is_res: bool,
) -> (EnvVarKind, gpui_kit::Hsla, gpui_kit::Hsla) {
    if !is_res {
        // Red / Unresolved
        (
            EnvVarKind::Unresolved,
            crate::theme::hex_to_hsla(0xef4444),
            crate::theme::hex_alpha_to_hsla(0xef4444, 0.14),
        )
    } else if scope.starts_with("Active") {
        // Sky Blue / Active Env
        (
            EnvVarKind::Active,
            crate::theme::hex_to_hsla(0x38bdf8),
            crate::theme::hex_alpha_to_hsla(0x38bdf8, 0.14),
        )
    } else if scope == "Global" {
        // Purple / Global Env
        (
            EnvVarKind::Global,
            crate::theme::hex_to_hsla(0xc084fc),
            crate::theme::hex_alpha_to_hsla(0xc084fc, 0.14),
        )
    } else {
        // Amber / Dynamic ($uuid, $timestamp, etc.)
        (
            EnvVarKind::Dynamic,
            crate::theme::hex_to_hsla(0xf59e0b),
            crate::theme::hex_alpha_to_hsla(0xf59e0b, 0.14),
        )
    }
}

/// Builds a compact hover tooltip that shows only the resolved value of a variable.
pub fn variable_value_tooltip(
    val: &str,
    scope: &str,
    is_res: bool,
) -> gpui_kit::component::tooltip::Tooltip {
    let (_kind, color, _bg_tint) = get_variable_kind_and_color(scope, is_res);

    if is_res {
        let text = if val.is_empty() {
            "\"\"".to_string()
        } else {
            val.to_string()
        };
        gpui_kit::component::tooltip::Tooltip::new(text)
            .font_family(".SystemMonoFont")
            .text_xs()
    } else {
        gpui_kit::component::tooltip::Tooltip::new("Variable not found")
            .text_color(color)
            .text_xs()
    }
}

/// Builds `InputContent` with inline tokens for all variable spans `{{var}}` found in `text`.
pub fn build_tokenized_content(text: &str, env: &EnvironmentState) -> InputContent {
    if !text.contains("{{") {
        return InputContent::new(text);
    }
    let mut content = InputContent::new(text);
    let effective_env = env.effective_environment();
    let spans = zest_core::interpolation::scan_variables(text, Some(&effective_env));
    for span in spans {
        if span.start < text.len() && span.end <= text.len() {
            let matched_text = &text[span.start..span.end];
            let token =
                InlineToken::new(span.name.clone(), matched_text).with_label(span.name.clone());
            if let Ok(new_content) = content.clone().with_token(span.start..span.end, token) {
                content = new_content;
            }
        }
    }
    content
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
        rope: &gpui_kit::component::input::Rope,
        offset: usize,
        _trigger: CompletionContext,
        _window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<CompletionResponse>> {
        let full_text = rope.to_string();
        let safe_offset = offset.min(full_text.len());
        let slice = &full_text[..safe_offset];

        if let Some(brace_pos) = slice.rfind("{{") {
            let after = &slice[brace_pos + 2..];
            if !after.contains("}}") && !after.contains('\n') && !after.contains('"') {
                let env = self.env_state.read(cx);
                let candidates = get_env_completions(env, after);

                let start_pos = rope.offset_to_position(brace_pos);
                let end_pos = rope.offset_to_position(safe_offset);

                let items: Vec<CompletionItem> = candidates
                    .into_iter()
                    .map(|c| {
                        let label = format!("{{{{{}}}}}", c.name);
                        let insert_text = format!("{{{{{}}}}}", c.name);
                        CompletionItem {
                            label: label.clone(),
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
                                new_text: insert_text,
                            })),
                            ..Default::default()
                        }
                    })
                    .collect();

                return Task::ready(Ok(CompletionResponse::Array(items)));
            }
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

#[derive(Default, Clone, Debug)]
pub struct AutocompleteNavState {
    pub selected_index: usize,
    pub dismissed: bool,
    pub last_query: Option<String>,
}

/// Handles keyboard navigation events (`ArrowDown`, `ArrowUp`, `Enter`, `Tab`, `Escape`)
/// for autocomplete popups. Returns `true` if the event was consumed.
pub fn handle_autocomplete_keydown<F>(
    event: &KeyDownEvent,
    candidates: &[EnvVarCandidate],
    nav_state: &Entity<AutocompleteNavState>,
    scroll_handle: &ScrollHandle,
    window: &mut Window,
    cx: &mut App,
    on_select: F,
) -> bool
where
    F: FnOnce(&EnvVarCandidate, &mut Window, &mut App),
{
    match event.keystroke.key.as_str() {
        "down" | "arrowdown" => {
            cx.stop_propagation();
            let len = candidates.len();
            if len > 0 {
                let next_idx = nav_state.update(cx, |st, cx| {
                    st.selected_index = (st.selected_index + 1) % len;
                    cx.notify();
                    st.selected_index
                });
                scroll_handle.scroll_to_item(next_idx);
            }
            true
        }
        "up" | "arrowup" => {
            cx.stop_propagation();
            let len = candidates.len();
            if len > 0 {
                let prev_idx = nav_state.update(cx, |st, cx| {
                    if st.selected_index == 0 {
                        st.selected_index = len - 1;
                    } else {
                        st.selected_index -= 1;
                    }
                    cx.notify();
                    st.selected_index
                });
                scroll_handle.scroll_to_item(prev_idx);
            }
            true
        }
        "enter" | "tab" => {
            cx.stop_propagation();
            let sel_idx = nav_state.read(cx).selected_index;
            if let Some(cand) = candidates.get(sel_idx) {
                let cand_clone = cand.clone();
                nav_state.update(cx, |st, cx| {
                    st.dismissed = true;
                    cx.notify();
                });
                on_select(&cand_clone, window, cx);
            }
            true
        }
        "escape" => {
            cx.stop_propagation();
            nav_state.update(cx, |st, cx| {
                st.dismissed = true;
                cx.notify();
            });
            true
        }
        _ => false,
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
        // Middle variable with path after it
        assert_eq!(
            detect_unclosed_env_var("https://api.com/{{host/users"),
            Some((16, 22, "host".to_string()))
        );
        // Middle variable while second variable is already closed
        assert_eq!(
            detect_unclosed_env_var("https://api.com/{{host/users/{{id}}"),
            Some((16, 22, "host".to_string()))
        );
        // Variable at start unclosed
        assert_eq!(
            detect_unclosed_env_var("{{base/api"),
            Some((0, 6, "base".to_string()))
        );
        // Variable at start closed
        assert_eq!(detect_unclosed_env_var("{{base_url}}/api"), None);
        // Variable at end
        assert_eq!(
            detect_unclosed_env_var("{{base_url"),
            Some((0, 10, "base_url".to_string()))
        );
        // Dynamic variable unclosed
        assert_eq!(
            detect_unclosed_env_var("https://api.com/{{$guid"),
            Some((16, 23, "$guid".to_string()))
        );

        // Test replacement in middle preserves prefix and suffix
        let text = "https://api.com/{{host/users";
        let (start, end, _) = detect_unclosed_env_var(text).unwrap();
        let mut replaced = text.to_string();
        replaced.replace_range(start..end, "{{base_url}}");
        assert_eq!(replaced, "https://api.com/{{base_url}}/users");
    }

    #[test]
    fn test_get_env_completions_and_details() {
        let store = std::sync::Arc::new(zest_core::storage::FsWorkspaceStore::new(
            std::path::Path::new("./test-env-comp"),
        ));
        let mut env = EnvironmentState::new(store, "default".to_string(), None);
        env.global_environment
            .variables
            .push(zest_core::models::EnvironmentVariable {
                key: "apiKey".to_string(),
                initial_value: "secret-123".to_string(),
                current_value: "".to_string(),
                secret: true,
            });

        let mut custom_env = zest_core::models::Environment::new("default", "Staging");
        custom_env
            .variables
            .push(zest_core::models::EnvironmentVariable {
                key: "baseUrl".to_string(),
                initial_value: "https://staging.api.com".to_string(),
                current_value: "".to_string(),
                secret: false,
            });
        let env_id = custom_env.id.clone();
        env.environments.push(custom_env);
        env.active_environment_id = Some(env_id);

        // 1. Completion test
        let completions = get_env_completions(&env, "base");
        assert!(completions.iter().any(|c| c.name == "baseUrl"
            && c.scope.contains("Staging")
            && c.kind == EnvVarKind::Active
            && c.description.is_none()));

        let global_comp = get_env_completions(&env, "api");
        assert!(global_comp
            .iter()
            .any(|c| c.name == "apiKey" && c.is_secret && c.kind == EnvVarKind::Global));

        // Dynamic completion test
        let dyn_comp = get_env_completions(&env, "guid");
        assert!(dyn_comp.iter().any(|c| c.name == "$guid"
            && c.kind == EnvVarKind::Dynamic
            && c.description.as_deref() == Some("Generates random UUID v4")
            && c.detail_text() == "Generates random UUID v4"));

        // 2. Variable details test
        let (val, scope, is_res) = get_variable_details(&env, "baseUrl");
        assert_eq!(val, "https://staging.api.com");
        assert!(scope.contains("Staging"));
        assert!(is_res);

        let (val, scope, is_res) = get_variable_details(&env, "apiKey");
        assert_eq!(val, "••••••••");
        assert_eq!(scope, "Global");
        assert!(is_res);

        let (_, _, is_res) = get_variable_details(&env, "nonExistent");
        assert!(!is_res);
    }

    #[test]
    fn test_autocomplete_nav_state() {
        let mut state = AutocompleteNavState::default();
        assert_eq!(state.selected_index, 0);
        assert!(!state.dismissed);
        assert_eq!(state.last_query, None);

        state.last_query = Some("foo".to_string());
        state.selected_index = 2;
        state.dismissed = true;

        assert_eq!(state.selected_index, 2);
        assert!(state.dismissed);
        assert_eq!(state.last_query, Some("foo".to_string()));
    }
}
