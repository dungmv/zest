use gpui_kit::Context;
use std::sync::Arc;

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
}

pub struct ToastState {
    pub toasts: Vec<ToastItem>,
}

impl ToastState {
    pub fn new() -> Self {
        Self { toasts: Vec::new() }
    }

    pub fn push_toast(&mut self, toast: ToastItem, cx: &mut Context<Self>) {
        self.toasts.push(toast);
        cx.notify();
    }

    pub fn push_info(
        &mut self,
        title: impl Into<String>,
        msg: impl Into<String>,
        cx: &mut Context<Self>,
    ) {
        self.push_toast(
            ToastItem {
                id: uuid::Uuid::new_v4().to_string(),
                kind: ToastKind::Info,
                title: title.into(),
                message: msg.into(),
                action_label: None,
                action_fn: None,
            },
            cx,
        );
    }

    pub fn push_error(
        &mut self,
        title: impl Into<String>,
        msg: impl Into<String>,
        cx: &mut Context<Self>,
    ) {
        self.push_toast(
            ToastItem {
                id: uuid::Uuid::new_v4().to_string(),
                kind: ToastKind::Error,
                title: title.into(),
                message: msg.into(),
                action_label: None,
                action_fn: None,
            },
            cx,
        );
    }

    pub fn push_success(
        &mut self,
        title: impl Into<String>,
        msg: impl Into<String>,
        cx: &mut Context<Self>,
    ) {
        self.push_toast(
            ToastItem {
                id: uuid::Uuid::new_v4().to_string(),
                kind: ToastKind::Success,
                title: title.into(),
                message: msg.into(),
                action_label: None,
                action_fn: None,
            },
            cx,
        );
    }

    pub fn dismiss_toast(&mut self, id: &str, cx: &mut Context<Self>) {
        self.toasts.retain(|t| t.id != id);
        cx.notify();
    }
}
