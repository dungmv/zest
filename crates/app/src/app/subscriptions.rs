use super::workspace_event_handler::{
    handle_environment_event, handle_response_event, handle_session_event, handle_workspace_event,
};
use super::AppView;
use crate::components::request_composer::composer_inputs::ComposerInputs;
use crate::env::{attach_missing_var_tokens, RetokenizeGuard};
use crate::state::{
    EnvironmentEvent, EnvironmentState, GitState, ModalEvent, ModalState, PersistenceState,
    RequestSessionState, ResponseEvent, ResponseState, SessionEvent, SidebarState,
    WorkspaceDataState, WorkspaceEvent,
};
use gpui_kit::component::input::InputEvent;
use gpui_kit::*;
use std::cell::RefCell;
use std::rc::Rc;
use zest_core::models::{ApiKeyLocation, Auth, RequestBody};

pub struct SubscriptionTargets<'a> {
    pub workspace_state: &'a Entity<WorkspaceDataState>,
    pub env_state: &'a Entity<EnvironmentState>,
    pub persistence_state: &'a Entity<PersistenceState>,
    pub modal_state: &'a Entity<ModalState>,
    pub sidebar_state: &'a Entity<SidebarState>,
    pub git_state: &'a Entity<GitState>,
    pub editor_state: &'a Entity<RequestSessionState>,
    pub response_state: &'a Entity<ResponseState>,
    pub composer_inputs: &'a ComposerInputs,
}

