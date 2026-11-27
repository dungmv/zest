use super::ResponseEvent;
use gpui_kit::Context;
use std::sync::Arc;
use zest_core::cache::ResponseCache;
use zest_core::grpc::GrpcCallSummary;
use zest_core::models::{
    ConsoleLogEntry, ConsoleLogLevel, HttpResponseRecord, ResponsePreviewKind, TestResult,
};

#[path = "response_format.rs"]
mod format;
use format::build_cached_format;
#[cfg(test)]
use format::{decode_bounded_utf8_prefix, CachedResponseFormat, MAX_OVERSIZED_PREVIEW_BYTES};
pub use format::{
    ConsoleFilter, FormattedResponseBody, ImageZoomMode, ResponseTabKind, ResponseViewMode,
    MAX_PREVIEW_IMAGE_BYTES,
};
use zest_core::transport::TransportFrame;
use zest_core::ws::WebSocketStatus;

/// View-side cap for the live WS frame log (service keeps its own copy).
const MAX_VIEW_FRAMES: usize = 1000;
/// View-side cap for script console logs within a single run (a runaway
/// `console.log` loop must not grow the panel without bound).
const MAX_CONSOLE_LOGS: usize = 1000;

use zest_core::storage::models::RequestExampleFile;
use zest_core::storage::WorkspaceStore;

/// Transient UI popover/filter/zoom flags for the response inspector.
#[derive(Default)]
pub struct ResponseUiFlags {
    /// Zoom preset for the image preview canvas (reset on response switch).
    pub image_zoom: ImageZoomMode,
    /// Active JSONPath query for the response Body tab (empty = no filter).
    pub jsonpath_query: String,
    /// Whether the JSONPath filter bar below the response toolbar is visible.
    pub show_jsonpath_bar: bool,
    pub show_examples_dropdown: bool,
    pub show_timing_popover: bool,
}

/// Realtime protocol (WebSocket / gRPC / SSE) session state mirrored from engines.
pub struct RealtimeResponseState {
    /// Live WebSocket connection status for the active request.
    pub ws_status: WebSocketStatus,
    /// Live frame log for the active request (mirrored from the WS/gRPC
    /// engines; shared by both realtime protocols).
    pub frames: Vec<TransportFrame>,
    /// Virtualized-list state backing the Messages tab (variable-height
    /// cards, tail-following while streaming).
    pub frames_list: gpui_kit::ListState,
    /// Handshake response headers of the active WS session.
    pub ws_handshake_headers: Vec<(String, String)>,
    /// Summary of the most recent gRPC call (status, headers, duration).
    pub grpc_last_call: Option<GrpcCallSummary>,
}

impl RealtimeResponseState {
    fn new() -> Self {
        let frames_list =
            gpui_kit::ListState::new(0, gpui_kit::ListAlignment::Top, gpui_kit::px(600.));
        // Auto-scroll along the tail while streaming, as long as the user
        // is scrolled to the bottom.
        frames_list.set_follow_mode(gpui_kit::FollowMode::Tail);
        Self {
            ws_status: WebSocketStatus::default(),
            frames: Vec::new(),
            frames_list,
            ws_handshake_headers: Vec::new(),
            grpc_last_call: None,
        }
    }
}

pub struct ResponseState {
    pub current_response: Option<Arc<HttpResponseRecord>>,
    pub cache: ResponseCache,
    pub active_example_name: Option<String>,
    pub active_tab: ResponseTabKind,
    pub view_mode: ResponseViewMode,
    pub ui: ResponseUiFlags,
    pub test_results: Vec<TestResult>,
    pub console_logs: Vec<ConsoleLogEntry>,
    /// Virtualized-list state backing the Console panel (tests then logs).
    pub console_list: gpui_kit::ListState,
    pub console_filter: ConsoleFilter,
    pub formatted_body: Option<FormattedResponseBody>,
    pub scroll_handle: gpui_kit::UniformListScrollHandle,
    pub wrap_lines: bool,
    pub realtime: RealtimeResponseState,
}

