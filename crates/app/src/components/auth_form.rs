//! Shared Auth configuration form used by both Request Composer and Workspace/Folder Settings.

use std::sync::Arc;

use crate::components::common::mask_secret;
use crate::components::env_input::{render_disabled_env_input, render_env_input};
use crate::state::{CollectionState, EnvironmentState, ModalState};
use crate::theme::{theme, AppTheme};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::InputState;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{div, px, App, Div, Entity, ParentElement, Styled, Window};
use zest_core::models::{ApiKeyLocation, Auth};

pub type SaveAuthCallback = Arc<dyn Fn(Auth, &mut App) + 'static>;

pub struct AuthFormInputs<'a> {
    pub bearer_token: &'a Entity<InputState>,
    pub basic_user: &'a Entity<InputState>,
    pub basic_pass: &'a Entity<InputState>,
    pub apikey_key: &'a Entity<InputState>,
    pub apikey_val: &'a Entity<InputState>,
}

pub struct AuthFormIds {
    pub type_dropdown: &'static str,
    pub bearer_token: &'static str,
    pub basic_user: &'static str,
    pub basic_pass: &'static str,
    pub apikey_key: &'static str,
    pub apikey_val: &'static str,
    pub apikey_location_dropdown: &'static str,
}

pub struct AuthFormOptions {
    pub ids: AuthFormIds,
    pub allow_inherit: bool,
    pub show_inherited_preview: bool,
    pub compact_rows: bool,
    pub none_message: &'static str,
}

pub struct AuthFormContext<'a> {
    pub auth: &'a Auth,
    pub folder_id: Option<&'a str>,
    pub workspace_state: &'a Entity<CollectionState>,
    pub env_state: &'a Entity<EnvironmentState>,
    pub modal_state: &'a Entity<ModalState>,
    pub inputs: AuthFormInputs<'a>,
    pub options: AuthFormOptions,
    pub on_save: SaveAuthCallback,
}

pub fn render_auth_form(ctx: AuthFormContext<'_>, window: &mut Window, cx: &mut App) -> Div {
    let t = theme();
    let compact = ctx.options.compact_rows;

    let type_row = render_type_row(
        ctx.auth,
        ctx.options.ids.type_dropdown,
        ctx.options.allow_inherit,
        compact,
        ctx.on_save.clone(),
        t,
    );

    let body = match ctx.auth {
        Auth::Inherit => render_inherit_section(
            ctx.folder_id,
            ctx.workspace_state,
            ctx.env_state,
            ctx.modal_state,
            ctx.options.show_inherited_preview,
            compact,
            t,
            cx,
        ),
        Auth::None => render_none_section(ctx.options.none_message, compact, t),
        Auth::Bearer { .. } => render_bearer_section(
            ctx.options.ids.bearer_token,
            ctx.inputs.bearer_token,
            ctx.env_state,
            ctx.modal_state,
            compact,
            ctx.on_save,
            t,
            window,
            cx,
        ),
        Auth::Basic { .. } => render_basic_section(
            ctx.options.ids.basic_user,
            ctx.options.ids.basic_pass,
            ctx.inputs.basic_user,
            ctx.inputs.basic_pass,
            ctx.env_state,
            ctx.modal_state,
            compact,
            ctx.on_save,
            t,
            window,
            cx,
        ),
        Auth::ApiKey { add_to, .. } => render_apikey_section(
            *add_to,
            &ctx.options.ids,
            ctx.inputs.apikey_key,
            ctx.inputs.apikey_val,
            ctx.env_state,
            ctx.modal_state,
            compact,
            ctx.on_save,
            t,
            window,
            cx,
        ),
    };

    let container = div().flex().flex_col();
    let container = if compact {
        container.size_full().p(px(8.))
    } else {
        container.gap(px(14.))
    };

    container.child(type_row).child(body)
}

fn form_row(label: &str, compact: bool, t: &AppTheme) -> Div {
    let row = div().w_full().flex().items_center().gap(px(10.));
    let row = if compact {
        row.px(px(6.)).py(px(2.))
    } else {
        row
    };
    row.child(
        div()
            .w(px(70.))
            .flex_none()
            .text_xs()
            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
            .text_color(t.text_secondary)
            .child(label.to_string()),
    )
}

