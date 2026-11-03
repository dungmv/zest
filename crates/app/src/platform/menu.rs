//! Native application menu (macOS menubar).
//!
//! Builds the standard "Zest" application menu (About, Settings, Services,
//! Hide/Show, Quit) like native macOS apps. Menu actions are registered as
//! global (app-level) handlers so they work even when no element inside the
//! window holds focus.

use crate::app::{HideApp, HideOtherApps, OpenAbout, OpenSettings, Quit, ShowAllApps};
use crate::state::ModalState;
use gpui_kit::*;

const APP_NAME: &str = "Zest";

/// Registers menu keybindings, global action handlers, and installs the
/// application menu bar.
pub fn init_app_menu(modal: Entity<ModalState>, cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-h", HideApp, None),
        KeyBinding::new("alt-cmd-h", HideOtherApps, None),
    ]);

    cx.on_action({
        let modal = modal.clone();
        move |_: &OpenAbout, cx| {
            modal.update(cx, |state, cx| state.open_about_modal(cx));
            cx.activate(true);
        }
    });
    cx.on_action(move |_: &OpenSettings, cx| {
        modal.update(cx, |state, cx| state.open_settings_modal(cx));
        cx.activate(true);
    });
    cx.on_action(|_: &HideApp, cx| cx.hide());
    cx.on_action(|_: &HideOtherApps, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAllApps, cx| cx.unhide_other_apps());
    cx.on_action(|_: &Quit, cx| cx.quit());

    cx.set_menus([Menu::new(APP_NAME).items([
        MenuItem::action(format!("About {APP_NAME}"), OpenAbout),
        MenuItem::separator(),
        MenuItem::action("Settings…", OpenSettings),
        MenuItem::separator(),
        MenuItem::os_submenu("Services", SystemMenuType::Services),
        MenuItem::separator(),
        MenuItem::action(format!("Hide {APP_NAME}"), HideApp),
        MenuItem::action("Hide Others", HideOtherApps),
        MenuItem::action("Show All", ShowAllApps),
        MenuItem::separator(),
        MenuItem::action(format!("Quit {APP_NAME}"), Quit),
    ])]);
}
