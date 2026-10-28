use super::composer_inputs::ComposerInputs;
use super::workspace_event_handler::{
    handle_environment_event, handle_response_event, handle_session_event, handle_workspace_event,
};
use super::AppView;
use crate::state::{
    EnvironmentEvent, EnvironmentState, GitState, ModalState, PersistenceState,
    RequestSessionState, ResponseEvent, ResponseState, SessionEvent, SidebarState, ToastState,
    WorkspaceDataState, WorkspaceEvent,
};
use gpui::*;
use gpui_component::input::InputEvent;
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
    #[allow(dead_code)]
    pub toast_state: &'a Entity<ToastState>,
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
    subscriptions.push(
        cx.subscribe(url_input, move |this, input, event, cx| match event {
            InputEvent::Change => {
                let val = input.read(cx).value().to_string();
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
                            this.state.toast.update(cx, |t, cx| {
                                t.push_success(
                                    "cURL Imported",
                                    format!("Parsed {} request to {}", method_name, target_url),
                                    cx,
                                );
                            });
                            return;
                        }
                        Err(e) => {
                            if val.lines().count() > 1 || val.contains(" -") || val.contains(" --")
                            {
                                this.state.toast.update(cx, |t, cx| {
                                    t.push_error(
                                        "cURL Parse Error",
                                        format!("Failed to parse cURL: {}", e),
                                        cx,
                                    );
                                });
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
        }),
    );

    // 2. Subscribe to JSON editor change
    let ed_json = editor_state.clone();
    let ws_json = workspace_state.clone();
    subscriptions.push(cx.subscribe(json_editor, move |_, txt, event, cx| {
        if let InputEvent::Change = event {
            let val = txt.read(cx).value().to_string();
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
    subscriptions.push(cx.subscribe(xml_editor, move |_, txt, event, cx| {
        if let InputEvent::Change = event {
            let val = txt.read(cx).value().to_string();
            let active_id = ed_xml.read(cx).active_request_id().to_string();
            ws_xml.update(cx, |ws, _| {
                ws.update_active_request(&active_id, |r| {
                    r.set_raw_body(val, "application/xml".to_string())
                });
            });
            ed_xml.update(cx, |ed, cx| ed.mark_dirty(cx));
        }
    }));

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
                    cx.refresh_windows();
                    cx.notify();
                }
            }
        });
    }));

    subscriptions
}