impl ResponseState {
    pub fn new() -> Self {
        let console_list =
            gpui_kit::ListState::new(0, gpui_kit::ListAlignment::Top, gpui_kit::px(600.));
        console_list.set_follow_mode(gpui_kit::FollowMode::Tail);
        Self {
            current_response: None,
            cache: ResponseCache::default(),
            active_example_name: None,
            active_tab: ResponseTabKind::Response,
            view_mode: ResponseViewMode::Pretty,
            ui: ResponseUiFlags::default(),
            test_results: Vec::new(),
            console_logs: Vec::new(),
            console_list,
            console_filter: ConsoleFilter::All,
            formatted_body: None,
            scroll_handle: gpui_kit::UniformListScrollHandle::new(),
            wrap_lines: false,
            realtime: RealtimeResponseState::new(),
        }
    }

    pub fn load_for_request_internal(&mut self, req_id: &str, store: &dyn WorkspaceStore) {
        self.active_example_name = None;
        if let Some(cached) = self.cache.get(&req_id.to_string()).cloned() {
            self.test_results = cached.test_results.clone();
            self.console_logs = cached.console_logs.clone();
            self.current_response = Some(cached);
        } else if let Ok(Some(loc)) = store.load_response_cache(req_id) {
            let record = Arc::new(loc.to_record(req_id));
            self.test_results = record.test_results.clone();
            self.console_logs = record.console_logs.clone();
            self.cache.insert(req_id.to_string(), record.clone());
            self.current_response = Some(record);
        } else {
            self.test_results.clear();
            self.console_logs.clear();
            self.current_response = None;
        }
        self.ui.jsonpath_query.clear();
        self.scroll_handle = gpui_kit::UniformListScrollHandle::new();
        self.refresh_formatted_body();
        self.reconcile_view_mode_for_response();
    }

    pub fn load_for_request(
        &mut self,
        req_id: &str,
        store: &dyn WorkspaceStore,
        cx: &mut Context<Self>,
    ) {
        self.load_for_request_internal(req_id, store);
        cx.emit(ResponseEvent::ContentChanged {
            sync_sent_preview: true,
        });
        cx.notify();
    }

    pub fn start_sending_internal(&mut self, req_id: Option<&str>) {
        if let Some(id) = req_id {
            self.cache.remove(&id.to_string());
        }
        self.current_response = None;
        self.formatted_body = None;
        self.test_results.clear();
        self.ui.show_timing_popover = false;
        self.active_example_name = None;
        self.scroll_handle = gpui_kit::UniformListScrollHandle::new();
        self.reset_realtime();
    }

    /// Clears the realtime protocol state (frame log, WS handshake headers,
    /// gRPC call summary) so a fresh send starts from a clean view. The WS
    /// status is deliberately left alone: the Runner sets `Connecting`
    /// around the same moment for WebSocket requests.
    fn reset_realtime(&mut self) {
        self.realtime.frames.clear();
        self.realtime.frames_list.reset(0);
        self.realtime.ws_handshake_headers.clear();
        self.realtime.grpc_last_call = None;
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
        cx: &mut Context<Self>,
    ) {
        self.active_example_name = None;
        self.test_results = response.test_results.clone();
        self.console_logs = response.console_logs.clone();

        self.cache.insert(req_id, response.clone());
        self.current_response = Some(response);
        self.ui.jsonpath_query.clear();
        self.scroll_handle = gpui_kit::UniformListScrollHandle::new();
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
        self.ui.jsonpath_query.clear();
        self.scroll_handle = gpui_kit::UniformListScrollHandle::new();
        self.refresh_formatted_body();
        self.reconcile_view_mode_for_response();
    }

