use gpui_kit::Context;
use std::sync::Arc;
use std::time::Duration;

pub const DEFAULT_INFO_DURATION: Duration = Duration::from_millis(4000);
pub const DEFAULT_SUCCESS_DURATION: Duration = Duration::from_millis(4000);
pub const DEFAULT_ERROR_DURATION: Duration = Duration::from_millis(5000);
pub const MAX_VISIBLE_TOASTS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

#[derive(Clone)]
pub struct ToastItem {
    pub id: String,
    pub kind: ToastKind,
    pub title: String,
    pub message: String,
    pub action_label: Option<String>,
    pub action_fn: Option<Arc<dyn Fn() + Send + Sync>>,
    pub duration: Option<Duration>,
}

impl ToastItem {
    pub fn new(kind: ToastKind, title: impl Into<String>, msg: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            title: title.into(),
            message: msg.into(),
            action_label: None,
            action_fn: None,
            duration: None,
        }
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn with_action(
        mut self,
        label: impl Into<String>,
        action: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        self.action_label = Some(label.into());
        self.action_fn = Some(Arc::new(action));
        self
    }
}

pub struct ToastState {
    pub toasts: Vec<ToastItem>,
}

impl Default for ToastState {
    fn default() -> Self {
        Self::new()
    }
}

impl ToastState {
    pub fn new() -> Self {
        Self { toasts: Vec::new() }
    }

    pub fn push_toast(&mut self, toast: ToastItem, cx: &mut Context<Self>) {
        let toast_id = toast.id.clone();
        let duration = toast.duration.unwrap_or(match toast.kind {
            ToastKind::Info => DEFAULT_INFO_DURATION,
            ToastKind::Success => DEFAULT_SUCCESS_DURATION,
            ToastKind::Error => DEFAULT_ERROR_DURATION,
        });

        self.toasts.push(toast);
        if self.toasts.len() > MAX_VISIBLE_TOASTS {
            self.toasts.remove(0);
        }
        cx.notify();

        if !duration.is_zero() {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(duration).await;
                if let Some(this) = this.upgrade() {
                    this.update(cx, |state, cx| {
                        state.dismiss_toast(&toast_id, cx);
                    });
                }
            })
            .detach();
        }
    }

    pub fn push_info(
        &mut self,
        title: impl Into<String>,
        msg: impl Into<String>,
        cx: &mut Context<Self>,
    ) {
        self.push_toast(ToastItem::new(ToastKind::Info, title, msg), cx);
    }

    pub fn push_error(
        &mut self,
        title: impl Into<String>,
        msg: impl Into<String>,
        cx: &mut Context<Self>,
    ) {
        self.push_toast(ToastItem::new(ToastKind::Error, title, msg), cx);
    }

    pub fn push_success(
        &mut self,
        title: impl Into<String>,
        msg: impl Into<String>,
        cx: &mut Context<Self>,
    ) {
        self.push_toast(ToastItem::new(ToastKind::Success, title, msg), cx);
    }

    pub fn dismiss_toast(&mut self, id: &str, cx: &mut Context<Self>) {
        let prev_len = self.toasts.len();
        self.toasts.retain(|t| t.id != id);
        if self.toasts.len() != prev_len {
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{AppContext, TestAppContext};

    #[gpui_kit::test]
    fn test_push_and_manual_dismiss(cx: &mut TestAppContext) {
        let toast_entity = cx.new(|_| ToastState::new());

        toast_entity.update(cx, |state, cx| {
            state.push_info("Test Info", "Info message", cx);
            state.push_error("Test Error", "Error message", cx);
        });

        toast_entity.read_with(cx, |state, _| {
            assert_eq!(state.toasts.len(), 2);
            assert_eq!(state.toasts[0].title, "Test Info");
            assert_eq!(state.toasts[1].title, "Test Error");
        });

        let id_to_dismiss = toast_entity.read_with(cx, |state, _| state.toasts[0].id.clone());

        toast_entity.update(cx, |state, cx| {
            state.dismiss_toast(&id_to_dismiss, cx);
        });

        toast_entity.read_with(cx, |state, _| {
            assert_eq!(state.toasts.len(), 1);
            assert_eq!(state.toasts[0].title, "Test Error");
        });
    }

    #[gpui_kit::test]
    fn test_toast_max_visible_limit(cx: &mut TestAppContext) {
        let toast_entity = cx.new(|_| ToastState::new());

        toast_entity.update(cx, |state, cx| {
            for i in 0..8 {
                state.push_info(format!("Title {i}"), format!("Msg {i}"), cx);
            }
        });

        toast_entity.read_with(cx, |state, _| {
            assert_eq!(state.toasts.len(), MAX_VISIBLE_TOASTS);
            assert_eq!(state.toasts[0].title, "Title 3");
            assert_eq!(state.toasts[MAX_VISIBLE_TOASTS - 1].title, "Title 7");
        });
    }

    #[gpui_kit::test]
    fn test_toast_auto_dismissal(cx: &mut TestAppContext) {
        let toast_entity = cx.new(|_| ToastState::new());

        toast_entity.update(cx, |state, cx| {
            let toast = ToastItem::new(ToastKind::Info, "Auto Dismiss", "Will vanish")
                .with_duration(Duration::from_millis(50));
            state.push_toast(toast, cx);
        });

        toast_entity.read_with(cx, |state, _| {
            assert_eq!(state.toasts.len(), 1);
        });

        // Advance simulated clock past the toast duration
        cx.background_executor
            .advance_clock(Duration::from_millis(60));
        cx.run_until_parked();

        toast_entity.read_with(cx, |state, _| {
            assert_eq!(state.toasts.len(), 0);
        });
    }
}