fn render_type_row(
    auth: &Auth,
    dropdown_id: &'static str,
    allow_inherit: bool,
    compact: bool,
    on_save: SaveAuthCallback,
    t: &AppTheme,
) -> Div {
    let is_inherit = matches!(auth, Auth::Inherit);
    let is_none = matches!(auth, Auth::None);
    let is_bearer = matches!(auth, Auth::Bearer { .. });
    let is_basic = matches!(auth, Auth::Basic { .. });
    let is_apikey = matches!(auth, Auth::ApiKey { .. });
    let auth_label = auth.display_label();

    form_row("Type:", compact, t).child(
        Button::new(dropdown_id)
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
            .dropdown_menu(move |mut menu, _window, _cx| {
                if allow_inherit {
                    let save = on_save.clone();
                    menu = menu.item(
                        PopupMenuItem::new("Inherit")
                            .checked(is_inherit)
                            .on_click(move |_, _, cx| save(Auth::Inherit, cx)),
                    );
                }

                let save_none = on_save.clone();
                let save_bearer = on_save.clone();
                let save_basic = on_save.clone();
                let save_apikey = on_save.clone();

                menu.item(
                    PopupMenuItem::new("None")
                        .checked(is_none)
                        .on_click(move |_, _, cx| save_none(Auth::None, cx)),
                )
                .item(
                    PopupMenuItem::new("Bearer Token")
                        .checked(is_bearer)
                        .on_click(move |_, _, cx| {
                            save_bearer(
                                Auth::Bearer {
                                    token: String::new(),
                                },
                                cx,
                            );
                        }),
                )
                .item(PopupMenuItem::new("Basic Auth").checked(is_basic).on_click(
                    move |_, _, cx| {
                        save_basic(
                            Auth::Basic {
                                username: String::new(),
                                password: String::new(),
                            },
                            cx,
                        );
                    },
                ))
                .item(
                    PopupMenuItem::new("API Key")
                        .checked(is_apikey)
                        .on_click(move |_, _, cx| {
                            save_apikey(
                                Auth::ApiKey {
                                    key: "X-API-Key".to_string(),
                                    value: String::new(),
                                    add_to: ApiKeyLocation::Header,
                                },
                                cx,
                            );
                        }),
                )
            }),
    )
}

#[allow(clippy::too_many_arguments)]
fn render_inherit_section(
    folder_id: Option<&str>,
    workspace_state: &Entity<CollectionState>,
    env_state: &Entity<EnvironmentState>,
    modal_state: &Entity<ModalState>,
    show_inherited_preview: bool,
    compact: bool,
    t: &AppTheme,
    cx: &App,
) -> Div {
    let ws_read = workspace_state.read(cx);
    let desc = ws_read.get_inherited_auth_description(folder_id);
    let banner = div()
        .p(if compact { px(8.) } else { px(10.) })
        .rounded(px(6.))
        .bg(t.bg_app)
        .border_1()
        .border_color(t.border)
        .flex()
        .items_center()
        .gap(px(8.))
        .child(Icon::new(IconName::Info).size(px(16.)))
        .child(div().text_xs().text_color(t.text_secondary).child(desc));

    if !show_inherited_preview {
        return banner;
    }

    let inherited = ws_read.inherited_auth_source(folder_id);
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(div().w_full().px(px(6.)).py(px(2.)).child(banner))
        .when_some(inherited, |this, info| {
            this.child(render_inherited_auth_fields(
                &info.auth,
                env_state,
                modal_state,
                t,
                cx,
            ))
        })
}

fn render_none_section(message: &'static str, compact: bool, t: &AppTheme) -> Div {
    let base = div()
        .text_xs()
        .italic()
        .text_color(t.text_muted)
        .child(message);
    if compact {
        base.w_full().px(px(6.)).py(px(2.))
    } else {
        base.pt(px(4.))
    }
}

