//! Active response and its cached, formatted, and realtime data.

use super::ResponseEvent;
use gpui_kit::{Context, Task};
use std::sync::Arc;
use zest_core::cache::ResponseCache;
use zest_core::models::HttpResponseRecord;

mod console;
mod realtime;
mod view;

#[cfg(test)]
use crate::components::response_inspector::format::CachedResponseFormat;
pub use crate::components::response_inspector::format::{
    ConsoleFilter, ImageZoomMode, ResponseTabKind, ResponseViewMode, MAX_PREVIEW_IMAGE_BYTES,
};
use crate::components::response_inspector::view::ResponseView;
use console::ResponseConsole;
use realtime::RealtimeResponseState;
#[cfg(test)]
use zest_core::models::{ConsoleLogEntry, ConsoleLogLevel, TestResult};

use zest_core::storage::models::RequestExampleFile;
use zest_core::storage::WorkspaceStore;

pub struct ResponseState {
    pub current_response: Option<Arc<HttpResponseRecord>>,
    pub cache: ResponseCache,
    pub active_example_name: Option<String>,
    view: ResponseView,
    console: ResponseConsole,
    realtime: RealtimeResponseState,
    sidecar_load: Option<Task<()>>,
}

impl ResponseState {
    pub fn new() -> Self {
        Self {
            current_response: None,
            cache: ResponseCache::default(),
            active_example_name: None,
            view: ResponseView::default(),
            console: ResponseConsole::default(),
            realtime: RealtimeResponseState::new(),
            sidecar_load: None,
        }
    }

    pub fn view(&self) -> &ResponseView {
        &self.view
    }

    pub fn console(&self) -> &ResponseConsole {
        &self.console
    }

    pub fn realtime(&self) -> &RealtimeResponseState {
        &self.realtime
    }

    pub(crate) fn reset_workspace(&mut self, cx: &mut Context<Self>) {
        *self = Self {
            view: self.view.reset_workspace(),
            ..Self::new()
        };
        cx.emit(ResponseEvent::ContentChanged {
            sync_sent_preview: true,
        });
        cx.notify();
    }

    fn apply_loaded_response(&mut self, response: Option<Arc<HttpResponseRecord>>) {
        self.active_example_name = None;
        self.current_response = response;
        if let Some(response) = self.current_response.clone() {
            self.console.load_output(&response);
        } else {
            self.console.clear();
        }
        self.view.reset_body();
        self.refresh_formatted_body();
        self.reconcile_view_mode_for_response();
    }

    pub fn load_for_request(
        &mut self,
        req_id: &str,
        store: Arc<dyn WorkspaceStore>,
        cx: &mut Context<Self>,
    ) {
        self.sidecar_load = None;
        let id = req_id.to_string();
        let cached = self.cache.get(&id).cloned();
        let read_sidecar = cached.is_none() && !matches!(req_id, "" | "scratch");
        self.apply_loaded_response(cached);
        self.notify_loaded_response(cx);
        if read_sidecar {
            self.sidecar_load = Some(cx.spawn(async move |this, cx| {
                let id_for_read = id.clone();
                let record = cx
                    .background_executor()
                    .spawn(async move {
                        store
                            .load_response_cache(&id_for_read)
                            .ok()
                            .flatten()
                            .map(|loc| Arc::new(loc.to_record(&id_for_read)))
                    })
                    .await;
                if let Some(record) = record {
                    let _ = this.update(cx, |state, cx| {
                        state.cache.insert(id, record.clone());
                        if state.active_example_name.is_none() {
                            state.apply_loaded_response(Some(record));
                            state.notify_loaded_response(cx);
                        }
                    });
                }
            }));
        }
    }

    fn notify_loaded_response(&self, cx: &mut Context<Self>) {
        cx.emit(ResponseEvent::ContentChanged {
            sync_sent_preview: true,
        });
        cx.notify();
    }

