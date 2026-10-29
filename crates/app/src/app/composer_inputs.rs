use crate::components::script_editor::{ScriptCompletionProvider, ScriptEditorKind};
use crate::state::{ComposerSyncScope, EnvironmentState, WorkspaceDataState};
use gpui_kit::component::input::{EditorState as CodeEditorState, InputState, TextareaState};
use gpui_kit::{App, AppContext as _, Entity, Window};
use std::rc::Rc;
use zest_core::models::{Auth, RequestBody};

type HighlighterFactory =
    Rc<dyn Fn(&str) -> Option<Box<dyn gpui_kit::component::input::InputHighlighter>>>;

pub struct ComposerInputs {
    pub url_input: Entity<InputState>,
    pub json_editor: Entity<CodeEditorState>,
    pub xml_editor: Entity<CodeEditorState>,
    pub raw_textarea: Entity<TextareaState>,
    pub pre_script_editor: Entity<CodeEditorState>,
    pub test_script_editor: Entity<CodeEditorState>,
    pub bearer_token_input: Entity<InputState>,
    pub basic_user_input: Entity<InputState>,
    pub basic_pass_input: Entity<InputState>,
    pub apikey_key_input: Entity<InputState>,
    pub apikey_val_input: Entity<InputState>,
}

impl ComposerInputs {
    pub fn new(
        window: &mut Window,
        env_state: Entity<EnvironmentState>,
        highlighter_factory: HighlighterFactory,
        cx: &mut App,
    ) -> Self {
        let url_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Enter request URL (e.g. https://httpbin.org/get)")
        });

        let json_editor = cx.new(|cx| {
            let mut state = CodeEditorState::new(window, cx);
            state.set_highlighter_factory(highlighter_factory.clone(), cx);
            state.lsp_mut().completion_provider = Some(Rc::new(
                crate::components::env_autocomplete::EnvCompletionProvider::new(env_state.clone()),
            ));
            state.language("json").line_number(true).folding(true)
        });

        let xml_editor = cx.new(|cx| {
            let mut state = CodeEditorState::new(window, cx);
            state.set_highlighter_factory(highlighter_factory.clone(), cx);
            state.lsp_mut().completion_provider = Some(Rc::new(
                crate::components::env_autocomplete::EnvCompletionProvider::new(env_state.clone()),
            ));
            state.language("xml").line_number(true).folding(true)
        });

        let raw_textarea = cx.new(|cx| TextareaState::new(window, cx));

        let pre_script_editor = cx.new(|cx| {
            let mut state = CodeEditorState::new(window, cx);
            state.set_highlighter_factory(highlighter_factory.clone(), cx);
            state.lsp_mut().completion_provider =
                Some(Rc::new(ScriptCompletionProvider::with_environment(
                    ScriptEditorKind::PreRequest,
                    env_state.clone(),
                )));
            state.language("javascript").line_number(true).folding(true)
        });

        let test_script_editor = cx.new(|cx| {
            let mut state = CodeEditorState::new(window, cx);
            state.set_highlighter_factory(highlighter_factory, cx);
            state.lsp_mut().completion_provider = Some(Rc::new(
                ScriptCompletionProvider::with_environment(ScriptEditorKind::Tests, env_state),
            ));
            state.language("javascript").line_number(true).folding(true)
        });