#[allow(clippy::too_many_arguments)]
fn render_bearer_section(
    input_id: &'static str,
    bearer_input: &Entity<InputState>,
    env_state: &Entity<EnvironmentState>,
    modal_state: &Entity<ModalState>,
    compact: bool,
    on_save: SaveAuthCallback,
    t: &AppTheme,
    window: &mut Window,
    cx: &mut App,
) -> Div {
    form_row("Token:", compact, t).child(div().flex_1().child(render_env_input(
        input_id,
        bearer_input,
        Some(env_state),
        Some(modal_state),
        window,
        cx,
        move |new_token, _window, cx| {
            on_save(Auth::Bearer { token: new_token }, cx);
        },
    )))
}

#[allow(clippy::too_many_arguments)]
fn render_basic_section(
    user_input_id: &'static str,
    pass_input_id: &'static str,
    basic_user_input: &Entity<InputState>,
    basic_pass_input: &Entity<InputState>,
    env_state: &Entity<EnvironmentState>,
    modal_state: &Entity<ModalState>,
    compact: bool,
    on_save: SaveAuthCallback,
    t: &AppTheme,
    window: &mut Window,
    cx: &mut App,
) -> Div {
    let pass_for_user = basic_pass_input.clone();
    let user_for_pass = basic_user_input.clone();
    let save_u = on_save.clone();
    let save_p = on_save;

    let col = div().w_full().flex().flex_col();
    let col = if compact { col } else { col.gap(px(8.)) };

    col.child(
        form_row("Username:", compact, t).child(div().flex_1().child(render_env_input(
            user_input_id,
            basic_user_input,
            Some(env_state),
            Some(modal_state),
            window,
            cx,
            move |new_user, _window, cx| {
                let pass = pass_for_user.read(cx).value().to_string();
                save_u(
                    Auth::Basic {
                        username: new_user,
                        password: pass,
                    },
                    cx,
                );
            },
        ))),
    )
    .child(
        form_row("Password:", compact, t).child(div().flex_1().child(render_env_input(
            pass_input_id,
            basic_pass_input,
            Some(env_state),
            Some(modal_state),
            window,
            cx,
            move |new_pass, _window, cx| {
                let user = user_for_pass.read(cx).value().to_string();
                save_p(
                    Auth::Basic {
                        username: user,
                        password: new_pass,
                    },
                    cx,
                );
            },
        ))),
    )
}

#[allow(clippy::too_many_arguments)]
fn render_apikey_section(
    add_to: ApiKeyLocation,
    ids: &AuthFormIds,
    apikey_key_input: &Entity<InputState>,
    apikey_val_input: &Entity<InputState>,
    env_state: &Entity<EnvironmentState>,
    modal_state: &Entity<ModalState>,
    compact: bool,
    on_save: SaveAuthCallback,
    t: &AppTheme,
    window: &mut Window,
    cx: &mut App,
) -> Div {
    let val_for_key = apikey_val_input.clone();
    let key_for_val = apikey_key_input.clone();
    let key_for_loc = apikey_key_input.clone();
    let val_for_loc = apikey_val_input.clone();

    let save_k = on_save.clone();
    let save_v = on_save.clone();
    let save_loc = on_save;

    let loc_label = if add_to.is_header() {
        "Header"
    } else {
        "Query Params"
    };

    let col = div().w_full().flex().flex_col();
    let col = if compact { col } else { col.gap(px(8.)) };

    col.child(
        form_row("Key:", compact, t).child(div().flex_1().child(render_env_input(
            ids.apikey_key,
            apikey_key_input,
            Some(env_state),
            Some(modal_state),
            window,
            cx,
            move |new_key, _window, cx| {
                let val = val_for_key.read(cx).value().to_string();
                save_k(
                    Auth::ApiKey {
                        key: new_key,
                        value: val,
                        add_to,
                    },
                    cx,
                );
            },
        ))),
    )
    .child(
        form_row("Value:", compact, t).child(div().flex_1().child(render_env_input(
            ids.apikey_val,
            apikey_val_input,
            Some(env_state),
            Some(modal_state),
            window,
            cx,
            move |new_val, _window, cx| {
                let key = key_for_val.read(cx).value().to_string();
                save_v(
                    Auth::ApiKey {
                        key,
                        value: new_val,
                        add_to,
                    },
                    cx,
                );
            },
        ))),
    )
    .child(
        form_row("Add to:", compact, t).child(
            Button::new(ids.apikey_location_dropdown)
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
                                .child(loc_label),
                        )
                        .child(
                            Icon::new(IconName::ChevronDown)
                                .size(px(12.))
                                .text_color(t.text_muted),
                        ),
                )
                .dropdown_menu(move |menu, _window, _cx| {
                    let k_h = key_for_loc.clone();
                    let v_h = val_for_loc.clone();
                    let save_h = save_loc.clone();

                    let k_q = key_for_loc.clone();
                    let v_q = val_for_loc.clone();
                    let save_q = save_loc.clone();

                    menu.item(
                        PopupMenuItem::new("Header")
                            .checked(add_to.is_header())
                            .on_click(move |_, _, cx| {
                                let key = k_h.read(cx).value().to_string();
                                let value = v_h.read(cx).value().to_string();
                                save_h(
                                    Auth::ApiKey {
                                        key,
                                        value,
                                        add_to: ApiKeyLocation::Header,
                                    },
                                    cx,
                                );
                            }),
                    )
                    .item(
                        PopupMenuItem::new("Query Params")
                            .checked(add_to.is_query())
                            .on_click(move |_, _, cx| {
                                let key = k_q.read(cx).value().to_string();
                                let value = v_q.read(cx).value().to_string();
                                save_q(
                                    Auth::ApiKey {
                                        key,
                                        value,
                                        add_to: ApiKeyLocation::Query,
                                    },
                                    cx,
                                );
                            }),
                    )
                }),
        ),
    )
}