    pub fn restore_live_response(&mut self, req_id: &str, cx: &mut Context<Self>) {
        self.active_example_name = None;
        if let Some(resp) = self.cache.get(&req_id.to_string()).cloned() {
            self.current_response = Some(resp);
        }
        self.ui.jsonpath_query.clear();
        self.scroll_handle = gpui_kit::UniformListScrollHandle::new();
        self.refresh_formatted_body();
        self.reconcile_view_mode_for_response();
        cx.emit(ResponseEvent::ContentChanged {
            sync_sent_preview: true,
        });
        cx.notify();
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

    pub fn refresh_formatted_body(&mut self) {
        if let Some(resp) = &self.current_response {
            let resp_ptr = Arc::as_ptr(resp) as usize;

            let cached_format = match &self.formatted_body {
                Some(current) => match &current.cached_format {
                    Some(cf) if cf.response_ptr == resp_ptr => cf.clone(),
                    _ => Arc::new(build_cached_format(resp)),
                },
                None => Arc::new(build_cached_format(resp)),
            };

            let (mut display_text, mut lines) =
                if matches!(self.view_mode, ResponseViewMode::Pretty) {
                    (
                        cached_format.pretty_text.clone(),
                        cached_format.pretty_lines.clone(),
                    )
                } else {
                    (
                        cached_format.raw_text.clone(),
                        cached_format.raw_lines.clone(),
                    )
                };

            // Apply the JSONPath query. The result is always pretty JSON
            // because the query runs on parsed data rather than the raw text
            // view; incomplete or invalid expressions silently keep the base
            // display so typing (e.g. `$.`) is never disrupted.
            let mut match_count = None;
            if !self.ui.jsonpath_query.is_empty() {
                if let Some(parsed) = &cached_format.parsed_json {
                    if let Ok(result) =
                        zest_core::json_path::run_json_path(parsed, &self.ui.jsonpath_query)
                    {
                        match_count = Some(result.match_count);
                        display_text = Arc::new(result.result_text);
                        lines = Arc::new(result.lines);
                    }
                }
            }

            self.formatted_body = Some(FormattedResponseBody {
                display_text,
                lines,
                is_json: cached_format.is_json,
                is_xml: cached_format.is_xml,
                is_binary: cached_format.is_binary,
                is_truncated: cached_format.is_truncated,
                preview_kind: cached_format.preview_kind,
                preview_image: cached_format.preview_image.clone(),
                image_dimensions: cached_format.image_dimensions,
                match_count,
                cached_format: Some(cached_format),
            });
        } else {
            self.formatted_body = None;
        }
    }

    /// Reconciles `view_mode` and resets `image_zoom` whenever the active
    /// response changes:
    /// - Image responses auto-switch to `Preview`.
    /// - Non-previewable responses fall back from `Preview` to `Pretty`.
    pub(crate) fn reconcile_view_mode_for_response(&mut self) {
        self.ui.image_zoom = ImageZoomMode::Fit;
        let preview_kind = self.formatted_body.as_ref().and_then(|f| f.preview_kind);
        match preview_kind {
            Some(ResponsePreviewKind::Image(_)) => {
                self.view_mode = ResponseViewMode::Preview;
            }
            None => {
                if self.view_mode == ResponseViewMode::Preview {
                    self.view_mode = ResponseViewMode::Pretty;
                }
            }
        }
    }

    pub fn set_response(
        &mut self,
        response: Option<Arc<HttpResponseRecord>>,
        cx: &mut Context<Self>,
    ) {
        if let Some(r) = &response {
            self.test_results = r.test_results.clone();
            self.console_logs = r.console_logs.clone();
        }
        self.current_response = response;
        self.ui.jsonpath_query.clear();
        self.scroll_handle = gpui_kit::UniformListScrollHandle::new();
        self.refresh_formatted_body();
        self.reconcile_view_mode_for_response();
        cx.emit(ResponseEvent::ContentChanged {
            sync_sent_preview: true,
        });
        cx.notify();
    }

    pub fn set_active_tab(&mut self, tab: ResponseTabKind, cx: &mut Context<Self>) {
        self.active_tab = tab;
        cx.notify();
    }

    pub fn set_view_mode(&mut self, mode: ResponseViewMode, cx: &mut Context<Self>) {
        if self.view_mode == mode {
            return;
        }
        self.view_mode = mode;
        self.refresh_formatted_body();
        cx.emit(ResponseEvent::ContentChanged {
            sync_sent_preview: false,
        });
        cx.notify();
    }

    pub fn set_image_zoom(&mut self, zoom: ImageZoomMode, cx: &mut Context<Self>) {
        if self.ui.image_zoom == zoom {
            return;
        }
        self.ui.image_zoom = zoom;
        cx.notify();
    }

    pub fn zoom_in_image(&mut self, cx: &mut Context<Self>) {
        let next = self.ui.image_zoom.zoom_in();
        self.set_image_zoom(next, cx);
    }

    pub fn zoom_out_image(&mut self, cx: &mut Context<Self>) {
        let prev = self.ui.image_zoom.zoom_out();
        self.set_image_zoom(prev, cx);
    }

    pub fn toggle_examples_dropdown(&mut self, cx: &mut Context<Self>) {
        self.ui.show_examples_dropdown = !self.ui.show_examples_dropdown;
        cx.notify();
    }

    pub fn toggle_timing_popover(&mut self, cx: &mut Context<Self>) {
        self.ui.show_timing_popover = !self.ui.show_timing_popover;
        cx.notify();
    }

    pub fn toggle_wrap_lines(&mut self, cx: &mut Context<Self>) {
        self.wrap_lines = !self.wrap_lines;
        cx.notify();
    }

    /// Updates the JSONPath query used to filter the response Body tab.
    ///
    /// Re-emits `ContentChanged` so the shared response editor re-syncs with
    /// the filtered display text through the standard event channel.
    pub fn set_jsonpath_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.ui.jsonpath_query = query;
        self.refresh_formatted_body();
        cx.emit(ResponseEvent::ContentChanged {
            sync_sent_preview: false,
        });
        cx.notify();
    }

