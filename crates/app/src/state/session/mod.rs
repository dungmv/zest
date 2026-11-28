//! Active request selection and composer session state.

mod send;
#[cfg(test)]
mod tests;

pub use crate::components::request_composer::tabs::RequestTabKind;
pub(crate) use send::SendId;

use super::{ComposerSyncScope, SessionEvent};
use crate::services::GrpcSchemaSummary;
use gpui_kit::{Context, Task};
use send::RequestSend;
use zest_core::graphql::GraphQLSchemaSummary;

#[derive(Default)]
struct BinaryFileInfo {
    /// Cached path and formatted size for the active binary request body.
    size: Option<(String, String)>,
    task: Option<Task<()>>,
}

pub struct RequestSessionState {
    pub active_request_id: Option<String>,
    pub active_tab_kind: RequestTabKind,
    send: RequestSend,
    send_id: u64,
    pub script_error: Option<String>,
    /// Resolved gRPC schema for the service/method picker (session-only).
    pub grpc_schema: Option<std::sync::Arc<GrpcSchemaSummary>>,
    /// Resolved GraphQL schema for the completion provider (session-only).
    pub graphql_schema: Option<GraphQLSchemaSummary>,
    binary_file: BinaryFileInfo,
    input_refresh_task: Option<Task<()>>,
}

impl RequestSessionState {
    pub fn new() -> Self {
        Self {
            active_request_id: Some("scratch".to_string()),
            active_tab_kind: RequestTabKind::Body,
            send: RequestSend::default(),
            send_id: 0,
            script_error: None,
            grpc_schema: None,
            graphql_schema: None,
            binary_file: BinaryFileInfo::default(),
            input_refresh_task: None,
        }
    }

    /// Returns the cached size label for `path` without touching the filesystem.
    pub fn binary_file_size_label(&self, path: &str) -> &str {
        if path.is_empty() {
            return "";
        }
        match &self.binary_file.size {
            Some((cached_path, label)) if cached_path == path => label.as_str(),
            _ => "Checking...",
        }
    }

    /// Queries `std::fs::metadata` off the UI thread and caches the formatted
    /// size for the active binary payload path.
    pub fn refresh_binary_file_size(&mut self, path: String, cx: &mut Context<Self>) {
        if path.is_empty() {
            self.binary_file.size = None;
            self.binary_file.task = None;
            cx.notify();
            return;
        }
        let path_for_bg = path.clone();
        let task = cx.spawn(async move |this, cx| {
            let formatted = cx
                .background_executor()
                .spawn(async move {
                    match std::fs::metadata(&path_for_bg) {
                        Ok(meta) => {
                            crate::components::body_editor::binary::format_byte_size(meta.len())
                        }
                        Err(_) => "File not found".to_string(),
                    }
                })
                .await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |state, cx| {
                    state.binary_file.size = Some((path, formatted));
                    cx.notify();
                });
            }
        });
        self.binary_file.task = Some(task);
    }

    pub fn active_request_id(&self) -> &str {
        self.active_request_id.as_deref().unwrap_or("scratch")
    }

    pub fn set_active_request_id(&mut self, req_id: &str, cx: &mut Context<Self>) {
        if self.active_request_id.as_deref() == Some(req_id) {
            return;
        }
        self.active_request_id = Some(req_id.to_string());
        cx.emit(SessionEvent::ActiveRequestChanged {
            req_id: req_id.to_string(),
        });
        cx.notify();
    }

    pub fn set_active_tab_kind(&mut self, kind: RequestTabKind, cx: &mut Context<Self>) {
        self.active_tab_kind = kind;
        cx.notify();
    }

    pub fn schedule_input_refresh(&mut self, cx: &mut Context<Self>) {
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(150))
                .await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |_, cx| cx.notify());
            }
        });
        self.input_refresh_task = Some(task);
    }

    pub fn mark_dirty(&mut self, cx: &mut Context<Self>) {
        let req_id = self.active_request_id().to_string();
        cx.emit(SessionEvent::RequestModified {
            req_id,
            sync_inputs: None,
        });
        cx.notify();
    }

    pub fn notify_request_reloaded(&mut self, cx: &mut Context<Self>) {
        let req_id = self.active_request_id().to_string();
        cx.emit(SessionEvent::RequestModified {
            req_id,
            sync_inputs: Some(ComposerSyncScope::All),
        });
        cx.notify();
    }

    pub fn notify_body_changed(&mut self, cx: &mut Context<Self>) {
        let req_id = self.active_request_id().to_string();
        cx.emit(SessionEvent::RequestModified {
            req_id,
            sync_inputs: Some(ComposerSyncScope::BodyOnly),
        });
        cx.notify();
    }

    pub fn sync_composer_inputs(&mut self, scope: ComposerSyncScope, cx: &mut Context<Self>) {
        cx.emit(SessionEvent::SyncComposerInputs { scope });
        cx.notify();
    }

    pub(crate) fn reset_workspace(&mut self, cx: &mut Context<Self>) {
        self.cancel_send();
        let send_id = self.send_id;
        *self = Self::new();
        self.send_id = send_id;
        self.active_request_id = None;
        cx.notify();
    }

    pub fn set_grpc_schema(
        &mut self,
        schema: Option<std::sync::Arc<GrpcSchemaSummary>>,
        cx: &mut Context<Self>,
    ) {
        self.grpc_schema = schema;
        cx.notify();
    }

    /// Resolved GraphQL schema for the deep autocomplete provider
    /// (session-only, like the gRPC one).
    pub fn set_graphql_schema(
        &mut self,
        schema: Option<GraphQLSchemaSummary>,
        cx: &mut Context<Self>,
    ) {
        self.graphql_schema = schema;
        cx.notify();
    }

    pub fn set_script_error(&mut self, err: Option<String>, cx: &mut Context<Self>) {
        self.script_error = err;
        cx.notify();
    }
}
