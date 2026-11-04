use crate::state::EnvironmentState;
use gpui_kit::component::input::{InlineToken, InputState};
use gpui_kit::{Context, Window};
use zest_core::models::Environment;

/// Converts raw `{{var}}` spans of the input text into inline variable tokens.
///
/// Tokens are attached programmatically via `build_tokenized_content` when a
/// value is set by the app (autocomplete pick, request sync), but text typed
/// or pasted by the user arrives without tokens. This hook re-scans the
/// current text and attaches a token to every closed `{{var}}` span that does
/// not carry one yet. The token text always equals the span text, so the
/// underlying value and the caret position never change. Returns `true` when
/// at least one span was converted.
pub fn attach_missing_var_tokens(
    inp: &mut InputState,
    env: &Environment,
    window: &mut Window,
    cx: &mut Context<InputState>,
) -> bool {
    let text = inp.value().to_string();
    if !text.contains("{{") {
        return false;
    }
    let mut converted = false;
    for span in zest_core::interpolation::scan_variables(&text, Some(env)) {
        if span.end > text.len() || span.start >= span.end {
            continue;
        }
        // Skip spans that already render as tokens.
        if inp
            .tokens()
            .iter()
            .any(|t| t.range() == (span.start..span.end))
        {
            continue;
        }
        let matched_text = &text[span.start..span.end];
        let token = InlineToken::new(span.name.clone(), matched_text).with_label(span.name.clone());
        if inp
            .replace_range_with_token(span.start..span.end, token, window, cx)
            .is_ok()
        {
            converted = true;
        }
    }
    converted
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn test_env() -> EnvironmentState {
        let store = Arc::new(zest_core::storage::FsWorkspaceStore::new(
            std::path::Path::new("./test-env-var-tokens"),
        ));
        EnvironmentState::new(store, "default".to_string(), None)
    }

    #[gpui_kit::test]
    fn test_attach_missing_var_tokens_converts_closed_spans(cx: &mut gpui_kit::TestAppContext) {
        let env = test_env().effective_environment();
        let (input, cx) = cx.add_window_view(InputState::new);
        cx.update(|window, cx| {
            input.update(cx, |inp, cx| {
                inp.set_value("https://api.com/{{host}}/x", window, cx);
                assert!(inp.tokens().is_empty());

                // First pass converts the raw span into a token without
                // touching the text.
                assert!(attach_missing_var_tokens(inp, &env, window, cx));
                assert_eq!(inp.value(), "https://api.com/{{host}}/x");
                let tokens = inp.tokens();
                assert_eq!(tokens.len(), 1);
                assert_eq!(tokens[0].token().id().to_string(), "host");
                assert_eq!(tokens[0].range(), 16..24);

                // Second pass is a no-op: the span already carries a token.
                assert!(!attach_missing_var_tokens(inp, &env, window, cx));
                assert_eq!(inp.tokens().len(), 1);
            });
        });
    }

    #[gpui_kit::test]
    fn test_attach_missing_var_tokens_skips_unclosed(cx: &mut gpui_kit::TestAppContext) {
        let env = test_env().effective_environment();
        let (input, cx) = cx.add_window_view(InputState::new);
        cx.update(|window, cx| {
            input.update(cx, |inp, cx| {
                inp.set_value("https://api.com/{{host", window, cx);
                assert!(!attach_missing_var_tokens(inp, &env, window, cx));
                assert!(inp.tokens().is_empty());
            });
        });
    }

    #[gpui_kit::test]
    fn test_attach_missing_var_tokens_handles_multiple_spans(cx: &mut gpui_kit::TestAppContext) {
        let env = test_env().effective_environment();
        let (input, cx) = cx.add_window_view(InputState::new);
        cx.update(|window, cx| {
            input.update(cx, |inp, cx| {
                inp.set_value("{{a}}/{{b}}", window, cx);
                assert!(attach_missing_var_tokens(inp, &env, window, cx));
                assert_eq!(inp.value(), "{{a}}/{{b}}");
                let tokens = inp.tokens();
                assert_eq!(tokens.len(), 2);
                assert_eq!(tokens[0].token().id().to_string(), "a");
                assert_eq!(tokens[1].token().id().to_string(), "b");

                // Idempotent on repeat runs.
                assert!(!attach_missing_var_tokens(inp, &env, window, cx));
            });
        });
    }
}