        let bearer_token_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Enter Bearer token..."));
        let basic_user_input = cx.new(|cx| InputState::new(window, cx).placeholder("Username..."));
        let basic_pass_input = cx.new(|cx| InputState::new(window, cx).placeholder("Password..."));
        let apikey_key_input = cx.new(|cx| InputState::new(window, cx).placeholder("X-API-Key"));
        let apikey_val_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("API key value..."));

        Self {
            url_input,
            json_editor,
            xml_editor,
            raw_textarea,
            pre_script_editor,
            test_script_editor,
            bearer_token_input,
            basic_user_input,
            basic_pass_input,
            apikey_key_input,
            apikey_val_input,
        }
    }

    pub fn sync_inputs(
        &mut self,
        req_id: &str,
        scope: ComposerSyncScope,
        workspace_state: &Entity<WorkspaceDataState>,
        env_state: &Entity<EnvironmentState>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(req) = workspace_state.read(cx).find_request(req_id).cloned() else {
            return;
        };

        if matches!(scope, ComposerSyncScope::All) {
            let tokenized_url = {
                let env = env_state.read(cx);
                crate::components::env_autocomplete::build_tokenized_content(&req.url, env)
            };
            self.url_input.update(cx, |inp, cx| {
                inp.set_value(tokenized_url, window, cx);
            });

            let env_holder = env_state.clone();
            match &req.auth {
                Auth::Bearer { token } => {
                    let env_m = env_holder.clone();
                    self.bearer_token_input.update(cx, |inp, cx| {
                        let env = env_m.read(cx);
                        inp.set_value(
                            crate::components::env_autocomplete::build_tokenized_content(
                                token, env,
                            ),
                            window,
                            cx,
                        );
                    });
                }
                Auth::Basic { username, password } => {
                    let env_u = env_holder.clone();
                    let env_p = env_holder.clone();
                    self.basic_user_input.update(cx, |inp, cx| {
                        let env = env_u.read(cx);
                        inp.set_value(
                            crate::components::env_autocomplete::build_tokenized_content(
                                username, env,
                            ),
                            window,
                            cx,
                        );
                    });
                    self.basic_pass_input.update(cx, |inp, cx| {
                        let env = env_p.read(cx);
                        inp.set_value(
                            crate::components::env_autocomplete::build_tokenized_content(
                                password, env,
                            ),
                            window,
                            cx,
                        );
                    });
                }
                Auth::ApiKey { key, value, .. } => {
                    let env_k = env_holder.clone();
                    let env_v = env_holder.clone();
                    self.apikey_key_input.update(cx, |inp, cx| {
                        let env = env_k.read(cx);
                        inp.set_value(
                            crate::components::env_autocomplete::build_tokenized_content(key, env),
                            window,
                            cx,
                        );
                    });
                    self.apikey_val_input.update(cx, |inp, cx| {
                        let env = env_v.read(cx);
                        inp.set_value(
                            crate::components::env_autocomplete::build_tokenized_content(
                                value, env,
                            ),
                            window,
                            cx,
                        );
                    });
                }
                Auth::Inherit | Auth::None => {
                    self.bearer_token_input.update(cx, |inp, cx| {
                        inp.set_value(String::new(), window, cx);
                    });
                    self.basic_user_input.update(cx, |inp, cx| {
                        inp.set_value(String::new(), window, cx);
                    });
                    self.basic_pass_input.update(cx, |inp, cx| {
                        inp.set_value(String::new(), window, cx);
                    });
                    self.apikey_key_input.update(cx, |inp, cx| {
                        inp.set_value(String::new(), window, cx);
                    });
                    self.apikey_val_input.update(cx, |inp, cx| {
                        inp.set_value(String::new(), window, cx);
                    });
                }
            }

            let pre_script = req
                .script
                .as_ref()
                .and_then(|s| s.pre_request.clone())
                .unwrap_or_default();
            self.pre_script_editor.update(cx, |txt, cx| {
                txt.set_value(pre_script, window, cx);
            });
            let test_script = req
                .script
                .as_ref()
                .and_then(|s| s.post_response.clone())
                .unwrap_or_default();
            self.test_script_editor.update(cx, |txt, cx| {
                txt.set_value(test_script, window, cx);
            });
        }

        match &req.body {
            RequestBody::Json { content: json_str } => {
                self.json_editor.update(cx, |txt, cx| {
                    txt.set_value(json_str.clone(), window, cx);
                });
            }
            RequestBody::Raw {
                content,
                content_type,
            } => {
                if content_type == "application/xml" || content_type == "text/xml" {
                    self.xml_editor.update(cx, |txt, cx| {
                        txt.set_value(content.clone(), window, cx);
                    });
                }
                self.raw_textarea.update(cx, |txt, cx| {
                    txt.set_value(content.clone(), window, cx);
                });
            }
            RequestBody::None
            | RequestBody::FormUrlEncoded { .. }
            | RequestBody::Multipart { .. }
            | RequestBody::Binary { .. } => {
                self.json_editor.update(cx, |txt, cx| {
                    txt.set_value(String::new(), window, cx);
                });
                self.raw_textarea.update(cx, |txt, cx| {
                    txt.set_value(String::new(), window, cx);
                });
            }
        }
    }
}
