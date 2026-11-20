//! Thin helpers around `gpui-kit`'s built-in `Notification` layer.
//!
//! - Use `*_in` variants inside UI callbacks where `&mut Window` is already in scope
//!   (avoids re-entrant window borrows).
//! - Use `info` / `success` / `error` from async tasks and background services that
//!   only hold `&mut App`.

use gpui_kit::component::notification::{Notification, NotificationType};
use gpui_kit::component::WindowExt as _;
use gpui_kit::{App, SharedString, Window};

fn build(
    kind: NotificationType,
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
) -> Notification {
    let message: SharedString = message.into();
    let note = Notification::new().with_type(kind).title(title);
    if message.is_empty() {
        note
    } else {
        note.message(message)
    }
}

/// Push a notification to the active window (or the first open window) when no
/// `&mut Window` handle is available (e.g., from async background services).
///
/// Scheduled on the foreground executor so it never collides with an active
/// `&mut Window` borrow on the call stack.
pub fn push(note: Notification, cx: &mut App) {
    cx.spawn(async move |cx| {
        cx.update(|cx| {
            let Some(handle) = cx.active_window().or_else(|| cx.windows().first().copied()) else {
                return;
            };
            let _ = handle.update(cx, |_, window, cx| {
                window.push_notification(note, cx);
            });
        });
    })
    .detach();
}

pub fn info(title: impl Into<SharedString>, message: impl Into<SharedString>, cx: &mut App) {
    push(build(NotificationType::Info, title, message), cx);
}

pub fn success(title: impl Into<SharedString>, message: impl Into<SharedString>, cx: &mut App) {
    push(build(NotificationType::Success, title, message), cx);
}

pub fn error(title: impl Into<SharedString>, message: impl Into<SharedString>, cx: &mut App) {
    push(build(NotificationType::Error, title, message), cx);
}

pub fn success_in(
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut App,
) {
    window.push_notification(build(NotificationType::Success, title, message), cx);
}

pub fn error_in(
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut App,
) {
    window.push_notification(build(NotificationType::Error, title, message), cx);
}
