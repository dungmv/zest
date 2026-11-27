use super::{form_row, SaveAuthCallback};
use crate::theme::AppTheme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::Icon;
use gpui_kit::{div, px, Div, ParentElement, Styled};
use zest_core::models::{ApiKeyLocation, Auth};

pub(super) fn render_type_row(
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
