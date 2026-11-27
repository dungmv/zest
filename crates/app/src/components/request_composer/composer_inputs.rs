use crate::components::script_editor::{ScriptCompletionProvider, ScriptEditorKind};
use crate::env::{attach_env_lsp, EnvCompletionProvider, EnvEditorDecorations};
use crate::state::{
    ComposerSyncScope, EnvironmentState, ModalState, RequestSessionState, WorkspaceDataState,
};
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

/// GraphQL composer inputs: the query/variables dual-pane editors plus the
/// operation name input, all persisted into `GraphQLPayload`.
pub struct GraphQLComposerFields {
    /// GraphQL query/mutation document editor.
    pub query_editor: Entity<CodeEditorState>,
    /// Variables JSON editor.
    pub variables_editor: Entity<CodeEditorState>,
    /// Optional operation name for multi-operation documents.
    pub operation_name_input: Entity<InputState>,
    /// Red gutter marks for variables JSON validation errors.
    pub variables_marks: GutterDecorationCollection,
    /// Whether the pointer currently hovers the variables editor —
    /// gates the in-editor diagnostic tooltip (hover-only by design).
    pub diagnostics_hover: Rc<std::cell::Cell<bool>>,
}

/// Environment variable decoration handles for all multi-line code editors in the composer.
pub struct EnvEditorDecorationSet {
    pub json: EnvEditorDecorations,
    pub xml: EnvEditorDecorations,
    pub grpc_message: EnvEditorDecorations,
    pub graphql_query: EnvEditorDecorations,
    pub graphql_variables: EnvEditorDecorations,
}

/// Auth tab input handles (Bearer, Basic, and API Key fields).
pub struct AuthComposerFields {
    pub bearer_token_input: Entity<InputState>,
    pub basic_user_input: Entity<InputState>,
    pub basic_pass_input: Entity<InputState>,
    pub apikey_key_input: Entity<InputState>,
    pub apikey_val_input: Entity<InputState>,
}

pub struct ComposerInputs {
    pub url_input: Entity<InputState>,
    pub json_editor: Entity<CodeEditorState>,
    pub xml_editor: Entity<CodeEditorState>,
    pub raw_textarea: Entity<TextareaState>,
    pub pre_script_editor: Entity<CodeEditorState>,
    pub test_script_editor: Entity<CodeEditorState>,
    pub auth: AuthComposerFields,
    pub grpc: GrpcComposerFields,
    pub graphql: GraphQLComposerFields,
    pub env_decorations: EnvEditorDecorationSet,
    /// WebSocket outgoing message box (session-only, never persisted).
    pub ws_message_textarea: Entity<TextareaState>,
    /// Request that currently owns `ws_message_textarea` content.
    ws_message_owner: Option<String>,
}