pub fn setup_app_subscriptions(
    targets: SubscriptionTargets,
    window: &mut Window,
    cx: &mut Context<AppView>,
) -> Vec<Subscription> {
    let mut subscriptions = Vec::new();

    let editor_state = targets.editor_state;
    let workspace_state = targets.workspace_state;
    let url_input = &targets.composer_inputs.url_input;
    let json_editor = &targets.composer_inputs.json_editor;
    let xml_editor = &targets.composer_inputs.xml_editor;
    let raw_textarea = &targets.composer_inputs.raw_textarea;
    let grpc_message_editor = &targets.composer_inputs.grpc.message_editor;
    let grpc_proto_path_input = &targets.composer_inputs.grpc.proto_path_input;
    let bearer_token_input = &targets.composer_inputs.bearer_token_input;
    let basic_user_input = &targets.composer_inputs.basic_user_input;
    let basic_pass_input = &targets.composer_inputs.basic_pass_input;
    let apikey_key_input = &targets.composer_inputs.apikey_key_input;
    let apikey_val_input = &targets.composer_inputs.apikey_val_input;
    let pre_script_editor = &targets.composer_inputs.pre_script_editor;
    let test_script_editor = &targets.composer_inputs.test_script_editor;
    let response_state = targets.response_state;

    // 1. Subscribe to URL input change
    let ed_url = editor_state.clone();
    let ws_url = workspace_state.clone();
    // Retokenizing a span re-emits `Change` with an identical value, and
    // undoing a conversion restores token-less text with an identical value.
    // The guard recognizes both so undo is never cancelled by an immediate
    // re-attach (see RetokenizeGuard).
    let url_retoken_guard = Rc::new(RefCell::new(RetokenizeGuard::default()));
    subscriptions.push(cx.subscribe_in(
        url_input,
        window,
        move |this, input, event, window, cx| match event {
            InputEvent::Change => {
                let val = input.read(cx).value().to_string();
                let is_new_value = url_retoken_guard.borrow_mut().should_process(&val);
                if !is_new_value {
                    return;
                }
                // A closing `}}` typed or pasted by the user completes a raw
                // `{{var}}` span; convert it into an inline variable tag.
                input.update(cx, |inp, cx| {
                    attach_missing_var_tokens(inp, window, cx);
                });
                let active_id = ed_url.read(cx).active_request_id().to_string();
                if zest_core::curl::is_curl_command(&val) {
                    let ws_id = ws_url
                        .read(cx)
                        .find_request(&active_id)
                        .map(|r| r.workspace_id.clone())
                        .unwrap_or_else(|| "scratch".to_string());
                    match zest_core::curl::curl_to_request(&ws_id, &val) {
                        Ok(parsed) => {
                            let method_name = parsed.method.as_str().to_string();
                            let target_url = parsed.url.clone();
                            ws_url.update(cx, |ws, _| {
                                ws.update_active_request(&active_id, |r| {
                                    r.apply_imported_curl(parsed);
                                });
                            });
                            ed_url.update(cx, |ed, cx| {
                                ed.notify_request_reloaded(cx);
                            });
                            crate::notify::success_in(
                                "cURL Imported",
                                format!("Parsed {} request to {}", method_name, target_url),
                                window,
                                cx,
                            );
                            return;
                        }
                        Err(e) => {
                            if val.lines().count() > 1 || val.contains(" -") || val.contains(" --")
                            {
                                crate::notify::error_in(
                                    "cURL Parse Error",
                                    format!("Failed to parse cURL: {}", e),
                                    window,
                                    cx,
                                );
                            }
                        }
                    }
                }
                let mut needs_refresh = false;
                ws_url.update(cx, |ws, _| {
                    ws.update_active_request(&active_id, |r| {
                        let had_template = r.url.contains("{{");
                        let (has_query, has_template) = r.apply_url(&val);
                        needs_refresh = has_query || had_template || has_template;
                    });
                });
                ed_url.update(cx, |ed, cx| {
                    ed.mark_dirty(cx);
                    if needs_refresh {
                        ed.schedule_input_refresh(cx);
                    }
                });
            }
            InputEvent::PressEnter { .. } => {
                this.send_current_request(cx);
            }
            _ => {}
        },
    ));

    // 2. Subscribe to JSON editor change
    let ed_json = editor_state.clone();
    let ws_json = workspace_state.clone();
    let env_json = targets.env_state.clone();
    let dec_json = targets.composer_inputs.env_decorations.json.clone();
    subscriptions.push(cx.subscribe(json_editor, move |_, txt, event, cx| {
        if let InputEvent::Change = event {
            let val = txt.read(cx).value().to_string();
            dec_json.sync(&val, &env_json, cx);
            let active_id = ed_json.read(cx).active_request_id().to_string();
            ws_json.update(cx, |ws, _| {
                ws.update_active_request(&active_id, |r| r.set_body_json(val));
            });
            ed_json.update(cx, |ed, cx| ed.mark_dirty(cx));
        }
    }));

    // 2b. Subscribe to XML editor change
    let ed_xml = editor_state.clone();
    let ws_xml = workspace_state.clone();
    let env_xml = targets.env_state.clone();
    let dec_xml = targets.composer_inputs.env_decorations.xml.clone();
    subscriptions.push(cx.subscribe(xml_editor, move |_, txt, event, cx| {
        if let InputEvent::Change = event {
            let val = txt.read(cx).value().to_string();
            dec_xml.sync(&val, &env_xml, cx);
            let active_id = ed_xml.read(cx).active_request_id().to_string();
            ws_xml.update(cx, |ws, _| {
                ws.update_active_request(&active_id, |r| {
                    r.set_raw_body(val, "application/xml".to_string())
                });
            });
            ed_xml.update(cx, |ed, cx| ed.mark_dirty(cx));
        }
    }));

    // 2b-2. Subscribe to gRPC .proto path input (persisted into GrpcPayload)
    let ed_gpath = editor_state.clone();
    let ws_gpath = workspace_state.clone();
    subscriptions.push(
        cx.subscribe(grpc_proto_path_input, move |_, inp, event, cx| {
            if let InputEvent::Change = event {
                let val = inp.read(cx).value().to_string();
                let active_id = ed_gpath.read(cx).active_request_id().to_string();
                ws_gpath.update(cx, |ws, _| {
                    ws.update_active_request(&active_id, |r| {
                        let path = if val.trim().is_empty() {
                            None
                        } else {
                            Some(val.clone())
                        };
                        r.grpc_mut().proto_path = path;
                    });
                });
                ed_gpath.update(cx, |ed, cx| ed.mark_dirty(cx));
            }
        }),
    );

    // 2c. Subscribe to gRPC message editor change (persisted into GrpcPayload)
    let ed_grpc = editor_state.clone();
    let ws_grpc = workspace_state.clone();
    let env_grpc = targets.env_state.clone();
    let dec_grpc = targets.composer_inputs.env_decorations.grpc_message.clone();
    let grpc_marks = targets.composer_inputs.grpc.error_marks.clone();
    let grpc_hover = targets.composer_inputs.grpc.diagnostics_hover.clone();
    subscriptions.push(cx.subscribe(grpc_message_editor, move |_, txt, event, cx| {
        if let InputEvent::Change = event {
            let val = txt.read(cx).value().to_string();
            dec_grpc.sync(&val, &env_grpc, cx);
            let active_id = ed_grpc.read(cx).active_request_id().to_string();
            ws_grpc.update(cx, |ws, _| {
                ws.update_active_request(&active_id, |r| r.set_grpc_message_json(val.clone()));
            });
            // Validate and surface the error on the offending line: editor
            // diagnostic (hover tooltip) + red gutter mark.
            crate::components::request_composer::refresh_grpc_validation(
                &txt,
                &grpc_marks,
                grpc_hover.get(),
                &val,
                &ed_grpc,
                &ws_grpc,
                cx,
            );
            ed_grpc.update(cx, |ed, cx| ed.mark_dirty(cx));
        }
    }));

    // 2d. Subscribe to GraphQL query editor change (persisted into GraphQLPayload)
    let ed_gql = editor_state.clone();
    let ws_gql = workspace_state.clone();
    let env_gql = targets.env_state.clone();
    let dec_gql = targets
        .composer_inputs
        .env_decorations
        .graphql_query
        .clone();
    subscriptions.push(cx.subscribe(
        &targets.composer_inputs.graphql.query_editor,
        move |_, txt, event, cx| {
            if let InputEvent::Change = event {
                let val = txt.read(cx).value().to_string();
                dec_gql.sync(&val, &env_gql, cx);
                let active_id = ed_gql.read(cx).active_request_id().to_string();
                ws_gql.update(cx, |ws, _| {
                    ws.update_active_request(&active_id, |r| {
                        r.graphql_mut().query = val;
                    });
                });
                ed_gql.update(cx, |ed, cx| ed.mark_dirty(cx));
            }
        },
    ));

    // 2e. Subscribe to GraphQL variables editor change (+ JSON validation)
    let ed_gvar = editor_state.clone();
    let ws_gvar = workspace_state.clone();
    let env_gvar = targets.env_state.clone();
    let dec_gvar = targets
        .composer_inputs
        .env_decorations
        .graphql_variables
        .clone();
    let gvar_marks = targets.composer_inputs.graphql.variables_marks.clone();
    let gvar_hover = targets.composer_inputs.graphql.diagnostics_hover.clone();
    subscriptions.push(cx.subscribe(
        &targets.composer_inputs.graphql.variables_editor,
        move |_, txt, event, cx| {
            if let InputEvent::Change = event {
                let val = txt.read(cx).value().to_string();
                dec_gvar.sync(&val, &env_gvar, cx);
                let active_id = ed_gvar.read(cx).active_request_id().to_string();
                ws_gvar.update(cx, |ws, _| {
                    ws.update_active_request(&active_id, |r| {
                        r.graphql_mut().variables = val.clone();
                    });
                });
                crate::components::request_composer::refresh_graphql_variables_validation(
                    &txt,
                    &gvar_marks,
                    gvar_hover.get(),
                    &val,
                    cx,
                );
                ed_gvar.update(cx, |ed, cx| ed.mark_dirty(cx));
            }
        },
    ));

    // 2f. Subscribe to GraphQL operation name input (persisted)
    let ed_gop = editor_state.clone();
    let ws_gop = workspace_state.clone();
    subscriptions.push(cx.subscribe(
        &targets.composer_inputs.graphql.operation_name_input,
        move |_, inp, event, cx| {
            if let InputEvent::Change = event {
                let val = inp.read(cx).value().to_string();
                let active_id = ed_gop.read(cx).active_request_id().to_string();
                ws_gop.update(cx, |ws, _| {
                    ws.update_active_request(&active_id, |r| {
                        let name = if val.trim().is_empty() {
                            None
                        } else {
                            Some(val.clone())
                        };
                        r.graphql_mut().operation_name = name;
                    });
                });
                ed_gop.update(cx, |ed, cx| ed.mark_dirty(cx));
            }
        },
    ));

    // 3. Subscribe to Raw textarea change
    let ed_raw = editor_state.clone();
    let ws_raw = workspace_state.clone();
    subscriptions.push(cx.subscribe(raw_textarea, move |_, txt, event, cx| {
        if let InputEvent::Change = event {
            let val = txt.read(cx).value().to_string();
            let active_id = ed_raw.read(cx).active_request_id().to_string();
            ws_raw.update(cx, |ws, _| {
                ws.update_active_request(&active_id, |r| {
                    let content_type = match &r.body {
                        RequestBody::Raw { content_type, .. } => content_type.clone(),
                        _ => "text/plain".to_string(),
                    };
                    r.set_raw_body(val, content_type);
                });
            });
            ed_raw.update(cx, |ed, cx| ed.mark_dirty(cx));
        }
    }));

    // 3b. Subscribe to Auth inputs
    let ed_bearer = editor_state.clone();
    let ws_bearer = workspace_state.clone();
    subscriptions.push(cx.subscribe(bearer_token_input, move |_, inp, event, cx| {
        if let InputEvent::Change = event {
            let val = inp.read(cx).value().to_string();
            let active_id = ed_bearer.read(cx).active_request_id().to_string();
            let mut needs_refresh = false;
            ws_bearer.update(cx, |ws, _| {
                ws.update_active_request(&active_id, |r| {
                    let had_template = match &r.auth {
                        Auth::Bearer { token } => token.contains("{{"),
                        _ => false,
                    };
                    needs_refresh = had_template || val.contains("{{");
                    r.set_bearer_token(val);
                });
            });
            ed_bearer.update(cx, |ed, cx| {
                ed.mark_dirty(cx);
                if needs_refresh {
                    ed.schedule_input_refresh(cx);
                }
            });
        }
    }));

    let ed_basic_u = editor_state.clone();
    let ws_basic_u = workspace_state.clone();
    let pass_ref = basic_pass_input.clone();
    subscriptions.push(cx.subscribe(basic_user_input, move |_, inp, event, cx| {
        if let InputEvent::Change = event {
            let u = inp.read(cx).value().to_string();
            let p = pass_ref.read(cx).value().to_string();
            let active_id = ed_basic_u.read(cx).active_request_id().to_string();
            let mut needs_refresh = false;
            ws_basic_u.update(cx, |ws, _| {
                ws.update_active_request(&active_id, |r| {
                    let had_template = match &r.auth {
                        Auth::Basic { username, password } => {
                            username.contains("{{") || password.contains("{{")
                        }
                        _ => false,
                    };
                    needs_refresh = had_template || u.contains("{{") || p.contains("{{");
                    r.set_basic_auth(u, p);
                });
            });
            ed_basic_u.update(cx, |ed, cx| {
                ed.mark_dirty(cx);
                if needs_refresh {
                    ed.schedule_input_refresh(cx);
                }
            });
        }
    }));

    let ed_basic_p = editor_state.clone();
    let ws_basic_p = workspace_state.clone();
    let user_ref = basic_user_input.clone();
    subscriptions.push(cx.subscribe(basic_pass_input, move |_, inp, event, cx| {
        if let InputEvent::Change = event {
            let u = user_ref.read(cx).value().to_string();
            let p = inp.read(cx).value().to_string();
            let active_id = ed_basic_p.read(cx).active_request_id().to_string();
            let mut needs_refresh = false;
            ws_basic_p.update(cx, |ws, _| {
                ws.update_active_request(&active_id, |r| {
                    let had_template = match &r.auth {
                        Auth::Basic { username, password } => {
                            username.contains("{{") || password.contains("{{")
                        }
                        _ => false,
                    };
                    needs_refresh = had_template || u.contains("{{") || p.contains("{{");
                    r.set_basic_auth(u, p);
                });
            });
            ed_basic_p.update(cx, |ed, cx| {
                ed.mark_dirty(cx);
                if needs_refresh {
                    ed.schedule_input_refresh(cx);
                }
            });
        }
    }));

    let ed_ak_k = editor_state.clone();
    let ws_ak_k = workspace_state.clone();
    let ak_val_ref = apikey_val_input.clone();
    subscriptions.push(cx.subscribe(apikey_key_input, move |_, inp, event, cx| {
        if let InputEvent::Change = event {
            let k = inp.read(cx).value().to_string();
            let v = ak_val_ref.read(cx).value().to_string();
            let active_id = ed_ak_k.read(cx).active_request_id().to_string();
            let mut needs_refresh = false;
            ws_ak_k.update(cx, |ws, _| {
                ws.update_active_request(&active_id, |r| {
                    let (had_template, add_to) = match &r.auth {
                        Auth::ApiKey { key, value, add_to } => {
                            (key.contains("{{") || value.contains("{{"), *add_to)
                        }
                        _ => (false, ApiKeyLocation::Header),
                    };
                    needs_refresh = had_template || k.contains("{{") || v.contains("{{");
                    r.set_api_key(k, v, add_to);
                });
            });
            ed_ak_k.update(cx, |ed, cx| {
                ed.mark_dirty(cx);
                if needs_refresh {
                    ed.schedule_input_refresh(cx);
                }
            });
        }
    }));

    let ed_ak_v = editor_state.clone();
    let ws_ak_v = workspace_state.clone();
    let ak_key_ref = apikey_key_input.clone();
    subscriptions.push(cx.subscribe(apikey_val_input, move |_, inp, event, cx| {
        if let InputEvent::Change = event {
            let k = ak_key_ref.read(cx).value().to_string();
            let v = inp.read(cx).value().to_string();
            let active_id = ed_ak_v.read(cx).active_request_id().to_string();
            let mut needs_refresh = false;
            ws_ak_v.update(cx, |ws, _| {
                ws.update_active_request(&active_id, |r| {
                    let (had_template, add_to) = match &r.auth {
                        Auth::ApiKey { key, value, add_to } => {
                            (key.contains("{{") || value.contains("{{"), *add_to)
                        }
                        _ => (false, ApiKeyLocation::Header),
                    };
                    needs_refresh = had_template || k.contains("{{") || v.contains("{{");
                    r.set_api_key(k, v, add_to);
                });
            });
            ed_ak_v.update(cx, |ed, cx| {
                ed.mark_dirty(cx);
                if needs_refresh {
                    ed.schedule_input_refresh(cx);
                }
            });
        }
    }));

    // 3c. Subscribe to Script editors
    let ed_pre = editor_state.clone();
    let ws_pre = workspace_state.clone();
    subscriptions.push(cx.subscribe(pre_script_editor, move |_, txt, event, cx| {
        if let InputEvent::Change = event {
            let val = txt.read(cx).value().to_string();
            let active_id = ed_pre.read(cx).active_request_id().to_string();
            ws_pre.update(cx, |ws, _| {
                ws.update_active_request(&active_id, |r| r.set_pre_script(val));
            });
            ed_pre.update(cx, |ed, cx| ed.mark_dirty(cx));
        }
    }));

    let ed_test = editor_state.clone();
    let ws_test = workspace_state.clone();
    subscriptions.push(cx.subscribe(test_script_editor, move |_, txt, event, cx| {
        if let InputEvent::Change = event {
            let val = txt.read(cx).value().to_string();
            let active_id = ed_test.read(cx).active_request_id().to_string();
            ws_test.update(cx, |ws, _| {
                ws.update_active_request(&active_id, |r| r.set_test_script(val));
            });
            ed_test.update(cx, |ed, cx| ed.mark_dirty(cx));
        }
    }));

    // 4. Subscribe to typed state events (synchronous, single-dispatch command channel)
    subscriptions.push(cx.subscribe_in(
        editor_state,
        window,
        |this, _ed, event: &SessionEvent, window, cx| {
            handle_session_event(this, event, window, cx);
        },
    ));
    subscriptions.push(cx.subscribe_in(
        workspace_state,
        window,
        |this, _ws, event: &WorkspaceEvent, window, cx| {
            handle_workspace_event(this, event, window, cx);
        },
    ));
    subscriptions.push(cx.subscribe_in(
        response_state,
        window,
        |this, _resp, event: &ResponseEvent, window, cx| {
            handle_response_event(this, event, window, cx);
        },
    ));
    subscriptions.push(cx.subscribe(
        targets.env_state,
        |this, _env, event: &EnvironmentEvent, cx| {
            handle_environment_event(this, event, cx);
        },
    ));
    subscriptions.push(cx.subscribe(
        targets.modal_state,
        |this, _modal, event: &ModalEvent, cx| {
            if matches!(event, ModalEvent::ThemeChanged) {
                this.composer_inputs
                    .refresh_all_env_decorations(&this.state.env, cx);
            }
        },
    ));

    // 5. Pure redraw observers: re-render AppView on entity state changes
    subscriptions.push(cx.observe(editor_state, |_, _, cx| cx.notify()));
    subscriptions.push(cx.observe(workspace_state, |_, _, cx| cx.notify()));
    subscriptions.push(cx.observe(response_state, |_, _, cx| cx.notify()));
    subscriptions.push(cx.observe(targets.sidebar_state, |_, _, cx| cx.notify()));
    subscriptions.push(cx.observe(targets.modal_state, |_, _, cx| cx.notify()));
    subscriptions.push(cx.observe(targets.git_state, |_, _, cx| cx.notify()));
    subscriptions.push(cx.observe(targets.env_state, |_, _, cx| cx.notify()));
    subscriptions.push(cx.observe(targets.persistence_state, |_, _, cx| cx.notify()));

    // 6. Observe OS appearance changes for System theme preference
    let modal_theme = targets.modal_state.clone();
    subscriptions.push(window.observe_window_appearance(move |window, cx| {
        let app_appearance = window.appearance();
        modal_theme.update(cx, |modal, cx| {
            if modal.theme_preference == crate::theme::ThemePreference::System {
                let mode = crate::theme::resolve_theme_mode(
                    crate::theme::ThemePreference::System,
                    app_appearance,
                );
                if crate::theme::current_theme_mode() != mode {
                    crate::theme::set_theme(crate::theme::ThemePreference::System, mode);
                    crate::theme::sync_component_theme(mode, cx);
                    cx.emit(ModalEvent::ThemeChanged);
                    cx.refresh_windows();
                    cx.notify();
                }
            }
        });
    }));

    subscriptions
}
