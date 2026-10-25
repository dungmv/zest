use crate::components::env_input::render_env_input;
use crate::components::inherited_headers::mask_secret;
use crate::components::kv_table::render_disabled_input;
use crate::state::{EnvironmentState, ModalState, RequestSessionState, WorkspaceDataState};
use crate::theme::{theme, AppTheme};
use gpui::prelude::FluentBuilder as _;
use gpui::{div, px, App, Div, Entity, ParentElement, Styled, Window};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::InputState;
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::Icon;
use gpui_kit::assets::IconName;
use zest_core::models::Auth;

pub struct AuthTabInputs<'a> {
    pub bearer_token_input: &'a Entity<InputState>,
    pub basic_user_input: &'a Entity<InputState>,
    pub basic_pass_input: &'a Entity<InputState>,
    pub apikey_key_input: &'a Entity<InputState>,
    pub apikey_val_input: &'a Entity<InputState>,
}

pub struct AuthTabContext<'a> {
    pub workspace_state: &'a Entity<WorkspaceDataState>,
    pub env_state: &'a Entity<EnvironmentState>,
    pub modal_state: &'a Entity<ModalState>,
    pub editor_state: &'a Entity<RequestSessionState>,
}

pub fn render_auth_tab(
    auth: &Auth,
    folder_id: Option<&str>,
    ctx: AuthTabContext,
    inputs: AuthTabInputs,
    window: &mut Window,
    cx: &mut App,
) -> Div {
    let workspace_state = ctx.workspace_state;
    let env_state = ctx.env_state;
    let modal_state = ctx.modal_state;
    let editor_state = ctx.editor_state;

    let t = theme();
    let ed_c = editor_state.clone();
    let ws_c = workspace_state.clone();
    let req_id_str = editor_state.read(cx).active_request_id().to_string();
    let auth_label = match auth {
        Auth::Inherit => "Inherit",
        Auth::None => "None",
        Auth::Bearer { .. } => "Bearer Token",
        Auth::Basic { .. } => "Basic Auth",
        Auth::ApiKey { .. } => "API Key",
    };
    let is_inherit = matches!(auth, Auth::Inherit);
    let is_none = matches!(auth, Auth::None);
    let is_bearer = matches!(auth, Auth::Bearer { .. });
    let is_basic = matches!(auth, Auth::Basic { .. });
    let is_apikey = matches!(auth, Auth::ApiKey { .. });

    div()
        .p(px(16.))
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(
                    div()
                        .w(px(70.))
                        .text_xs()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(t.text_secondary)
                        .child("Type:"),
                )
                .child({
                    let ed_m = ed_c.clone();
                    let ws_m = ws_c.clone();
                    let req_id_m = req_id_str.clone();
                    Button::new("btn-auth-type-dropdown")
                        .ghost()
                        .compact()
                        .px(px(8.))
                        .py(px(4.))
                        .h(px(28.))
                        .rounded(px(4.))
                        .bg(t.bg_card)
                        .border_1()
                        .border_color(t.border)
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(t.text_primary)
                                .child(auth_label),
                        )
                        .dropdown_menu(move |menu, _window, _cx| {
                            let ed_inh = ed_m.clone();
                            let ws_inh = ws_m.clone();
                            let req_inh = req_id_m.clone();
                            let ed_none = ed_m.clone();
                            let ws_none = ws_m.clone();
                            let req_none = req_id_m.clone();
                            let ed_bearer = ed_m.clone();
                            let ws_bearer = ws_m.clone();
                            let req_bearer = req_id_m.clone();
                            let ed_basic = ed_m.clone();
                            let ws_basic = ws_m.clone();
                            let req_basic = req_id_m.clone();
                            let ed_apikey = ed_m.clone();
                            let ws_apikey = ws_m.clone();
                            let req_apikey = req_id_m.clone();

                            menu.item(PopupMenuItem::new("Inherit").checked(is_inherit).on_click(
                                move |_, _, cx| {
                                    ws_inh.update(cx, |ws, _| {
                                        ws.update_active_request(&req_inh, |r| {
                                            r.set_auth(Auth::Inherit);
                                        });
                                    });
                                    ed_inh.update(cx, |ed, cx| {
                                        ed.mark_dirty(cx);
                                    });
                                },
                            ))
                            .item(PopupMenuItem::new("None").checked(is_none).on_click(
                                move |_, _, cx| {
                                    ws_none.update(cx, |ws, _| {
                                        ws.update_active_request(&req_none, |r| {
                                            r.set_auth(Auth::None);
                                        });
                                    });
                                    ed_none.update(cx, |ed, cx| {
                                        ed.mark_dirty(cx);
                                    });
                                },
                            ))
                            .item(
                                PopupMenuItem::new("Bearer Token")
                                    .checked(is_bearer)
                                    .on_click(move |_, _, cx| {
                                        ws_bearer.update(cx, |ws, _| {
                                            ws.update_active_request(&req_bearer, |r| {
                                                r.set_auth(Auth::Bearer {
                                                    token: "".to_string(),
                                                });
                                            });
                                        });
                                        ed_bearer.update(cx, |ed, cx| {
                                            ed.mark_dirty(cx);
                                        });
                                    }),
                            )
                            .item(PopupMenuItem::new("Basic Auth").checked(is_basic).on_click(
                                move |_, _, cx| {
                                    ws_basic.update(cx, |ws, _| {
                                        ws.update_active_request(&req_basic, |r| {
                                            r.set_auth(Auth::Basic {
                                                username: "".to_string(),
                                                password: "".to_string(),
                                            });
                                        });
                                    });
                                    ed_basic.update(cx, |ed, cx| {
                                        ed.mark_dirty(cx);
                                    });
                                },
                            ))
                            .item(
                                PopupMenuItem::new("API Key").checked(is_apikey).on_click(
                                    move |_, _, cx| {
                                        ws_apikey.update(cx, |ws, _| {
                                            ws.update_active_request(&req_apikey, |r| {
                                                r.set_auth(Auth::ApiKey {
                                                    key: "X-API-Key".to_string(),
                                                    value: "".to_string(),
                                                    in_header: true,
                                                });
                                            });
                                        });
                                        ed_apikey.update(cx, |ed, cx| {
                                            ed.mark_dirty(cx);
                                        });
                                    },
                                ),
                            )
                        })
                }),
        )
        .child({
            match auth {
                Auth::Inherit => {
                    let ws_read = workspace_state.read(cx);
                    let desc = ws_read.get_inherited_auth_description(folder_id);
                    let inherited = ws_read.inherited_auth_source(folder_id);
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .child(
                            div()
                                .p(px(10.))
                                .rounded(px(6.))
                                .bg(t.bg_app)
                                .border_1()
                                .border_color(t.border)
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(Icon::new(IconName::Info).size(px(16.)))
                                .child(div().text_xs().text_color(t.text_secondary).child(desc)),
                        )
                        .when_some(inherited, |this, info| {
                            this.child(render_inherited_auth_fields(&info.auth, t))
                        })
                }
                Auth::None => div()
                    .pt(px(4.))
                    .text_xs()
                    .italic()
                    .text_color(t.text_muted)
                    .child("No authentication will be added to this request."),
                Auth::Bearer { .. } => {
                    let ed_b = ed_c.clone();
                    let ws_b = ws_c.clone();
                    let req_b = req_id_str.clone();
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(
                            div()
                                .w(px(70.))
                                .text_xs()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(t.text_secondary)
                                .child("Token:"),
                        )
                        .child(div().w_full().max_w(px(400.)).child(render_env_input(
                            "auth-bearer-token-input",
                            inputs.bearer_token_input,
                            Some(env_state),
                            Some(modal_state),
                            window,
                            cx,
                            move |new_token, _window, cx| {
                                ws_b.update(cx, |ws, _| {
                                    ws.update_active_request(&req_b, |r| {
                                        r.set_bearer_token(new_token);
                                    });
                                });
                                ed_b.update(cx, |ed, cx| {
                                    ed.mark_dirty(cx);
                                });
                            },
                        )))
                }
                Auth::Basic { .. } => {
                    let ed_u = ed_c.clone();
                    let ws_u = ws_c.clone();
                    let req_u = req_id_str.clone();
                    let ed_p = ed_c.clone();
                    let ws_p = ws_c.clone();
                    let req_p = req_id_str.clone();
                    let p_in = inputs.basic_pass_input.clone();
                    let u_in = inputs.basic_user_input.clone();
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .child(
                                    div()
                                        .w(px(70.))
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(t.text_secondary)
                                        .child("Username:"),
                                )
                                .child(div().w_full().max_w(px(400.)).child(render_env_input(
                                    "auth-basic-user-input",
                                    inputs.basic_user_input,
                                    Some(env_state),
                                    Some(modal_state),
                                    window,
                                    cx,
                                    move |new_user, _window, cx| {
                                        let pass = p_in.read(cx).value().to_string();
                                        ws_u.update(cx, |ws, _| {
                                            ws.update_active_request(&req_u, |r| {
                                                r.set_basic_auth(new_user, pass);
                                            });
                                        });
                                        ed_u.update(cx, |ed, cx| {
                                            ed.mark_dirty(cx);
                                        });
                                    },
                                ))),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .child(
                                    div()
                                        .w(px(70.))
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(t.text_secondary)
                                        .child("Password:"),
                                )
                                .child(div().w_full().max_w(px(400.)).child(render_env_input(
                                    "auth-basic-pass-input",
                                    inputs.basic_pass_input,
                                    Some(env_state),
                                    Some(modal_state),
                                    window,
                                    cx,
                                    move |new_pass, _window, cx| {
                                        let user = u_in.read(cx).value().to_string();
                                        ws_p.update(cx, |ws, _| {
                                            ws.update_active_request(&req_p, |r| {
                                                r.set_basic_auth(user, new_pass);
                                            });
                                        });
                                        ed_p.update(cx, |ed, cx| {
                                            ed.mark_dirty(cx);
                                        });
                                    },
                                ))),
                        )
                }
                Auth::ApiKey { in_header, .. } => {
                    let in_header_val = *in_header;
                    let ed_loc = ed_c.clone();
                    let ws_loc = ws_c.clone();
                    let req_loc = req_id_str.clone();
                    let ed_k = ed_c.clone();
                    let ws_k = ws_c.clone();
                    let req_k = req_id_str.clone();
                    let ed_v = ed_c.clone();
                    let ws_v = ws_c.clone();
                    let req_v = req_id_str.clone();
                    let v_in = inputs.apikey_val_input.clone();
                    let k_in = inputs.apikey_key_input.clone();
                    let loc_label = if in_header_val {
                        "Header"
                    } else {
                        "Query Params"
                    };

                    div()
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .child(
                                    div()
                                        .w(px(70.))
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(t.text_secondary)
                                        .child("Key:"),
                                )
                                .child(div().w_full().max_w(px(400.)).child(render_env_input(
                                    "auth-apikey-key-input",
                                    inputs.apikey_key_input,
                                    Some(env_state),
                                    Some(modal_state),
                                    window,
                                    cx,
                                    move |new_key, _window, cx| {
                                        let val = v_in.read(cx).value().to_string();
                                        ws_k.update(cx, |ws, _| {
                                            ws.update_active_request(&req_k, |r| {
                                                r.set_api_key(new_key, val, in_header_val);
                                            });
                                        });
                                        ed_k.update(cx, |ed, cx| {
                                            ed.mark_dirty(cx);
                                        });
                                    },
                                ))),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .child(
                                    div()
                                        .w(px(70.))
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(t.text_secondary)
                                        .child("Value:"),
                                )
                                .child(div().w_full().max_w(px(400.)).child(render_env_input(
                                    "auth-apikey-val-input",
                                    inputs.apikey_val_input,
                                    Some(env_state),
                                    Some(modal_state),
                                    window,
                                    cx,
                                    move |new_val, _window, cx| {
                                        let key = k_in.read(cx).value().to_string();
                                        ws_v.update(cx, |ws, _| {
                                            ws.update_active_request(&req_v, |r| {
                                                r.set_api_key(key, new_val, in_header_val);
                                            });
                                        });
                                        ed_v.update(cx, |ed, cx| {
                                            ed.mark_dirty(cx);
                                        });
                                    },
                                ))),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .child(
                                    div()
                                        .w(px(70.))
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(t.text_secondary)
                                        .child("Add to:"),
                                )
                                .child({
                                    let ed_l = ed_loc.clone();
                                    let ws_l = ws_loc.clone();
                                    let req_l = req_loc.clone();
                                    let k_input = inputs.apikey_key_input.clone();
                                    let v_input = inputs.apikey_val_input.clone();
                                    Button::new("btn-auth-apikey-location-dropdown")
                                        .ghost()
                                        .compact()
                                        .px(px(8.))
                                        .py(px(4.))
                                        .h(px(28.))
                                        .rounded(px(4.))
                                        .bg(t.bg_card)
                                        .border_1()
                                        .border_color(t.border)
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .text_color(t.text_primary)
                                                .child(loc_label),
                                        )
                                        .dropdown_menu(move |menu, _window, _cx| {
                                            let ed_h = ed_l.clone();
                                            let ws_h = ws_l.clone();
                                            let req_h = req_l.clone();
                                            let ed_q = ed_l.clone();
                                            let ws_q = ws_l.clone();
                                            let req_q = req_l.clone();
                                            let k_h = k_input.clone();
                                            let v_h = v_input.clone();
                                            let k_q = k_input.clone();
                                            let v_q = v_input.clone();

                                            menu.item(
                                                PopupMenuItem::new("Header")
                                                    .checked(in_header_val)
                                                    .on_click(move |_, _, cx| {
                                                        let key = k_h.read(cx).value().to_string();
                                                        let val = v_h.read(cx).value().to_string();
                                                        ws_h.update(cx, |ws, _| {
                                                            ws.update_active_request(&req_h, |r| {
                                                                r.set_api_key(key, val, true);
                                                            });
                                                        });
                                                        ed_h.update(cx, |ed, cx| {
                                                            ed.mark_dirty(cx);
                                                        });
                                                    }),
                                            )
                                            .item(
                                                PopupMenuItem::new("Query Params")
                                                    .checked(!in_header_val)
                                                    .on_click(move |_, _, cx| {
                                                        let key = k_q.read(cx).value().to_string();
                                                        let val = v_q.read(cx).value().to_string();
                                                        ws_q.update(cx, |ws, _| {
                                                            ws.update_active_request(&req_q, |r| {
                                                                r.set_api_key(key, val, false);
                                                            });
                                                        });
                                                        ed_q.update(cx, |ed, cx| {
                                                            ed.mark_dirty(cx);
                                                        });
                                                    }),
                                            )
                                        })
                                }),
                        )
                }
            }
        })
}