    /// Toggles the JSONPath filter bar below the response toolbar. Closing the
    /// bar clears any active query so the full response body is restored.
    /// Returns the new visibility state.
    pub fn toggle_jsonpath_bar(&mut self, cx: &mut Context<Self>) -> bool {
        if self.ui.show_jsonpath_bar {
            self.close_jsonpath_bar(cx);
            false
        } else {
            self.ui.show_jsonpath_bar = true;
            cx.notify();
            true
        }
    }

    /// Closes the JSONPath filter bar and clears any active query.
    pub fn close_jsonpath_bar(&mut self, cx: &mut Context<Self>) {
        self.ui.show_jsonpath_bar = false;
        if !self.ui.jsonpath_query.is_empty() {
            self.ui.jsonpath_query.clear();
            self.refresh_formatted_body();
            cx.emit(ResponseEvent::ContentChanged {
                sync_sent_preview: false,
            });
        }
        cx.notify();
    }

    pub fn set_console_filter(&mut self, filter: ConsoleFilter, cx: &mut Context<Self>) {
        self.console_filter = filter;
        cx.notify();
    }

    pub fn clear_console_logs(&mut self, cx: &mut Context<Self>) {
        self.test_results.clear();
        self.console_logs.clear();
        self.sync_console_list();
        cx.notify();
    }

    pub fn set_pre_request_logs(&mut self, logs: Vec<ConsoleLogEntry>, cx: &mut Context<Self>) {
        self.console_logs = self.cap_console_logs(logs);
        self.sync_console_list();
        cx.notify();
    }

    /// Caps the log vec to [`MAX_CONSOLE_LOGS`], keeping the most recent
    /// entries (chronological order preserved, oldest dropped).
    fn cap_console_logs(&self, mut logs: Vec<ConsoleLogEntry>) -> Vec<ConsoleLogEntry> {
        if logs.len() > MAX_CONSOLE_LOGS {
            let overflow = logs.len() - MAX_CONSOLE_LOGS;
            logs.drain(..overflow);
        }
        logs
    }

