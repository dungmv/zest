use crate::components::env_input::render_env_input;
use crate::state::{EnvironmentState, ModalState, ScopeSettingsTarget, WorkspaceDataState};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::InputState;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::Icon;
use gpui_kit::{div, px, App, Entity, IntoElement, ParentElement, Styled, Window};
use zest_core::models::{ApiKeyLocation, Auth};

pub struct AuthTabContext<'a> {
    pub ws_state: &'a Entity<WorkspaceDataState>,
    pub env_state: &'a Entity<EnvironmentState>,
    pub modal_state: &'a Entity<ModalState>,
    pub target: &'a ScopeSettingsTarget,
    pub current_auth: &'a Auth,
    pub is_folder: bool,
    pub folder_id_opt: Option<&'a str>,
    pub bearer_input: &'a Entity<InputState>,
    pub basic_user_input: &'a Entity<InputState>,
    pub basic_pass_input: &'a Entity<InputState>,
    pub api_key_input: &'a Entity<InputState>,
    pub api_val_input: &'a Entity<InputState>,
}

/// Display name of an auth type, shared by the Auth tab title and its type dropdown.
pub fn auth_type_label(auth: &Auth) -> &'static str {
    match auth {
        Auth::Inherit => "Inherit",
        Auth::None => "None",
        Auth::Bearer { .. } => "Bearer Token",
        Auth::Basic { .. } => "Basic Auth",
        Auth::ApiKey { .. } => "API Key",
    }
}