/// Read-only mirror of a local auth form row: same label/input layout, disabled input.
fn disabled_field(label: &str, value: &str, t: &AppTheme) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .child(
            div()
                .w(px(70.))
                .text_xs()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(t.text_secondary)
                .child(label.to_string()),
        )
        .child(
            div()
                .w_full()
                .max_w(px(400.))
                .child(render_disabled_input(value, t)),
        )
}

/// Disabled mirror of the local auth forms, shown under the inherit description so the
/// user sees the effective inherited configuration (secrets masked).
fn render_inherited_auth_fields(auth: &Auth, t: &AppTheme) -> Div {
    match auth {
        Auth::Bearer { token } => disabled_field("Token:", &mask_secret(token), t),
        Auth::Basic { username, password } => div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(disabled_field("Username:", username, t))
            .child(disabled_field("Password:", &mask_secret(password), t)),
        Auth::ApiKey {
            key,
            value,
            in_header,
        } => div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(disabled_field("Key:", key, t))
            .child(disabled_field("Value:", &mask_secret(value), t))
            .child(disabled_field(
                "Add to:",
                if *in_header { "Header" } else { "Query Params" },
                t,
            )),
        // None adds no fields: the description banner already states no auth applies.
        // Inherit is unreachable: `inherited_auth_source` never reports `Inherit`.
        Auth::None | Auth::Inherit => div(),
    }
}
