use crate::components::script_editor::{ScriptCompletionProvider, ScriptEditorKind};
use crate::state::{ComposerSyncScope, EnvironmentState, RequestSessionState, WorkspaceDataState};
use gpui_kit::component::input::{
    EditorState as CodeEditorState, GutterDecorationCollection, InputState, TextareaState,
};
use gpui_kit::{App, AppContext as _, Entity, Window};
use std::rc::Rc;
use zest_core::models::{Auth, Protocol, RequestBody};

type HighlighterFactory =
    Rc<dyn Fn(&str) -> Option<Box<dyn gpui_kit::component::input::InputHighlighter>>>;

/// gRPC composer inputs: the message editor plus everything that decorates
/// it (proto path, validation gutter marks, hover gate).
pub struct GrpcComposerFields {
    /// gRPC request message JSON editor (persisted into `GrpcPayload`).
    pub message_editor: Entity<CodeEditorState>,
    /// `.proto` file path input for gRPC requests (persisted).
    pub proto_path_input: Entity<InputState>,
    /// Red gutter marks for gRPC message validation errors.
    pub error_marks: GutterDecorationCollection,
    /// Whether the pointer currently hovers the gRPC message editor —
    /// gates the in-editor diagnostic tooltip (hover-only by design).
    pub diagnostics_hover: Rc<std::cell::Cell<bool>>,
}

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
    pub grpc: GrpcComposerFields,
    /// WebSocket outgoing message box (session-only, never persisted).
    pub ws_message_textarea: Entity<TextareaState>,
    /// Request that currently owns `ws_message_textarea` content.
    ws_message_owner: Option<String>,
}

impl ComposerInputs {
    pub fn new(
        window: &mut Window,
        env_state: Entity<EnvironmentState>,
        session_state: Entity<RequestSessionState>,
        workspace_state: Entity<WorkspaceDataState>,
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
                crate::components::completion::env_autocomplete::EnvCompletionProvider::new(
                    env_state.clone(),
                ),
            ));
            state.language("json").line_number(true).folding(true)
        });

        let xml_editor = cx.new(|cx| {
            let mut state = CodeEditorState::new(window, cx);
            state.set_highlighter_factory(highlighter_factory.clone(), cx);
            state.lsp_mut().completion_provider = Some(Rc::new(
                crate::components::completion::env_autocomplete::EnvCompletionProvider::new(
                    env_state.clone(),
                ),
            ));
            state.language("xml").line_number(true).folding(true)
        });

        let raw_textarea = cx.new(|cx| TextareaState::new(window, cx));

        let grpc_message_editor = cx.new(|cx| {
            let mut state = CodeEditorState::new(window, cx);
            state.set_highlighter_factory(highlighter_factory.clone(), cx);
            state.lsp_mut().completion_provider = Some(Rc::new(
                crate::components::completion::proto_json::ProtoJsonCompletionProvider::new(
                    env_state.clone(),
                    session_state.clone(),
                    workspace_state.clone(),
                ),
            ));
            state.language("json").line_number(true).folding(true)
        });
        let grpc_error_marks = grpc_message_editor.update(cx, |state, cx| {
            state.create_gutter_decorations_collection(vec![], cx)
        });
        let grpc_diagnostics_hover = std::rc::Rc::new(std::cell::Cell::new(false));

        let ws_message_textarea = cx.new(|cx| TextareaState::new(window, cx));

        let grpc_proto_path_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Path to .proto file…"));

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
            grpc: GrpcComposerFields {
                message_editor: grpc_message_editor,
                proto_path_input: grpc_proto_path_input,
                error_marks: grpc_error_marks,
                diagnostics_hover: grpc_diagnostics_hover,
            },
            ws_message_textarea,
            ws_message_owner: None,
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
                crate::components::completion::env_autocomplete::build_tokenized_content(
                    &req.url, env,
                )
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
                            crate::components::completion::env_autocomplete::build_tokenized_content(
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
                            crate::components::completion::env_autocomplete::build_tokenized_content(
                                username, env,
                            ),
                            window,
                            cx,
                        );
                    });
                    self.basic_pass_input.update(cx, |inp, cx| {
                        let env = env_p.read(cx);
                        inp.set_value(
                            crate::components::completion::env_autocomplete::build_tokenized_content(
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
                            crate::components::completion::env_autocomplete::build_tokenized_content(key, env),
                            window,
                            cx,
                        );
                    });
                    self.apikey_val_input.update(cx, |inp, cx| {
                        let env = env_v.read(cx);
                        inp.set_value(
                            crate::components::completion::env_autocomplete::build_tokenized_content(
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

        // Protocol-specific message editors. gRPC payload is persisted in the
        // request file; the WebSocket outgoing box is session-only, so it is
        // cleared when switching to a different request.
        if matches!(scope, ComposerSyncScope::All)
            && self.ws_message_owner.as_deref() != Some(req_id)
        {
            self.ws_message_owner = Some(req_id.to_string());
            self.ws_message_textarea.update(cx, |txt, cx| {
                txt.set_value(String::new(), window, cx);
            });
        }
        if req.protocol == Protocol::Grpc {
            let message_json = req.grpc_message_json().to_string();
            self.grpc.message_editor.update(cx, |txt, cx| {
                txt.set_value(message_json, window, cx);
            });
            let proto_path = req
                .grpc
                .as_ref()
                .and_then(|g| g.proto_path.clone())
                .unwrap_or_default();
            self.grpc.proto_path_input.update(cx, |inp, cx| {
                inp.set_value(proto_path, window, cx);
            });
        } else {
            self.grpc.message_editor.update(cx, |txt, cx| {
                txt.set_value(String::new(), window, cx);
            });
        }
    }
}