    /// Rebuilds the Console panel's virtual list state after the tests/logs
    /// vecs change.
    fn sync_console_list(&mut self) {
        self.console_list
            .reset(self.test_results.len() + self.console_logs.len());
    }

    // --- WebSocket session view ------------------------------------------

    pub fn set_ws_status(&mut self, status: WebSocketStatus, cx: &mut Context<Self>) {
        self.realtime.ws_status = status;
        cx.notify();
    }

    pub fn push_frame(&mut self, message: TransportFrame, cx: &mut Context<Self>) {
        let old_count = self.realtime.frames.len();
        self.realtime.frames.push(message);
        if self.realtime.frames.len() > MAX_VIEW_FRAMES {
            let overflow = self.realtime.frames.len() - MAX_VIEW_FRAMES;
            self.realtime.frames.drain(..overflow);
            // Every index shifted: rebuild the virtual list state.
            self.realtime.frames_list.reset(MAX_VIEW_FRAMES);
        } else {
            self.realtime.frames_list.splice(old_count..old_count, 1);
        }
        cx.notify();
    }

    pub fn set_grpc_last_call(&mut self, summary: Option<GrpcCallSummary>, cx: &mut Context<Self>) {
        self.realtime.grpc_last_call = summary;
        cx.notify();
    }

    pub fn set_ws_handshake_headers(
        &mut self,
        headers: Vec<(String, String)>,
        cx: &mut Context<Self>,
    ) {
        self.realtime.ws_handshake_headers = headers;
        cx.notify();
    }

    /// Restores the realtime view when switching back to a request that has
    /// a live (or previously failed) connection / frame history in an
    /// engine. Does not touch `grpc_last_call`; callers restore it per
    /// protocol from the engine snapshot.
    pub fn restore_realtime_session(
        &mut self,
        status: WebSocketStatus,
        messages: Vec<TransportFrame>,
        handshake_headers: Vec<(String, String)>,
        cx: &mut Context<Self>,
    ) {
        self.realtime.ws_status = status;
        self.realtime.frames = messages;
        self.realtime.frames_list.reset(self.realtime.frames.len());
        self.realtime.ws_handshake_headers = handshake_headers;
        self.scroll_handle = gpui_kit::UniformListScrollHandle::new();
        cx.notify();
    }

    pub fn set_test_results(
        &mut self,
        results: Vec<TestResult>,
        logs: Vec<ConsoleLogEntry>,
        cx: &mut Context<Self>,
    ) {
        self.test_results = results;
        self.console_logs = self.cap_console_logs(logs);
        self.sync_console_list();
        cx.notify();
    }

    pub fn clear_test_results(&mut self, cx: &mut Context<Self>) {
        self.test_results.clear();
        self.console_logs.clear();
        self.sync_console_list();
        cx.notify();
    }

    pub fn filtered_test_results(&self) -> Vec<&TestResult> {
        self.test_results
            .iter()
            .filter(|res| match self.console_filter {
                ConsoleFilter::All | ConsoleFilter::Tests => true,
                ConsoleFilter::Logs => false,
                ConsoleFilter::Errors => !res.passed,
            })
            .collect()
    }

    pub fn filtered_console_logs(&self) -> Vec<&ConsoleLogEntry> {
        self.console_logs
            .iter()
            .filter(|entry| match self.console_filter {
                ConsoleFilter::All | ConsoleFilter::Logs => true,
                ConsoleFilter::Tests => false,
                ConsoleFilter::Errors => entry.level == ConsoleLogLevel::Error,
            })
            .collect()
    }
}

#[cfg(test)]
impl ResponseState {
    fn build_cached_format(resp: &Arc<HttpResponseRecord>) -> CachedResponseFormat {
        build_cached_format(resp)
    }
}

#[cfg(test)]
#[path = "response_state_tests.rs"]
mod tests;
