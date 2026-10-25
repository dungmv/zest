use super::composer_inputs::ComposerInputs;
use super::sync::save_active_request_without_redraw;
use super::AppView;
use crate::state::{
    EnvironmentState, GitState, ModalState, PersistenceState, RequestSessionState, ResponseState,
    SidebarState, ToastState, WorkspaceDataState,
};
use gpui::*;
use gpui_component::input::InputEvent;
use zest_core::models::{Auth, HttpRequestItem, RequestBody};

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
    initial_req: Option<&HttpRequestItem>,
    window: &mut Window,
    cx: &mut Context<AppView>,
) -> Vec<Subscription> {
    let mut subscriptions = Vec::new();

    let editor_state = targets.editor_state;
    let workspace_state = targets.workspace_state;
    let persistence_state = targets.persistence_state;
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
    let per_url = persistence_state.clone();
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
                                ed.bump_request_version(cx);
                                ed.bump_body_version(cx);
                            });
                            save_active_request_without_redraw(&ed_url, &per_url, &ws_url, cx);
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
                    ed.is_dirty = true;
                    if needs_refresh {
                        ed.schedule_input_refresh(cx);
                    }
                });
                save_active_request_without_redraw(&ed_url, &per_url, &ws_url, cx);
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
    let per_json = persistence_state.clone();
    subscriptions.push(cx.subscribe(json_editor, move |_, txt, event, cx| {
        if let InputEvent::Change = event {
            let val = txt.read(cx).value().to_string();
            let active_id = ed_json.read(cx).active_request_id().to_string();
            ws_json.update(cx, |ws, _| {
                ws.update_active_request(&active_id, |r| r.set_body_json(val));
            });
            ed_json.update(cx, |ed, cx| ed.mark_dirty(cx));
            save_active_request_without_redraw(&ed_json, &per_json, &ws_json, cx);
        }
    }));

    // 2b. Subscribe to XML editor change
    let ed_xml = editor_state.clone();
    let ws_xml = workspace_state.clone();
    let per_xml = persistence_state.clone();
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
            save_active_request_without_redraw(&ed_xml, &per_xml, &ws_xml, cx);
        }
    }));

    // 3. Subscribe to Raw textarea change
    let ed_raw = editor_state.clone();
    let ws_raw = workspace_state.clone();
    let per_raw = persistence_state.clone();
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
            save_active_request_without_redraw(&ed_raw, &per_raw, &ws_raw, cx);
        }
    }));

    // 3b. Subscribe to Auth inputs
    let ed_bearer = editor_state.clone();
    let ws_bearer = workspace_state.clone();
    let per_bearer = persistence_state.clone();
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
                ed.is_dirty = true;
                if needs_refresh {
                    ed.schedule_input_refresh(cx);
                }
            });
            save_active_request_without_redraw(&ed_bearer, &per_bearer, &ws_bearer, cx);
        }
    }));

    let ed_basic_u = editor_state.clone();
    let ws_basic_u = workspace_state.clone();
    let per_basic_u = persistence_state.clone();
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
                ed.is_dirty = true;
                if needs_refresh {
                    ed.schedule_input_refresh(cx);
                }
            });
            save_active_request_without_redraw(&ed_basic_u, &per_basic_u, &ws_basic_u, cx);
        }
    }));

    let ed_basic_p = editor_state.clone();
    let ws_basic_p = workspace_state.clone();
    let per_basic_p = persistence_state.clone();
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
                ed.is_dirty = true;
                if needs_refresh {
                    ed.schedule_input_refresh(cx);
                }
            });
            save_active_request_without_redraw(&ed_basic_p, &per_basic_p, &ws_basic_p, cx);
        }
    }));

    let ed_ak_k = editor_state.clone();
    let ws_ak_k = workspace_state.clone();
    let per_ak_k = persistence_state.clone();
    let ak_val_ref = apikey_val_input.clone();
    subscriptions.push(cx.subscribe(apikey_key_input, move |_, inp, event, cx| {
        if let InputEvent::Change = event {
            let k = inp.read(cx).value().to_string();
            let v = ak_val_ref.read(cx).value().to_string();
            let active_id = ed_ak_k.read(cx).active_request_id().to_string();
            let mut needs_refresh = false;
            ws_ak_k.update(cx, |ws, _| {
                ws.update_active_request(&active_id, |r| {
                    let (had_template, in_header) = match &r.auth {
                        Auth::ApiKey {
                            key,
                            value,
                            in_header,
                        } => (key.contains("{{") || value.contains("{{"), *in_header),
                        _ => (false, true),
                    };
                    needs_refresh = had_template || k.contains("{{") || v.contains("{{");
                    r.set_api_key(k, v, in_header);
                });
            });
            ed_ak_k.update(cx, |ed, cx| {
                ed.is_dirty = true;
                if needs_refresh {
                    ed.schedule_input_refresh(cx);
                }
            });
            save_active_request_without_redraw(&ed_ak_k, &per_ak_k, &ws_ak_k, cx);
        }
    }));

    let ed_ak_v = editor_state.clone();
    let ws_ak_v = workspace_state.clone();
    let per_ak_v = persistence_state.clone();
    let ak_key_ref = apikey_key_input.clone();
    subscriptions.push(cx.subscribe(apikey_val_input, move |_, inp, event, cx| {
        if let InputEvent::Change = event {
            let k = ak_key_ref.read(cx).value().to_string();
            let v = inp.read(cx).value().to_string();
            let active_id = ed_ak_v.read(cx).active_request_id().to_string();
            let mut needs_refresh = false;
            ws_ak_v.update(cx, |ws, _| {
                ws.update_active_request(&active_id, |r| {
                    let (had_template, in_header) = match &r.auth {
                        Auth::ApiKey {
                            key,
                            value,
                            in_header,
                        } => (key.contains("{{") || value.contains("{{"), *in_header),
                        _ => (false, true),
                    };
                    needs_refresh = had_template || k.contains("{{") || v.contains("{{");
                    r.set_api_key(k, v, in_header);
                });
            });
            ed_ak_v.update(cx, |ed, cx| {
                ed.is_dirty = true;
                if needs_refresh {
                    ed.schedule_input_refresh(cx);
                }
            });
            save_active_request_without_redraw(&ed_ak_v, &per_ak_v, &ws_ak_v, cx);
        }
    }));

    // 3c. Subscribe to Script editors
    let ed_pre = editor_state.clone();
    let ws_pre = workspace_state.clone();
    let per_pre = persistence_state.clone();
    subscriptions.push(cx.subscribe(pre_script_editor, move |_, txt, event, cx| {
        if let InputEvent::Change = event {
            let val = txt.read(cx).value().to_string();
            let active_id = ed_pre.read(cx).active_request_id().to_string();
            ws_pre.update(cx, |ws, _| {
                ws.update_active_request(&active_id, |r| r.set_pre_script(val));
            });
            ed_pre.update(cx, |ed, cx| ed.mark_dirty(cx));
            save_active_request_without_redraw(&ed_pre, &per_pre, &ws_pre, cx);
        }
    }));

    let ed_test = editor_state.clone();
    let ws_test = workspace_state.clone();
    let per_test = persistence_state.clone();
    subscriptions.push(cx.subscribe(test_script_editor, move |_, txt, event, cx| {
        if let InputEvent::Change = event {
            let val = txt.read(cx).value().to_string();
            let active_id = ed_test.read(cx).active_request_id().to_string();
            ws_test.update(cx, |ws, _| {
                ws.update_active_request(&active_id, |r| r.set_test_script(val));
            });
            ed_test.update(cx, |ed, cx| ed.mark_dirty(cx));
            save_active_request_without_redraw(&ed_test, &per_test, &ws_test, cx);
        }
    }));

    // 4. Observe editor_state: sync inputs, reload response history & auto-save
    let ws_clone = workspace_state.clone();
    let per_clone = persistence_state.clone();
    let resp_clone = response_state.clone();
    let mut last_req_id = initial_req.map(|r| r.id.clone());
    subscriptions.push(
        cx.observe_in(editor_state, window, move |this, ed, window, cx| {
            this.composer_inputs.sync_active_request_editors(
                &ed,
                &this.state.workspace,
                &this.state.env,
                window,
                cx,
            );

            let (active_id, is_dirty) = {
                let ed_ref = ed.read(cx);
                (ed_ref.active_request_id().to_string(), ed_ref.is_dirty)
            };
            let is_scratch = active_id == "scratch" || active_id.is_empty();
            if last_req_id.as_deref() != Some(&active_id) {
                last_req_id = Some(active_id.clone());
                let ws_store = ws_clone.read(cx).store.clone();
                resp_clone.update(cx, |r_state, cx| {
                    r_state.load_for_request(&active_id, ws_store.as_ref(), cx);
                });
            } else if !is_scratch && is_dirty {
                // Debounced auto-save active request to workspace storage
                let ed_ent = ed.clone();
                let ws_ent = ws_clone.clone();
                per_clone.update(cx, |per, cx| {
                    per.schedule_save_active_request(active_id, ed_ent, ws_ent, cx);
                });
            }
        }),
    );

    // 5. Observe workspace_state: synchronize workspace changes
    let ed_sync = editor_state.clone();
    let resp_sync = response_state.clone();
    let per_sync = persistence_state.clone();
    let git_sync = targets.git_state.clone();
    let mut last_workspace_path = workspace_state.read(cx).workspace_path.clone();
    subscriptions.push(
        cx.observe_in(workspace_state, window, move |this, ws, window, cx| {
            this.composer_inputs.sync_active_request_editors(
                &this.state.session,
                &ws,
                &this.state.env,
                window,
                cx,
            );

            let current_ws_path = ws.read(cx).workspace_path.clone();
            if current_ws_path != last_workspace_path {
                last_workspace_path = current_ws_path.clone();
                git_sync.update(cx, |git, cx| {
                    git.set_repo_path(current_ws_path, cx);
                });
                let (next_id, ws_store) = {
                    let ws_ref = ws.read(cx);
                    let per_ref = per_sync.read(cx);
                    let id = per_ref
                        .local_state
                        .active_request
                        .as_ref()
                        .and_then(|act_id| {
                            ws_ref
                                .requests
                                .iter()
                                .find(|r| &r.id == act_id)
                                .map(|r| r.id.clone())
                        })
                        .or_else(|| ws_ref.requests.first().map(|r| r.id.clone()))
                        .unwrap_or_else(|| "scratch".to_string());
                    (id, ws_ref.store.clone())
                };
                ed_sync.update(cx, |ed, cx| {
                    ed.set_active_request_id(&next_id, cx);
                });
                resp_sync.update(cx, |resp, cx| {
                    resp.load_for_request(&next_id, ws_store.as_ref(), cx);
                });
                return;
            }

            let last_renamed = ws.read(cx).last_renamed_request.clone();
            if let Some((old_id, new_id)) = last_renamed {
                resp_sync.update(cx, |resp, _cx| {
                    resp.rename_request(&old_id, &new_id);
                });
                ed_sync.update(cx, |ed, cx| {
                    if ed.active_request_id.as_deref() == Some(&old_id) {
                        ed.active_request_id = Some(new_id);
                        cx.notify();
                    }
                });
            }

            if ws.read(cx).last_renamed_example.is_some() {
                let last_renamed_ex = ws.update(cx, |ws, _cx| ws.last_renamed_example.take());
                if let Some((_req_id, old_ex, new_ex)) = last_renamed_ex {
                    resp_sync.update(cx, |resp, cx| {
                        resp.rename_example(&old_ex, &new_ex, cx);
                    });
                }
            }
        }),
    );

    // 6. Observe response_state: synchronize displays & re-render on response/example changes
    subscriptions.push(
        cx.observe_in(response_state, window, move |this, resp, window, cx| {
            this.inspector_displays
                .sync_response_editor(&resp, window, cx);
            this.inspector_displays
                .sync_sent_request_preview(&resp, window, cx);
            cx.notify();
        }),
    );

    // 7. Observe sidebar_state, modal_state, git_state, env_state, persistence_state: re-render AppView on UI changes
    subscriptions.push(cx.observe(targets.sidebar_state, |_, _, cx| {
        cx.notify();
    }));
    subscriptions.push(cx.observe(targets.modal_state, |_, _, cx| {
        cx.notify();
    }));
    subscriptions.push(cx.observe(targets.git_state, |_, _, cx| {
        cx.notify();
    }));
    subscriptions.push(cx.observe(targets.env_state, |_, _, cx| {
        cx.notify();
    }));
    subscriptions.push(cx.observe(targets.persistence_state, |_, _, cx| {
        cx.notify();
    }));

    // 8. Observe OS appearance changes for System theme preference
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