pub fn render_auth_tab(
    ctx: AuthTabContext<'_>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let t = theme();
    let ws_s = ctx.ws_state.clone();
    let tgt = ctx.target.clone();

    let auth_label = auth_type_label(ctx.current_auth);

    let is_inherit = matches!(ctx.current_auth, Auth::Inherit);
    let is_none = matches!(ctx.current_auth, Auth::None);
    let is_bearer = matches!(ctx.current_auth, Auth::Bearer { .. });
    let is_basic = matches!(ctx.current_auth, Auth::Basic { .. });
    let is_apikey = matches!(ctx.current_auth, Auth::ApiKey { .. });

    div()
        .flex()
        .flex_col()
        .gap(px(14.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(
                    div()
                        .w(px(70.))
                        .flex_none()
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .text_color(t.text_secondary)
                        .child("Type:"),
                )
                .child({
                    let ws_drop = ws_s.clone();
                    let tgt_drop = tgt.clone();

                    Button::new("btn-scope-auth-type-dropdown")
                        .secondary()
                        .compact()
                        .w(px(140.))
                        .px(px(8.))
                        .py(px(4.))
                        .h(px(28.))
                        .rounded(px(4.))
                        .child(
                            div()
                                .w_full()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                                        .text_color(t.text_primary)
                                        .child(auth_label),
                                )
                                .child(
                                    Icon::new(IconName::ChevronDown)
                                        .size(px(12.))
                                        .text_color(t.text_muted),
                                ),
                        )
                        .dropdown_menu(move |menu, _window, _cx| {
                            let mut m = menu;
                            if ctx.is_folder {
                                let ws_inh = ws_drop.clone();
                                let tgt_inh = tgt_drop.clone();
                                m = m.item(
                                    PopupMenuItem::new("Inherit")
                                        .checked(is_inherit)
                                        .on_click(move |_, _, cx| {
                                            if let ScopeSettingsTarget::Folder(fid) = &tgt_inh {
                                                let cur_h = ws_inh.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default();
                                                ws_inh.update(cx, |s, cx| s.save_folder_scope_settings(fid, cur_h, Auth::Inherit, cx));
                                            }
                                        }),
                                );
                            }

                            let ws_none = ws_drop.clone();
                            let tgt_none = tgt_drop.clone();
                            let ws_bearer = ws_drop.clone();
                            let tgt_bearer = tgt_drop.clone();
                            let ws_basic = ws_drop.clone();
                            let tgt_basic = tgt_drop.clone();
                            let ws_api = ws_drop.clone();
                            let tgt_api = tgt_drop.clone();

                            m.item(
                                PopupMenuItem::new("None")
                                    .checked(is_none)
                                    .on_click(move |_, _, cx| {
                                        match &tgt_none {
                                            ScopeSettingsTarget::Workspace => {
                                                let cur_h = ws_none.read(cx).active_workspace.as_ref().map(|w| w.headers.clone()).unwrap_or_default();
                                                ws_none.update(cx, |s, cx| s.save_workspace_scope_settings(cur_h, Auth::None, cx));
                                            }
                                            ScopeSettingsTarget::Folder(fid) => {
                                                let cur_h = ws_none.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default();
                                                ws_none.update(cx, |s, cx| s.save_folder_scope_settings(fid, cur_h, Auth::None, cx));
                                            }
                                        }
                                    }),
                            )
                            .item(
                                PopupMenuItem::new("Bearer Token")
                                    .checked(is_bearer)
                                    .on_click(move |_, _, cx| {
                                        let new_auth = Auth::Bearer { token: String::new() };
                                        match &tgt_bearer {
                                            ScopeSettingsTarget::Workspace => {
                                                let cur_h = ws_bearer.read(cx).active_workspace.as_ref().map(|w| w.headers.clone()).unwrap_or_default();
                                                ws_bearer.update(cx, |s, cx| s.save_workspace_scope_settings(cur_h, new_auth, cx));
                                            }
                                            ScopeSettingsTarget::Folder(fid) => {
                                                let cur_h = ws_bearer.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default();
                                                ws_bearer.update(cx, |s, cx| s.save_folder_scope_settings(fid, cur_h, new_auth, cx));
                                            }
                                        }
                                    }),
                            )
                            .item(
                                PopupMenuItem::new("Basic Auth")
                                    .checked(is_basic)
                                    .on_click(move |_, _, cx| {
                                        let new_auth = Auth::Basic { username: String::new(), password: String::new() };
                                        match &tgt_basic {
                                            ScopeSettingsTarget::Workspace => {
                                                let cur_h = ws_basic.read(cx).active_workspace.as_ref().map(|w| w.headers.clone()).unwrap_or_default();
                                                ws_basic.update(cx, |s, cx| s.save_workspace_scope_settings(cur_h, new_auth, cx));
                                            }
                                            ScopeSettingsTarget::Folder(fid) => {
                                                let cur_h = ws_basic.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default();
                                                ws_basic.update(cx, |s, cx| s.save_folder_scope_settings(fid, cur_h, new_auth, cx));
                                            }
                                        }
                                    }),
                            )
                            .item(
                                PopupMenuItem::new("API Key")
                                    .checked(is_apikey)
                                    .on_click(move |_, _, cx| {
                                        let new_auth = Auth::ApiKey { key: "X-API-Key".to_string(), value: String::new(), add_to: ApiKeyLocation::Header };
                                        match &tgt_api {
                                            ScopeSettingsTarget::Workspace => {
                                                let cur_h = ws_api.read(cx).active_workspace.as_ref().map(|w| w.headers.clone()).unwrap_or_default();
                                                ws_api.update(cx, |s, cx| s.save_workspace_scope_settings(cur_h, new_auth, cx));
                                            }
                                            ScopeSettingsTarget::Folder(fid) => {
                                                let cur_h = ws_api.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default();
                                                ws_api.update(cx, |s, cx| s.save_folder_scope_settings(fid, cur_h, new_auth, cx));
                                            }
                                        }
                                    }),
                            )
                        })
                }),
        )
        .child({
            match ctx.current_auth {
                Auth::Inherit => {
                    let desc = ws_s.read(cx).get_inherited_auth_description(ctx.folder_id_opt);
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
                        .child(
                            div()
                                .text_xs()
                                .text_color(t.text_secondary)
                                .child(desc),
                        )
                }
                Auth::None => {
                    div()
                        .pt(px(4.))
                        .text_xs()
                        .italic()
                        .text_color(t.text_muted)
                        .child(if ctx.is_folder {
                            "No authentication will be added to requests in this folder."
                        } else {
                            "No authentication will be added to requests in this workspace."
                        })
                }
                Auth::Bearer { .. } => {
                    let ws_b = ctx.ws_state.clone();
                    let tgt_b = ctx.target.clone();
                    let on_bearer_update = move |v: String, _window: &mut Window, cx: &mut App| {
                        let new_auth = Auth::Bearer { token: v };
                        match &tgt_b {
                            ScopeSettingsTarget::Workspace => {
                                let cur_h = ws_b.read(cx).active_workspace.as_ref().map(|w| w.headers.clone()).unwrap_or_default();
                                ws_b.update(cx, |s, cx| s.save_workspace_scope_settings(cur_h, new_auth, cx));
                            }
                            ScopeSettingsTarget::Folder(fid) => {
                                let cur_h = ws_b.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default();
                                ws_b.update(cx, |s, cx| s.save_folder_scope_settings(fid, cur_h, new_auth, cx));
                            }
                        }
                    };

                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(
                            div()
                                .w(px(70.))
                                .flex_none()
                                .text_xs()
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                .text_color(t.text_secondary)
                                .child("Token:"),
                        )
                        .child(
                            div()
                                .flex_1()
                                .child(render_env_input(
                                    "scope-auth-bearer-token",
                                    ctx.bearer_input,
                                    Some(ctx.env_state),
                                    Some(ctx.modal_state),
                                    window,
                                    cx,
                                    on_bearer_update,
                                )),
                        )
                }
                Auth::Basic { .. } => {
                    let ws_s_u = ctx.ws_state.clone();
                    let tgt_u = ctx.target.clone();
                    let on_user_update = move |u: String, _window: &mut Window, cx: &mut App| {
                        let pass = match &tgt_u {
                            ScopeSettingsTarget::Workspace => match &ws_s_u.read(cx).workspace_meta.auth {
                                Auth::Basic { password, .. } => password.clone(),
                                _ => String::new(),
                            },
                            ScopeSettingsTarget::Folder(fid) => match ws_s_u.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| &f.auth) {
                                Some(Auth::Basic { password, .. }) => password.clone(),
                                _ => String::new(),
                            },
                        };
                        let new_auth = Auth::Basic { username: u, password: pass };
                        match &tgt_u {
                            ScopeSettingsTarget::Workspace => {
                                let cur_h = ws_s_u.read(cx).active_workspace.as_ref().map(|w| w.headers.clone()).unwrap_or_default();
                                ws_s_u.update(cx, |s, cx| s.save_workspace_scope_settings(cur_h, new_auth, cx));
                            }
                            ScopeSettingsTarget::Folder(fid) => {
                                let cur_h = ws_s_u.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default();
                                ws_s_u.update(cx, |s, cx| s.save_folder_scope_settings(fid, cur_h, new_auth, cx));
                            }
                        }
                    };

                    let ws_s_p = ctx.ws_state.clone();
                    let tgt_p = ctx.target.clone();
                    let on_pass_update = move |p: String, _window: &mut Window, cx: &mut App| {
                        let user = match &tgt_p {
                            ScopeSettingsTarget::Workspace => match &ws_s_p.read(cx).workspace_meta.auth {
                                Auth::Basic { username, .. } => username.clone(),
                                _ => String::new(),
                            },
                            ScopeSettingsTarget::Folder(fid) => match ws_s_p.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| &f.auth) {
                                Some(Auth::Basic { username, .. }) => username.clone(),
                                _ => String::new(),
                            },
                        };
                        let new_auth = Auth::Basic { username: user, password: p };
                        match &tgt_p {
                            ScopeSettingsTarget::Workspace => {
                                let cur_h = ws_s_p.read(cx).active_workspace.as_ref().map(|w| w.headers.clone()).unwrap_or_default();
                                ws_s_p.update(cx, |s, cx| s.save_workspace_scope_settings(cur_h, new_auth, cx));
                            }
                            ScopeSettingsTarget::Folder(fid) => {
                                let cur_h = ws_s_p.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default();
                                ws_s_p.update(cx, |s, cx| s.save_folder_scope_settings(fid, cur_h, new_auth, cx));
                            }
                        }
                    };

                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .child(
                                    div()
                                        .w(px(70.))
                                        .flex_none()
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                        .text_color(t.text_secondary)
                                        .child("Username:"),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .child(render_env_input(
                                            "scope-auth-basic-user",
                                            ctx.basic_user_input,
                                            Some(ctx.env_state),
                                            Some(ctx.modal_state),
                                            window,
                                            cx,
                                            on_user_update,
                                        )),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .child(
                                    div()
                                        .w(px(70.))
                                        .flex_none()
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                        .text_color(t.text_secondary)
                                        .child("Password:"),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .child(render_env_input(
                                            "scope-auth-basic-pass",
                                            ctx.basic_pass_input,
                                            Some(ctx.env_state),
                                            Some(ctx.modal_state),
                                            window,
                                            cx,
                                            on_pass_update,
                                        )),
                                ),
                        )
                }
                Auth::ApiKey { add_to, .. } => {
                    let add_to_val = *add_to;
                    let ws_s_k = ctx.ws_state.clone();
                    let tgt_k = ctx.target.clone();
                    let on_api_key_update = move |k: String, _window: &mut Window, cx: &mut App| {
                        let (v, cur_loc) = match &tgt_k {
                            ScopeSettingsTarget::Workspace => match &ws_s_k.read(cx).workspace_meta.auth {
                                Auth::ApiKey { value, add_to, .. } => (value.clone(), *add_to),
                                _ => (String::new(), ApiKeyLocation::Header),
                            },
                            ScopeSettingsTarget::Folder(fid) => match ws_s_k.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| &f.auth) {
                                Some(Auth::ApiKey { value, add_to, .. }) => (value.clone(), *add_to),
                                _ => (String::new(), ApiKeyLocation::Header),
                            },
                        };
                        let new_auth = Auth::ApiKey { key: k, value: v, add_to: cur_loc };
                        match &tgt_k {
                            ScopeSettingsTarget::Workspace => {
                                let cur_h = ws_s_k.read(cx).active_workspace.as_ref().map(|w| w.headers.clone()).unwrap_or_default();
                                ws_s_k.update(cx, |s, cx| s.save_workspace_scope_settings(cur_h, new_auth, cx));
                            }
                            ScopeSettingsTarget::Folder(fid) => {
                                let cur_h = ws_s_k.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default();
                                ws_s_k.update(cx, |s, cx| s.save_folder_scope_settings(fid, cur_h, new_auth, cx));
                            }
                        }
                    };

                    let ws_s_v = ctx.ws_state.clone();
                    let tgt_v = ctx.target.clone();
                    let on_api_val_update = move |v: String, _window: &mut Window, cx: &mut App| {
                        let (k, cur_loc) = match &tgt_v {
                            ScopeSettingsTarget::Workspace => match &ws_s_v.read(cx).workspace_meta.auth {
                                Auth::ApiKey { key, add_to, .. } => (key.clone(), *add_to),
                                _ => ("X-API-Key".to_string(), ApiKeyLocation::Header),
                            },
                            ScopeSettingsTarget::Folder(fid) => match ws_s_v.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| &f.auth) {
                                Some(Auth::ApiKey { key, add_to, .. }) => (key.clone(), *add_to),
                                _ => ("X-API-Key".to_string(), ApiKeyLocation::Header),
                            },
                        };
                        let new_auth = Auth::ApiKey { key: k, value: v, add_to: cur_loc };
                        match &tgt_v {
                            ScopeSettingsTarget::Workspace => {
                                let cur_h = ws_s_v.read(cx).active_workspace.as_ref().map(|w| w.headers.clone()).unwrap_or_default();
                                ws_s_v.update(cx, |s, cx| s.save_workspace_scope_settings(cur_h, new_auth, cx));
                            }
                            ScopeSettingsTarget::Folder(fid) => {
                                let cur_h = ws_s_v.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default();
                                ws_s_v.update(cx, |s, cx| s.save_folder_scope_settings(fid, cur_h, new_auth, cx));
                            }
                        }
                    };

                    let api_key_input = ctx.api_key_input;
                    let api_val_input = ctx.api_val_input;

                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .child(
                                    div()
                                        .w(px(70.))
                                        .flex_none()
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                        .text_color(t.text_secondary)
                                        .child("Key:"),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .child(render_env_input(
                                            "scope-auth-apikey-key",
                                            api_key_input,
                                            Some(ctx.env_state),
                                            Some(ctx.modal_state),
                                            window,
                                            cx,
                                            on_api_key_update,
                                        )),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .child(
                                    div()
                                        .w(px(70.))
                                        .flex_none()
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                        .text_color(t.text_secondary)
                                        .child("Value:"),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .child(render_env_input(
                                            "scope-auth-apikey-val",
                                            api_val_input,
                                            Some(ctx.env_state),
                                            Some(ctx.modal_state),
                                            window,
                                            cx,
                                            on_api_val_update,
                                        )),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .child(
                                    div()
                                        .w(px(70.))
                                        .flex_none()
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                        .text_color(t.text_secondary)
                                        .child("Add to:"),
                                )
                                .child({
                                    let loc_label = if add_to_val.is_header() { "Header" } else { "Query Params" };
                                    let ws_loc = ctx.ws_state.clone();
                                    let tgt_loc = ctx.target.clone();
                                    let k_input = api_key_input.clone();
                                    let v_input = api_val_input.clone();

                                    Button::new("btn-scope-auth-apikey-location-dropdown")
                                        .secondary()
                                        .compact()
                                        .min_w(px(120.))
                                        .px(px(8.))
                                        .py(px(4.))
                                        .h(px(28.))
                                        .rounded(px(4.))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui_kit::FontWeight::MEDIUM)
                                                .text_color(t.text_primary)
                                                .child(loc_label),
                                        )
                                        .dropdown_menu(move |menu, _window, _cx| {
                                            let ws_h = ws_loc.clone();
                                            let tgt_h = tgt_loc.clone();
                                            let k_h = k_input.clone();
                                            let v_h = v_input.clone();

                                            let ws_q = ws_loc.clone();
                                            let tgt_q = tgt_loc.clone();
                                            let k_q = k_input.clone();
                                            let v_q = v_input.clone();

                                            menu.item(
                                                PopupMenuItem::new("Header")
                                                    .checked(add_to_val.is_header())
                                                    .on_click(move |_, _, cx| {
                                                        let key = k_h.read(cx).value().to_string();
                                                        let val = v_h.read(cx).value().to_string();
                                                        match &tgt_h {
                                                            ScopeSettingsTarget::Workspace => {
                                                                let cur_h = ws_h.read(cx).active_workspace.as_ref().map(|w| w.headers.clone()).unwrap_or_default();
                                                                ws_h.update(cx, |s, cx| s.save_workspace_scope_settings(cur_h, Auth::ApiKey { key, value: val, add_to: ApiKeyLocation::Header }, cx));
                                                            }
                                                            ScopeSettingsTarget::Folder(fid) => {
                                                                let cur_h = ws_h.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default();
                                                                ws_h.update(cx, |s, cx| s.save_folder_scope_settings(fid, cur_h, Auth::ApiKey { key, value: val, add_to: ApiKeyLocation::Header }, cx));
                                                            }
                                                        }
                                                    }),
                                            )
                                            .item(
                                                PopupMenuItem::new("Query Params")
                                                    .checked(add_to_val.is_query())
                                                    .on_click(move |_, _, cx| {
                                                        let key = k_q.read(cx).value().to_string();
                                                        let val = v_q.read(cx).value().to_string();
                                                        match &tgt_q {
                                                            ScopeSettingsTarget::Workspace => {
                                                                let cur_h = ws_q.read(cx).active_workspace.as_ref().map(|w| w.headers.clone()).unwrap_or_default();
                                                                ws_q.update(cx, |s, cx| s.save_workspace_scope_settings(cur_h, Auth::ApiKey { key, value: val, add_to: ApiKeyLocation::Query }, cx));
                                                            }
                                                            ScopeSettingsTarget::Folder(fid) => {
                                                                let cur_h = ws_q.read(cx).folders.iter().find(|f| &f.id == fid).map(|f| f.headers.clone()).unwrap_or_default();
                                                                ws_q.update(cx, |s, cx| s.save_folder_scope_settings(fid, cur_h, Auth::ApiKey { key, value: val, add_to: ApiKeyLocation::Query }, cx));
                                                            }
                                                        }
                                                    }),
                                            )
                                        })
                                }),
                        )
                }
            }
        })
}