/// Read-only mirror of a local auth form row: same label/input layout, disabled env input.
fn disabled_field(
    input_id: &'static str,
    label: &str,
    value: &str,
    env_state: &Entity<EnvironmentState>,
    modal_state: &Entity<ModalState>,
    t: &AppTheme,
    cx: &App,
) -> Div {
    form_row(label, true, t).child(
        div()
            .flex_1()
            .min_w(px(0.))
            .child(render_disabled_env_input(
                input_id,
                value,
                Some(env_state),
                Some(modal_state),
                cx,
            )),
    )
}

/// Disabled mirror of the local auth forms, shown under the inherit description so the
/// user sees the effective inherited configuration (secrets masked, env vars tokenized).
fn render_inherited_auth_fields(
    auth: &Auth,
    env_state: &Entity<EnvironmentState>,
    modal_state: &Entity<ModalState>,
    t: &AppTheme,
    cx: &App,
) -> Div {
    match auth {
        Auth::Bearer { token } => disabled_field(
            "auth-inherit-bearer-token",
            "Token:",
            &mask_secret(token),
            env_state,
            modal_state,
            t,
            cx,
        ),
        Auth::Basic { username, password } => div()
            .w_full()
            .flex()
            .flex_col()
            .child(disabled_field(
                "auth-inherit-basic-user",
                "Username:",
                username,
                env_state,
                modal_state,
                t,
                cx,
            ))
            .child(disabled_field(
                "auth-inherit-basic-pass",
                "Password:",
                &mask_secret(password),
                env_state,
                modal_state,
                t,
                cx,
            )),
        Auth::ApiKey { key, value, add_to } => div()
            .w_full()
            .flex()
            .flex_col()
            .child(disabled_field(
                "auth-inherit-apikey-key",
                "Key:",
                key,
                env_state,
                modal_state,
                t,
                cx,
            ))
            .child(disabled_field(
                "auth-inherit-apikey-val",
                "Value:",
                &mask_secret(value),
                env_state,
                modal_state,
                t,
                cx,
            ))
            .child(disabled_field(
                "auth-inherit-apikey-loc",
                "Add to:",
                if add_to.is_header() {
                    "Header"
                } else {
                    "Query Params"
                },
                env_state,
                modal_state,
                t,
                cx,
            )),
        // None adds no fields: the description banner already states no auth applies.
        // Inherit is unreachable: `inherited_auth_source` never reports `Inherit`.
        Auth::None | Auth::Inherit => div(),
    }
}