impl ComposerInputs {
    pub fn new(
        window: &mut Window,
        env_state: Entity<EnvironmentState>,
        modal_state: Entity<ModalState>,
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
            state.lsp_mut().completion_provider =
                Some(Rc::new(EnvCompletionProvider::new(env_state.clone())));
            attach_env_lsp(&mut state, env_state.clone(), modal_state.clone());
            state.language("json").line_number(true).folding(true)
        });
        let json_env_dec = json_editor.update(cx, EnvEditorDecorations::new);

        let xml_editor = cx.new(|cx| {
            let mut state = CodeEditorState::new(window, cx);
            state.set_highlighter_factory(highlighter_factory.clone(), cx);
            state.lsp_mut().completion_provider =
                Some(Rc::new(EnvCompletionProvider::new(env_state.clone())));
            attach_env_lsp(&mut state, env_state.clone(), modal_state.clone());
            state.language("xml").line_number(true).folding(true)
        });
        let xml_env_dec = xml_editor.update(cx, EnvEditorDecorations::new);

        let raw_textarea = cx.new(|cx| TextareaState::new(window, cx));

        let grpc_message_editor = cx.new(|cx| {
            let mut state = CodeEditorState::new(window, cx);
            state.set_highlighter_factory(highlighter_factory.clone(), cx);
            state.lsp_mut().completion_provider = Some(Rc::new(
                super::grpc_completion::ProtoJsonCompletionProvider::new(
                    env_state.clone(),
                    session_state.clone(),
                    workspace_state.clone(),
                ),
            ));
            attach_env_lsp(&mut state, env_state.clone(), modal_state.clone());
            state.language("json").line_number(true).folding(true)
        });
        let grpc_env_dec = grpc_message_editor.update(cx, EnvEditorDecorations::new);
        let grpc_error_marks = grpc_message_editor.update(cx, |state, cx| {
            state.create_gutter_decorations_collection(vec![], cx)
        });
        let grpc_diagnostics_hover = std::rc::Rc::new(std::cell::Cell::new(false));

        let ws_message_textarea = cx.new(|cx| TextareaState::new(window, cx));

        // The GraphQL editors need their own env handle: `env_state` is
        // moved into the script editors below.
        let env_state_for_graphql = env_state.clone();
        let graphql_query_editor = cx.new(|cx| {
            let mut state = CodeEditorState::new(window, cx);
            state.set_highlighter_factory(highlighter_factory.clone(), cx);
            state.lsp_mut().completion_provider = Some(Rc::new(
                super::graphql_completion::GraphQLQueryCompletionProvider::new(
                    env_state_for_graphql.clone(),
                    session_state.clone(),
                ),
            ));
            attach_env_lsp(
                &mut state,
                env_state_for_graphql.clone(),
                modal_state.clone(),
            );
            state.language("graphql").line_number(true).folding(true)
        });
        let graphql_query_env_dec = graphql_query_editor.update(cx, EnvEditorDecorations::new);
        let graphql_variables_editor = cx.new(|cx| {
            let mut state = CodeEditorState::new(window, cx);
            state.set_highlighter_factory(highlighter_factory.clone(), cx);
            state.lsp_mut().completion_provider = Some(Rc::new(EnvCompletionProvider::new(
                env_state_for_graphql.clone(),
            )));
            attach_env_lsp(&mut state, env_state_for_graphql, modal_state);
            state.language("json").line_number(true).folding(true)
        });
        let graphql_variables_env_dec =
            graphql_variables_editor.update(cx, EnvEditorDecorations::new);
        let graphql_variables_marks = graphql_variables_editor.update(cx, |state, cx| {
            state.create_gutter_decorations_collection(vec![], cx)
        });
        let graphql_diagnostics_hover = std::rc::Rc::new(std::cell::Cell::new(false));
        let graphql_operation_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Operation name (optional)"));

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
            auth: AuthComposerFields {
                bearer_token_input,
                basic_user_input,
                basic_pass_input,
                apikey_key_input,
                apikey_val_input,
            },
            grpc: GrpcComposerFields {
                message_editor: grpc_message_editor,
                proto_path_input: grpc_proto_path_input,
                error_marks: grpc_error_marks,
                diagnostics_hover: grpc_diagnostics_hover,
            },
            graphql: GraphQLComposerFields {
                query_editor: graphql_query_editor,
                variables_editor: graphql_variables_editor,
                operation_name_input: graphql_operation_name_input,
                variables_marks: graphql_variables_marks,
                diagnostics_hover: graphql_diagnostics_hover,
            },
            env_decorations: EnvEditorDecorationSet {
                json: json_env_dec,
                xml: xml_env_dec,
                grpc_message: grpc_env_dec,
                graphql_query: graphql_query_env_dec,
                graphql_variables: graphql_variables_env_dec,
            },
            ws_message_textarea,
            ws_message_owner: None,
        }
    }

    /// Recomputes environment variable decorations across all env-aware code editors.
    pub fn refresh_all_env_decorations(&self, env_state: &Entity<EnvironmentState>, cx: &mut App) {
        let json_val = self.json_editor.read(cx).value().to_string();
        let xml_val = self.xml_editor.read(cx).value().to_string();
        let grpc_val = self.grpc.message_editor.read(cx).value().to_string();
        let gql_query_val = self.graphql.query_editor.read(cx).value().to_string();
        let gql_vars_val = self.graphql.variables_editor.read(cx).value().to_string();

        self.env_decorations.json.sync(&json_val, env_state, cx);
        self.env_decorations.xml.sync(&xml_val, env_state, cx);
        self.env_decorations
            .grpc_message
            .sync(&grpc_val, env_state, cx);
        self.env_decorations
            .graphql_query
            .sync(&gql_query_val, env_state, cx);
        self.env_decorations
            .graphql_variables
            .sync(&gql_vars_val, env_state, cx);
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
                crate::env::build_tokenized_content(&req.url, env)
            };
            self.url_input.update(cx, |inp, cx| {
                inp.set_value(tokenized_url, window, cx);
            });

            let env_holder = env_state.clone();
            match &req.auth {
                Auth::Bearer { token } => {
                    let env_m = env_holder.clone();
                    self.auth.bearer_token_input.update(cx, |inp, cx| {
                        let env = env_m.read(cx);
                        inp.set_value(crate::env::build_tokenized_content(token, env), window, cx);
                    });
                }
                Auth::Basic { username, password } => {
                    let env_u = env_holder.clone();
                    let env_p = env_holder.clone();
                    self.auth.basic_user_input.update(cx, |inp, cx| {
                        let env = env_u.read(cx);
                        inp.set_value(
                            crate::env::build_tokenized_content(username, env),
                            window,
                            cx,
                        );
                    });
                    self.auth.basic_pass_input.update(cx, |inp, cx| {
                        let env = env_p.read(cx);
                        inp.set_value(
                            crate::env::build_tokenized_content(password, env),
                            window,
                            cx,
                        );
                    });
                }
                Auth::ApiKey { key, value, .. } => {
                    let env_k = env_holder.clone();
                    let env_v = env_holder.clone();
                    self.auth.apikey_key_input.update(cx, |inp, cx| {
                        let env = env_k.read(cx);
                        inp.set_value(crate::env::build_tokenized_content(key, env), window, cx);
                    });
                    self.auth.apikey_val_input.update(cx, |inp, cx| {
                        let env = env_v.read(cx);
                        inp.set_value(crate::env::build_tokenized_content(value, env), window, cx);
                    });
                }
                Auth::Inherit | Auth::None => {
                    self.auth.bearer_token_input.update(cx, |inp, cx| {
                        inp.set_value(String::new(), window, cx);
                    });
                    self.auth.basic_user_input.update(cx, |inp, cx| {
                        inp.set_value(String::new(), window, cx);
                    });
                    self.auth.basic_pass_input.update(cx, |inp, cx| {
                        inp.set_value(String::new(), window, cx);
                    });
                    self.auth.apikey_key_input.update(cx, |inp, cx| {
                        inp.set_value(String::new(), window, cx);
                    });
                    self.auth.apikey_val_input.update(cx, |inp, cx| {
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

        // GraphQL query/variables/operation name are persisted in the
        // request file; other protocols clear the editors.
        if req.protocol == Protocol::GraphQL {
            let payload = req.graphql.clone().unwrap_or_default();
            self.graphql.query_editor.update(cx, |txt, cx| {
                txt.set_value(payload.query, window, cx);
            });
            self.graphql.variables_editor.update(cx, |txt, cx| {
                txt.set_value(payload.variables, window, cx);
            });
            self.graphql.operation_name_input.update(cx, |inp, cx| {
                inp.set_value(payload.operation_name.unwrap_or_default(), window, cx);
            });
        } else {
            self.graphql.query_editor.update(cx, |txt, cx| {
                txt.set_value(String::new(), window, cx);
            });
            self.graphql.variables_editor.update(cx, |txt, cx| {
                txt.set_value(String::new(), window, cx);
            });
            self.graphql.operation_name_input.update(cx, |inp, cx| {
                inp.set_value(String::new(), window, cx);
            });
        }

        self.refresh_all_env_decorations(env_state, cx);
    }
}
