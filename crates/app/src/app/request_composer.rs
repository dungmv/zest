use super::auth_tab::{render_auth_tab, AuthTabContext, AuthTabInputs};
use super::AppView;
use crate::components::*;
use crate::state::*;
use crate::theme::theme;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::Sizable as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{div, px, AnyElement, Context, IntoElement, ParentElement, Styled, Window};
use zest_core::models::Auth;

impl AppView {
    pub(crate) fn render_request_composer(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = theme();
        let active_tab_kind = self.state.session.read(cx).active_tab_kind;
        let req_id_str = self.state.session.read(cx).active_request_id().to_string();
        let ws_entity = self.state.workspace.clone();
        let ed_entity = self.state.session.clone();
        let per_entity = self.state.persistence.clone();
        let env_entity = self.state.env.clone();
        let app_view_entity = cx.entity().clone();

        let ws_read = ws_entity.read(cx);
        let per_read = per_entity.read(cx);
        let env_read = env_entity.read(cx);

        let active_req = ws_read.find_request(&req_id_str);
        let active_req_info = active_req.map(|r| {
            (
                r.params.len(),
                r.headers.len(),
                r.script
                    .as_ref()
                    .and_then(|s| s.pre_request.as_ref())
                    .map(|s| !s.trim().is_empty())
                    .unwrap_or(false),
                r.script
                    .as_ref()
                    .and_then(|s| s.post_response.as_ref())
                    .map(|s| !s.trim().is_empty())
                    .unwrap_or(false),
            )
        });

        let active_conflict = per_read.conflicts.get(&req_id_str).cloned();
        let active_parse_err = ws_read.parse_errors.get(&req_id_str).cloned();
        let corrupt_sec_err = env_read.corrupt_secrets_warning.clone();

        div()
            .size_full()
            .bg(t.bg_editor)
            .relative()
            .flex()
            .flex_col()
            // Conflict banner
            .when_some(active_conflict, |this, msg| {
                let per_rel = per_entity.clone();
                let per_keep = per_entity.clone();
                let ws_rel = ws_entity.clone();
                let ws_keep = ws_entity.clone();
                let cid = req_id_str.clone();
                let cid_k = req_id_str.clone();
                this.child(
                    div()
                        .px(px(12.))
                        .py(px(6.))
                        .bg(gpui_kit::Hsla {
                            a: 0.15,
                            ..t.status_4xx
                        })
                        .border_b_1()
                        .border_color(t.status_4xx)
                        .flex()
                        .items_center()
                        .justify_between()
                        .text_xs()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .font_weight(gpui_kit::FontWeight::BOLD)
                                        .text_color(t.status_4xx)
                                        .child("! Conflict:"),
                                )
                                .child(div().text_color(t.text_primary).child(msg)),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(
                                    Button::new("btn-conflict-reload")
                                        .primary()
                                        .small()
                                        .label("Reload")
                                        .on_click(move |_, _, cx| {
                                            per_rel.update(cx, |per, cx| {
                                                per.resolve_conflict_reload(
                                                    &cid,
                                                    ws_rel.clone(),
                                                    cx,
                                                );
                                            });
                                        }),
                                )
                                .child(
                                    Button::new("btn-conflict-keep")
                                        .secondary()
                                        .small()
                                        .label("Keep mine")
                                        .on_click(move |_, _, cx| {
                                            per_keep.update(cx, |per, cx| {
                                                per.resolve_conflict_keep(
                                                    &cid_k,
                                                    ws_keep.clone(),
                                                    cx,
                                                );
                                            });
                                        }),
                                ),
                        ),
                )
            })
            // Parse error banner
            .when_some(active_parse_err, |this, err| {
                this.child(
                    div()
                        .px(px(12.))
                        .py(px(6.))
                        .bg(gpui_kit::Hsla {
                            a: 0.12,
                            ..t.status_5xx
                        })
                        .border_b_1()
                        .border_color(t.status_5xx)
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .text_xs()
                        .text_color(t.status_5xx)
                        .child("! Parse error on disk (showing last good in-memory version):")
                        .child(
                            div()
                                .font_family("monospace")
                                .text_color(t.text_primary)
                                .child(err),
                        ),
                )
            })
            // Corrupt secrets banner
            .when_some(corrupt_sec_err, |this, msg| {
                this.child(
                    div()
                        .px(px(12.))
                        .py(px(6.))
                        .bg(gpui_kit::Hsla {
                            a: 0.15,
                            ..t.status_4xx
                        })
                        .border_b_1()
                        .border_color(t.status_4xx)
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .text_xs()
                        .text_color(t.status_4xx)
                        .child(format!("! Sensitive Data Warning: {}", msg)),
                )
            })
            // Top Request Bar: Method + URL + Send
            .child({
                let view_handle = app_view_entity.clone();
                RequestBar::new(
                    ws_entity.clone(),
                    self.state.env.clone(),
                    self.state.modal.clone(),
                    ed_entity.clone(),
                    self.composer_inputs.url_input.clone(),
                    move |_, cx| {
                        view_handle.update(cx, |view, cx| {
                            view.send_current_request(cx);
                        });
                    },
                )
            })
            // Request Tabs with TabBar: Body, Params [count], Headers [count], Auth, Pre-request, Post-response
            .child({
                let active_idx = match active_tab_kind {
                    RequestTabKind::Body => 0,
                    RequestTabKind::Params => 1,
                    RequestTabKind::Headers => 2,
                    RequestTabKind::Auth => 3,
                    RequestTabKind::PreRequestScript => 4,
                    RequestTabKind::Tests => 5,
                };
                let param_count = active_req_info.as_ref().map(|info| info.0).unwrap_or(0);
                let header_count = active_req_info.as_ref().map(|info| info.1).unwrap_or(0);
                let has_pre = active_req_info.as_ref().map(|info| info.2).unwrap_or(false);
                let has_test = active_req_info.as_ref().map(|info| info.3).unwrap_or(false);
                let ed_c = ed_entity.clone();

                TabBar::new("request-composer-tabs")
                    .underline()
                    .px(px(8.))
                    .selected_index(active_idx)
                    .on_click(move |&idx, _, cx| {
                        let kind = match idx {
                            0 => RequestTabKind::Body,
                            1 => RequestTabKind::Params,
                            2 => RequestTabKind::Headers,
                            3 => RequestTabKind::Auth,
                            4 => RequestTabKind::PreRequestScript,
                            _ => RequestTabKind::Tests,
                        };
                        ed_c.update(cx, |state, cx| {
                            state.set_active_tab_kind(kind, cx);
                        });
                    })
                    .child(Tab::new().label("Body"))
                    .child(Tab::new().label("Params").when(param_count > 0, |tab| {
                        tab.suffix(render_tab_badge(
                            format!("{param_count}"),
                            active_tab_kind == RequestTabKind::Params,
                            t,
                        ))
                    }))
                    .child(Tab::new().label("Headers").when(header_count > 0, |tab| {
                        tab.suffix(render_tab_badge(
                            format!("{header_count}"),
                            active_tab_kind == RequestTabKind::Headers,
                            t,
                        ))
                    }))
                    .child(Tab::new().label("Auth"))
                    .child(Tab::new().label("Pre-request").when(has_pre, |tab| {
                        tab.suffix(render_tab_badge(
                            "*",
                            active_tab_kind == RequestTabKind::PreRequestScript,
                            t,
                        ))
                    }))
                    .child(Tab::new().label("Post-response").when(has_test, |tab| {
                        tab.suffix(render_tab_badge(
                            "*",
                            active_tab_kind == RequestTabKind::Tests,
                            t,
                        ))
                    }))
            })
            // Request Tab Content
            .child(div().flex_1().overflow_hidden().child({
                match active_tab_kind {
                    RequestTabKind::Body => div().size_full().child(
                        BodyEditor::new(
                            ed_entity.clone(),
                            self.composer_inputs.json_editor.clone(),
                            self.composer_inputs.xml_editor.clone(),
                            self.composer_inputs.raw_textarea.clone(),
                        )
                        .workspace_state(ws_entity.clone())
                        .env_state(self.state.env.clone())
                        .modal_state(self.state.modal.clone())
                        .toast_state(self.state.toast.clone()),
                    ),
                    RequestTabKind::Params => {
                        let params = ws_read
                            .find_request(&req_id_str)
                            .map(|r| r.params.clone())
                            .unwrap_or_default();
                        let ed_up = ed_entity.clone();
                        let ws_up = ws_entity.clone();
                        let ed_add = ed_entity.clone();
                        let ws_add = ws_entity.clone();
                        let ed_del = ed_entity.clone();
                        let ws_del = ws_entity.clone();
                        let req_id_k = req_id_str.clone();
                        let req_id_a = req_id_str.clone();
                        let req_id_d = req_id_str.clone();
                        div().size_full().child(
                            KvTable::new(
                                format!("params-kv-{}", req_id_str),
                                params,
                                move |idx, k, v, enabled, _, cx| {
                                    ws_up.update(cx, |ws, _| {
                                        ws.update_active_request(&req_id_k, |r| {
                                            r.update_param(idx, k, v, enabled);
                                        });
                                    });
                                    ed_up.update(cx, |state, cx| {
                                        state.mark_dirty(cx);
                                    });
                                },
                                move |_, cx| {
                                    ws_add.update(cx, |ws, _| {
                                        ws.update_active_request(&req_id_a, |r| {
                                            r.add_param();
                                        });
                                    });
                                    ed_add.update(cx, |state, cx| {
                                        state.mark_dirty(cx);
                                    });
                                },
                                move |idx, _, cx| {
                                    ws_del.update(cx, |ws, _| {
                                        ws.update_active_request(&req_id_d, |r| {
                                            r.remove_param(idx);
                                        });
                                    });
                                    ed_del.update(cx, |state, cx| {
                                        state.mark_dirty(cx);
                                    });
                                },
                            )
                            .env_state(self.state.env.clone())
                            .modal_state(self.state.modal.clone()),
                        )
                    }
                    RequestTabKind::Headers => {
                        let active_r = ws_read.find_request(&req_id_str);
                        let headers = active_r.map(|r| r.headers.clone()).unwrap_or_default();
                        let readonly_rows = active_r
                            .map(|r| inherited_readonly_rows(ws_read, r))
                            .unwrap_or_default();
                        let ed_up = ed_entity.clone();
                        let ws_up = ws_entity.clone();
                        let ed_add = ed_entity.clone();
                        let ws_add = ws_entity.clone();
                        let ed_del = ed_entity.clone();
                        let ws_del = ws_entity.clone();
                        let req_id_k = req_id_str.clone();
                        let req_id_a = req_id_str.clone();
                        let req_id_d = req_id_str.clone();
                        div().size_full().flex().flex_col().child(
                            div().flex_1().child(
                                KvTable::new(
                                    format!("headers-kv-{}", req_id_str),
                                    headers,
                                    move |idx, k, v, enabled, _, cx| {
                                        ws_up.update(cx, |ws, _| {
                                            ws.update_active_request(&req_id_k, |r| {
                                                r.update_header(idx, k, v, enabled);
                                            });
                                        });
                                        ed_up.update(cx, |state, cx| {
                                            state.mark_dirty(cx);
                                        });
                                    },
                                    move |_, cx| {
                                        ws_add.update(cx, |ws, _| {
                                            ws.update_active_request(&req_id_a, |r| {
                                                r.add_header();
                                            });
                                        });
                                        ed_add.update(cx, |state, cx| {
                                            state.mark_dirty(cx);
                                        });
                                    },
                                    move |idx, _, cx| {
                                        ws_del.update(cx, |ws, _| {
                                            ws.update_active_request(&req_id_d, |r| {
                                                r.remove_header(idx);
                                            });
                                        });
                                        ed_del.update(cx, |state, cx| {
                                            state.mark_dirty(cx);
                                        });
                                    },
                                )
                                .readonly_rows(readonly_rows)
                                .env_state(self.state.env.clone())
                                .modal_state(self.state.modal.clone()),
                            ),
                        )
                    }
                    RequestTabKind::Auth => {
                        let active_r = ws_read.find_request(&req_id_str);
                        let auth = active_r.map(|r| r.auth.clone()).unwrap_or(Auth::Inherit);
                        let folder_id = active_r.and_then(|r| r.folder_id.clone());

                        render_auth_tab(
                            &auth,
                            folder_id.as_deref(),
                            AuthTabContext {
                                workspace_state: &self.state.workspace,
                                env_state: &self.state.env,
                                modal_state: &self.state.modal,
                                editor_state: &self.state.session,
                            },
                            AuthTabInputs {
                                bearer_token_input: &self.composer_inputs.bearer_token_input,
                                basic_user_input: &self.composer_inputs.basic_user_input,
                                basic_pass_input: &self.composer_inputs.basic_pass_input,
                                apikey_key_input: &self.composer_inputs.apikey_key_input,
                                apikey_val_input: &self.composer_inputs.apikey_val_input,
                            },
                            window,
                            cx,
                        )
                    }
                    RequestTabKind::PreRequestScript => {
                        let pre_txt = self.composer_inputs.pre_script_editor.clone();
                        let script_err = ed_entity.read(cx).script_error.clone();
                        let is_vert = self.state.modal.read(cx).split_layout_vertical;
                        div().size_full().child(
                            ScriptEditor::new(
                                self.composer_inputs.pre_script_editor.clone(),
                                ScriptEditorKind::PreRequest,
                                script_err,
                                move |snippet, window, cx| {
                                    pre_txt.update(cx, |t, cx| {
                                        let cur = t.value().to_string();
                                        let new_val = if cur.is_empty() {
                                            snippet.to_string()
                                        } else if cur.ends_with('\n') {
                                            format!("{}{}", cur, snippet)
                                        } else {
                                            format!("{}\n{}", cur, snippet)
                                        };
                                        t.set_value(new_val, window, cx);
                                    });
                                },
                            )
                            .split_layout_vertical(is_vert),
                        )
                    }
                    RequestTabKind::Tests => {
                        let test_txt = self.composer_inputs.test_script_editor.clone();
                        let script_err = ed_entity.read(cx).script_error.clone();
                        let is_vert = self.state.modal.read(cx).split_layout_vertical;
                        div().size_full().child(
                            ScriptEditor::new(
                                self.composer_inputs.test_script_editor.clone(),
                                ScriptEditorKind::Tests,
                                script_err,
                                move |snippet, window, cx| {
                                    test_txt.update(cx, |t, cx| {
                                        let cur = t.value().to_string();
                                        let new_val = if cur.is_empty() {
                                            snippet.to_string()
                                        } else if cur.ends_with('\n') {
                                            format!("{}{}", cur, snippet)
                                        } else {
                                            format!("{}\n{}", cur, snippet)
                                        };
                                        t.set_value(new_val, window, cx);
                                    });
                                },
                            )
                            .split_layout_vertical(is_vert),
                        )
                    }
                }
            }))
            .into_any_element()
    }
}
