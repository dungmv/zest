//! Single-line `InputState` inline-token attachment and undo-safe retokenization.

use crate::state::EnvironmentState;
use gpui_kit::component::input::{InlineToken, InputContent, InputState};
use gpui_kit::{Context, Window};

/// Builds `InputContent` with inline tokens for all variable spans `{{var}}` found in `text`.
pub fn build_tokenized_content(text: &str, env: &EnvironmentState) -> InputContent {
    if !text.contains("{{") {
        return InputContent::new(text);
    }
    let mut content = InputContent::new(text);
    let effective_env = env.effective_environment();
    let spans = zest_core::env::scan_variables(text, Some(&effective_env));
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

/// Converts raw `{{var}}` spans of the input text into inline variable tokens.
///
/// Tokens are attached programmatically via `build_tokenized_content` when a
/// value is set by the app (autocomplete pick, request sync), but text typed
/// or pasted by the user arrives without tokens. This hook re-scans the
/// current text and attaches a token to every closed `{{var}}` span that does
/// not carry one yet. The token text always equals the span text, so the
/// underlying value never changes, and the caret is put back where the edit
/// left it. Returns `true` when at least one span was converted.
pub fn attach_missing_var_tokens(
    inp: &mut InputState,
    window: &mut Window,
    cx: &mut Context<InputState>,
) -> bool {
    let text = inp.value().to_string();
    if !text.contains("{{") {
        return false;
    }
    let caret_before = inp.cursor();
    let mut converted = false;
    for span in zest_core::env::scan_variable_spans(&text) {
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
    // Conversion keeps the text identical but parks the caret at the end of
    // the last converted span; restore the position the edit left behind.
    if converted && inp.cursor() != caret_before {
        inp.set_selected_range(caret_before..caret_before, cx);
    }
    converted
}

/// Remembers the input value the last time retokenization ran.
///
/// Converting a span re-emits `InputEvent::Change` even though the text is
/// unchanged, and undoing a conversion restores token-less text with an
/// identical value. Both must be ignored so the undo stack is never poisoned.
#[derive(Default)]
pub struct RetokenizeGuard {
    prev_value: Option<String>,
}

impl RetokenizeGuard {
    /// Returns `true` when `value` differs from the last processed change and
    /// retokenization plus request sync should run.
    pub fn should_process(&mut self, value: &str) -> bool {
        if self.prev_value.as_deref() == Some(value) {
            return false;
        }
        self.prev_value = Some(value.to_string());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::component::input::{Input, InputEvent, Redo, Undo};
    #[cfg(test)]
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{AppContext as _, Entity, IntoElement, Render, Subscription, TestAppContext};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[gpui_kit::test]
    fn test_attach_missing_var_tokens_converts_closed_spans(cx: &mut TestAppContext) {
        let (input, cx) = cx.add_window_view(InputState::new);
        cx.update(|window, cx| {
            input.update(cx, |inp, cx| {
                inp.set_value("https://api.com/{{host}}/x", window, cx);
                assert!(inp.tokens().is_empty());

                assert!(attach_missing_var_tokens(inp, window, cx));
                assert_eq!(inp.value(), "https://api.com/{{host}}/x");
                let tokens = inp.tokens();
                assert_eq!(tokens.len(), 1);
                assert_eq!(tokens[0].token().id().to_string(), "host");
                assert_eq!(tokens[0].range(), 16..24);

                assert!(!attach_missing_var_tokens(inp, window, cx));
                assert_eq!(inp.tokens().len(), 1);
            });
        });
    }

    #[gpui_kit::test]
    fn test_attach_missing_var_tokens_skips_unclosed(cx: &mut TestAppContext) {
        let (input, cx) = cx.add_window_view(InputState::new);
        cx.update(|window, cx| {
            input.update(cx, |inp, cx| {
                inp.set_value("https://api.com/{{host", window, cx);
                assert!(!attach_missing_var_tokens(inp, window, cx));
                assert!(inp.tokens().is_empty());
            });
        });
    }

    #[gpui_kit::test]
    fn test_attach_missing_var_tokens_handles_multiple_spans(cx: &mut TestAppContext) {
        let (input, cx) = cx.add_window_view(InputState::new);
        cx.update(|window, cx| {
            input.update(cx, |inp, cx| {
                inp.set_value("{{a}}/{{b}}", window, cx);
                assert!(attach_missing_var_tokens(inp, window, cx));
                assert_eq!(inp.value(), "{{a}}/{{b}}");
                let tokens = inp.tokens();
                assert_eq!(tokens.len(), 2);
                assert_eq!(tokens[0].token().id().to_string(), "a");
                assert_eq!(tokens[1].token().id().to_string(), "b");
                assert!(!attach_missing_var_tokens(inp, window, cx));
            });
        });
    }

    #[gpui_kit::test]
    fn test_attach_missing_var_tokens_preserves_caret(cx: &mut TestAppContext) {
        let (input, cx) = cx.add_window_view(InputState::new);
        cx.update(|window, cx| {
            input.update(cx, |inp, cx| {
                inp.set_value("a", window, cx);
                inp.insert("{{h}}/x/{{b}}/tail", window, cx);
                assert_eq!(inp.cursor(), 19);
                assert_eq!(inp.tokens().len(), 0);

                assert!(attach_missing_var_tokens(inp, window, cx));
                assert_eq!(inp.value(), "a{{h}}/x/{{b}}/tail");
                assert_eq!(inp.tokens().len(), 2);
                assert_eq!(inp.cursor(), 19, "conversion must not steal the caret");
            });
        });
    }

    struct RetokHost {
        input: Entity<InputState>,
        changes: usize,
        _sub: Subscription,
    }
    impl Render for RetokHost {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            Input::new(&self.input)
        }
    }

    #[gpui_kit::test]
    fn test_retokenize_guard_lets_undo_progress(cx: &mut TestAppContext) {
        cx.update(gpui_kit::component::init);
        let (host, cx) = cx.add_window_view(|window, cx| {
            let input = cx.new(|cx| InputState::new(window, cx));
            let guard = Rc::new(RefCell::new(RetokenizeGuard::default()));
            let _sub = {
                let guard = guard.clone();
                cx.subscribe_in(
                    &input,
                    window,
                    move |this: &mut RetokHost, input, ev, window, cx| {
                        if let InputEvent::Change = ev {
                            this.changes += 1;
                            let val = input.read(cx).value().to_string();
                            if guard.borrow_mut().should_process(&val) {
                                input.update(cx, |inp, cx| {
                                    attach_missing_var_tokens(inp, window, cx);
                                });
                            }
                        }
                    },
                )
            };
            RetokHost {
                input,
                changes: 0,
                _sub,
            }
        });
        let input = host.read_with(cx, |h, _| h.input.clone());
        let snapshot = |cx: &mut gpui_kit::VisualTestContext| -> (String, usize, usize) {
            let (v, t) = input.read_with(cx, |i, _| (i.value().to_string(), i.tokens().len()));
            (v, t, host.read_with(cx, |h, _| h.changes))
        };

        cx.update(|window, cx| {
            input.update(cx, |inp, cx| {
                inp.focus(window, cx);
                inp.insert("a", window, cx);
            })
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            input.update(cx, |inp, cx| inp.insert("{{h}}/x/{{b}}/tail", window, cx))
        });
        cx.run_until_parked();

        let (value, tokens, changes) = snapshot(cx);
        assert_eq!(value, "a{{h}}/x/{{b}}/tail");
        assert_eq!(tokens, 2);
        let changes_after_paste = changes;

        cx.dispatch_action(Undo);
        cx.run_until_parked();
        let (value, tokens, changes) = snapshot(cx);
        assert_eq!(value, "a{{h}}/x/{{b}}/tail");
        assert_eq!(tokens, 1);
        assert_eq!(changes, changes_after_paste + 1, "no re-attach after undo");

        cx.dispatch_action(Undo);
        cx.run_until_parked();
        let (_, tokens, _) = snapshot(cx);
        assert_eq!(tokens, 0);

        cx.dispatch_action(Undo);
        cx.run_until_parked();
        let (value, tokens, _) = snapshot(cx);
        assert_eq!(value, "a", "undo reaches the text edit itself");
        assert_eq!(tokens, 0);

        cx.dispatch_action(Redo);
        cx.run_until_parked();
        cx.dispatch_action(Redo);
        cx.run_until_parked();
        cx.dispatch_action(Redo);
        cx.run_until_parked();
        let (value, tokens, _) = snapshot(cx);
        assert_eq!(value, "a{{h}}/x/{{b}}/tail");
        assert_eq!(tokens, 2);
    }

    #[gpui_kit::test]
    fn test_replace_text_in_range_delivers_composed_vietnamese_char(cx: &mut TestAppContext) {
        cx.update(gpui_kit::component::init);
        let (host, cx) = cx.add_window_view(|window, cx| {
            let input = cx.new(|cx| InputState::new(window, cx));
            let sub: Subscription = cx.subscribe_in(
                &input,
                window,
                |_: &mut RetokHost,
                 _: &Entity<InputState>,
                 _: &gpui_kit::component::input::InputEvent,
                 _,
                 _| {},
            );
            RetokHost {
                input,
                changes: 0,
                _sub: sub,
            }
        });
        let input = host.read_with(cx, |h, _| h.input.clone());

        cx.update(|window, cx| {
            input.update(cx, |inp, cx| {
                inp.focus(window, cx);
                use gpui_kit::EntityInputHandler as _;
                inp.replace_text_in_range(None, "á", window, cx);
                inp.replace_text_in_range(None, "ộ", window, cx);
                inp.replace_text_in_range(None, "ế", window, cx);
                inp.replace_text_in_range(None, "Đ", window, cx);
                assert_eq!(
                    inp.value(),
                    "áộếĐ",
                    "platform-delivered composed chars must arrive intact"
                );
            });
        });
    }

    #[gpui_kit::test]
    fn test_vietnamese_backspace_ime_storm_around_tokens(cx: &mut TestAppContext) {
        cx.update(gpui_kit::component::init);
        let (host, cx) = cx.add_window_view(|window, cx| {
            let input = cx.new(|cx| InputState::new(window, cx));
            let guard = Rc::new(RefCell::new(RetokenizeGuard::default()));
            let _sub = {
                let guard = guard.clone();
                cx.subscribe_in(
                    &input,
                    window,
                    move |_: &mut RetokHost, input, ev, window, cx| {
                        if let InputEvent::Change = ev {
                            let val = input.read(cx).value().to_string();
                            if guard.borrow_mut().should_process(&val) {
                                input.update(cx, |inp, cx| {
                                    attach_missing_var_tokens(inp, window, cx);
                                });
                            }
                        }
                    },
                )
            };
            RetokHost {
                input,
                changes: 0,
                _sub,
            }
        });
        let input = host.read_with(cx, |h, _| h.input.clone());

        cx.update(|window, cx| {
            input.update(cx, |inp, cx| inp.focus(window, cx));
        });

        cx.update(|window, cx| window.input("https://api.com/{{host}}/{{", cx));
        cx.update(|window, cx| window.input("á", cx));

        for (plain, composed) in [("a", "á"), ("e", "ế"), ("a", "ấ"), ("o", "ộ"), ("u", "ữ")]
        {
            cx.update(|window, cx| window.press("backspace", cx));
            cx.update(|window, cx| window.input(composed, cx));
            cx.update(|window, cx| window.input(plain, cx));
            cx.update(|window, cx| window.press("backspace", cx));
        }

        for _ in 0..6 {
            cx.update(|window, cx| window.press("backspace", cx));
        }

        cx.update(|window, cx| {
            window.input("p", cx);
            window.press("backspace", cx);
            window.input("ộ", cx);
        });

        let value = input.read_with(cx, |i, _| i.value().to_string());
        let token_count = input.read_with(cx, |i, _| i.tokens().len());
        assert!(
            value.starts_with("https://api.com"),
            "URL prefix must survive the storm, got {value:?}"
        );
        assert!(token_count <= 1, "at most the host span carries a token");
    }
}