    pub fn start_sending_internal(&mut self, req_id: Option<&str>) {
        self.sidecar_load = None;
        if let Some(id) = req_id {
            self.cache.remove(&id.to_string());
        }
        self.current_response = None;
        self.view.formatted_body = None;
        self.console.clear_results();
        self.view.show_timing_popover = false;
        self.active_example_name = None;
        self.view.scroll_handle = gpui_kit::UniformListScrollHandle::new();
        self.realtime.reset();
    }

    pub fn start_sending(&mut self, req_id: Option<&str>, cx: &mut Context<Self>) {
        self.start_sending_internal(req_id);
        cx.emit(ResponseEvent::ContentChanged {
            sync_sent_preview: true,
        });
        cx.notify();
    }

    pub fn set_response_for_request(
        &mut self,
        req_id: String,
        response: Arc<HttpResponseRecord>,
        active: bool,
        cx: &mut Context<Self>,
    ) {
        self.cache.insert(req_id, response.clone());
        if !active || self.active_example_name.is_some() {
            return;
        }
        self.sidecar_load = None;
        self.active_example_name = None;
        self.console.load_output(&response);

        self.current_response = Some(response);
        self.view.reset_body();
        self.refresh_formatted_body();
        self.reconcile_view_mode_for_response();
        cx.emit(ResponseEvent::ContentChanged {
            sync_sent_preview: true,
        });
        cx.notify();
    }

    pub fn rename_request(&mut self, old_id: &str, new_id: &str) {
        self.cache.rename(&old_id.to_string(), new_id.to_string());
    }

    pub fn view_example(&mut self, example: &RequestExampleFile, cx: &mut Context<Self>) {
        self.view_example_internal(example);
        cx.emit(ResponseEvent::ContentChanged {
            sync_sent_preview: true,
        });
        cx.notify();
    }

    pub fn view_example_internal(&mut self, example: &RequestExampleFile) {
        self.active_example_name = Some(example.name.clone());
        let headers: Vec<(String, String)> = example
            .headers
            .iter()
            .map(|h| (h.key.clone(), h.value.clone()))
            .collect();
        let body = example.body.as_deref().unwrap_or("").as_bytes().to_vec();
        let size_bytes = body.len();
        let status_text = zest_core::models::status_code_to_reason(example.status).to_string();
        let record = HttpResponseRecord {
            id: format!("example-{}", example.name),
            request_id: String::new(),
            status_code: example.status,
            status_text,
            headers,
            cookies: Vec::new(),
            body,
            duration_ms: 0,
            size_bytes,
            timing: Default::default(),
            request_sent: None,
            test_results: Vec::new(),
            console_logs: Vec::new(),
            created_at: chrono::Utc::now(),
        };
        self.current_response = Some(Arc::new(record));
        self.view.reset_body();
        self.refresh_formatted_body();
        self.reconcile_view_mode_for_response();
    }

    pub fn restore_live_response(&mut self, req_id: &str, cx: &mut Context<Self>) {
        let response = self.cache.get(&req_id.to_string()).cloned();
        self.apply_loaded_response(response);
        self.notify_loaded_response(cx);
    }

    pub fn rename_example(&mut self, old_name: &str, new_name: &str, cx: &mut Context<Self>) {
        self.rename_example_internal(old_name, new_name);
        cx.notify();
    }

    pub fn rename_example_internal(&mut self, old_name: &str, new_name: &str) {
        if self.active_example_name.as_deref() == Some(old_name) {
            self.active_example_name = Some(new_name.to_string());
        }
    }

    pub fn set_response(
        &mut self,
        response: Option<Arc<HttpResponseRecord>>,
        cx: &mut Context<Self>,
    ) {
        self.sidecar_load = None;
        if let Some(r) = &response {
            self.console.load_output(r);
        }
        self.current_response = response;
        self.view.reset_body();
        self.refresh_formatted_body();
        self.reconcile_view_mode_for_response();
        cx.emit(ResponseEvent::ContentChanged {
            sync_sent_preview: true,
        });
        cx.notify();
    }
}

#[cfg(test)]
impl ResponseState {
    fn build_cached_format(resp: &Arc<HttpResponseRecord>) -> CachedResponseFormat {
        crate::components::response_inspector::format::build_cached_format(resp)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod preview_tests;
