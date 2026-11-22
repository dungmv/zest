use super::ResponseEvent;
use gpui_kit::Context;
use std::sync::Arc;
use zest_core::cache::ResponseCache;
use zest_core::grpc::GrpcCallSummary;
use zest_core::models::Protocol;
use zest_core::models::{ConsoleLogEntry, ConsoleLogLevel, HttpResponseRecord, TestResult};
use zest_core::transport::TransportFrame;
use zest_core::ws::WebSocketStatus;

/// View-side cap for the live WS frame log (service keeps its own copy).
const MAX_VIEW_FRAMES: usize = 1000;
/// Bodies above this size skip JSON/XML pretty-printing entirely: parsing
/// and re-serializing multi-megabyte payloads runs on the UI thread on every
/// response arrival and request switch.
const MAX_FORMAT_BODY_BYTES: usize = 2 * 1024 * 1024;
/// Maximum number of lines materialized for the (virtualized) body view;
/// payloads beyond the cap end with a truncation marker line.
const MAX_FORMAT_LINES: usize = 2000;
/// Maximum characters rendered per line: single-line megabyte bodies (e.g.
/// minified JSON) would otherwise become one giant text node per frame.
const MAX_FORMAT_LINE_CHARS: usize = 1000;
/// View-side cap for script console logs within a single run (a runaway
/// `console.log` loop must not grow the panel without bound).
const MAX_CONSOLE_LOGS: usize = 1000;

/// Splits at most [`MAX_FORMAT_LINES`] lines of at most
/// [`MAX_FORMAT_LINE_CHARS`] chars each, appending truncation markers when
/// the payload continues beyond either cap.
fn bounded_lines(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = text
        .lines()
        .take(MAX_FORMAT_LINES)
        .map(|s| {
            if s.chars().count() > MAX_FORMAT_LINE_CHARS {
                let mut cut: String = s.chars().take(MAX_FORMAT_LINE_CHARS).collect();
                cut.push_str("… (line truncated)");
                cut
            } else {
                s.to_string()
            }
        })
        .collect();
    let total = text.lines().count();
    if total > MAX_FORMAT_LINES {
        lines.push(format!("… (truncated, {} lines total)", total));
    }
    lines
}
use zest_core::storage::models::RequestExampleFile;
use zest_core::storage::WorkspaceStore;

#[derive(Clone, Debug)]
pub(crate) struct CachedResponseFormat {
    pub(crate) response_ptr: usize,
    pub(crate) raw_text: Arc<String>,
    pub(crate) raw_lines: Arc<Vec<String>>,
    pub(crate) pretty_text: Arc<String>,
    pub(crate) pretty_lines: Arc<Vec<String>>,
    /// Parsed JSON body, retained for JSONPath query evaluation (kept only
    /// for the active response, so memory stays bounded).
    pub(crate) parsed_json: Option<Arc<serde_json::Value>>,
    pub(crate) is_json: bool,
    pub(crate) is_xml: bool,
}

#[derive(Clone, Debug, Default)]
pub struct FormattedResponseBody {
    pub display_text: Arc<String>,
    pub lines: Arc<Vec<String>>,
    pub is_json: bool,
    pub is_xml: bool,
    /// Number of JSONPath query matches when a query is active, else `None`.
    pub match_count: Option<usize>,
    pub(crate) cached_format: Option<Arc<CachedResponseFormat>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResponseTabKind {
    #[default]
    Response,
    Request,
    Messages,
    Headers,
    Cookies,
    Console,
}

impl ResponseTabKind {
    /// Response-area tab strip for a request of the given protocol:
    /// - HTTP/GraphQL: Response, Request Sent, Headers, Cookies, Console
    /// - SSE: Messages (live event log), Response (accumulated), Headers,
    ///   Cookies, Console
    /// - WebSocket: Messages (frame log), Headers (handshake), Console
    /// - gRPC: Messages (stream frames), Headers, Console
    pub fn tabs_for(protocol: Protocol) -> &'static [ResponseTabKind] {
        match protocol {
            Protocol::Http | Protocol::GraphQL => &[
                ResponseTabKind::Response,
                ResponseTabKind::Request,
                ResponseTabKind::Headers,
                ResponseTabKind::Cookies,
                ResponseTabKind::Console,
            ],
            Protocol::Sse => &[
                ResponseTabKind::Messages,
                ResponseTabKind::Response,
                ResponseTabKind::Headers,
                ResponseTabKind::Cookies,
                ResponseTabKind::Console,
            ],
            Protocol::WebSocket | Protocol::Grpc => &[
                ResponseTabKind::Messages,
                ResponseTabKind::Headers,
                ResponseTabKind::Console,
            ],
        }
    }

    /// Returns `kind` if it is valid for `protocol`, else the first valid tab.
    pub fn clamp_for(kind: ResponseTabKind, protocol: Protocol) -> ResponseTabKind {
        let tabs = Self::tabs_for(protocol);
        if tabs.contains(&kind) {
            kind
        } else {
            tabs[0]
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ResponseTabKind::Response => "Response",
            ResponseTabKind::Request => "Request Sent",
            ResponseTabKind::Messages => "Messages",
            ResponseTabKind::Headers => "Headers",
            ResponseTabKind::Cookies => "Cookies",
            ResponseTabKind::Console => "Console",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConsoleFilter {
    #[default]
    All,
    Tests,
    Logs,
    Errors,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResponseViewMode {
    #[default]
    Pretty,
    Raw,
    Preview,
    Hex,
}

pub struct ResponseState {
    pub current_response: Option<Arc<HttpResponseRecord>>,
    pub cache: ResponseCache,
    pub active_example_name: Option<String>,
    pub active_tab: ResponseTabKind,
    pub view_mode: ResponseViewMode,
    /// Active JSONPath query for the response Body tab (empty = no filter).
    pub jsonpath_query: String,
    /// Whether the JSONPath filter bar below the response toolbar is visible.
    pub show_jsonpath_bar: bool,
    pub show_examples_dropdown: bool,
    pub show_timing_popover: bool,
    pub test_results: Vec<TestResult>,
    pub console_logs: Vec<ConsoleLogEntry>,
    /// Virtualized-list state backing the Console panel (tests then logs).
    pub console_list: gpui_kit::ListState,
    pub console_filter: ConsoleFilter,
    pub formatted_body: Option<FormattedResponseBody>,
    pub scroll_handle: gpui_kit::UniformListScrollHandle,
    pub wrap_lines: bool,
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

impl ResponseState {
    pub fn new() -> Self {
        let frames_list =
            gpui_kit::ListState::new(0, gpui_kit::ListAlignment::Top, gpui_kit::px(600.));
        // Auto-scroll along the tail while streaming, as long as the user
        // is scrolled to the bottom.
        frames_list.set_follow_mode(gpui_kit::FollowMode::Tail);
        let console_list =
            gpui_kit::ListState::new(0, gpui_kit::ListAlignment::Top, gpui_kit::px(600.));
        console_list.set_follow_mode(gpui_kit::FollowMode::Tail);
        Self {
            current_response: None,
            cache: ResponseCache::default(),
            active_example_name: None,
            active_tab: ResponseTabKind::Response,
            view_mode: ResponseViewMode::Pretty,
            jsonpath_query: String::new(),
            show_jsonpath_bar: false,
            show_examples_dropdown: false,
            show_timing_popover: false,
            test_results: Vec::new(),
            console_logs: Vec::new(),
            console_list,
            console_filter: ConsoleFilter::All,
            formatted_body: None,
            scroll_handle: gpui_kit::UniformListScrollHandle::new(),
            wrap_lines: false,
            ws_status: WebSocketStatus::default(),
            frames: Vec::new(),
            frames_list,
            ws_handshake_headers: Vec::new(),
            grpc_last_call: None,
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
        self.jsonpath_query.clear();
        self.scroll_handle = gpui_kit::UniformListScrollHandle::new();
        self.refresh_formatted_body();
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
        self.show_timing_popover = false;
        self.active_example_name = None;
        self.scroll_handle = gpui_kit::UniformListScrollHandle::new();
        self.reset_realtime();
    }

    /// Clears the realtime protocol state (frame log, WS handshake headers,
    /// gRPC call summary) so a fresh send starts from a clean view. The WS
    /// status is deliberately left alone: the Runner sets `Connecting`
    /// around the same moment for WebSocket requests.
    fn reset_realtime(&mut self) {
        self.frames.clear();
        self.frames_list.reset(0);
        self.ws_handshake_headers.clear();
        self.grpc_last_call = None;
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
        self.jsonpath_query.clear();
        self.scroll_handle = gpui_kit::UniformListScrollHandle::new();
        self.refresh_formatted_body();
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
        self.jsonpath_query.clear();
        self.scroll_handle = gpui_kit::UniformListScrollHandle::new();
        self.refresh_formatted_body();
    }

    pub fn restore_live_response(&mut self, req_id: &str, cx: &mut Context<Self>) {
        self.active_example_name = None;
        if let Some(resp) = self.cache.get(&req_id.to_string()).cloned() {
            self.current_response = Some(resp);
        }
        self.jsonpath_query.clear();
        self.scroll_handle = gpui_kit::UniformListScrollHandle::new();
        self.refresh_formatted_body();
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
                    _ => Arc::new(Self::build_cached_format(resp)),
                },
                None => Arc::new(Self::build_cached_format(resp)),
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
            if !self.jsonpath_query.is_empty() {
                if let Some(parsed) = &cached_format.parsed_json {
                    if let Ok(result) =
                        zest_core::json_path::run_json_path(parsed, &self.jsonpath_query)
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
                match_count,
                cached_format: Some(cached_format),
            });
        } else {
            self.formatted_body = None;
        }
    }

    fn build_cached_format(resp: &Arc<HttpResponseRecord>) -> CachedResponseFormat {
        let oversized = resp.body.len() > MAX_FORMAT_BODY_BYTES;
        let raw_str = String::from_utf8_lossy(&resp.body).to_string();
        let parsed_json = if oversized {
            None
        } else {
            serde_json::from_str::<serde_json::Value>(&raw_str).ok()
        };
        let is_json = !oversized
            && (parsed_json.is_some()
                || resp.is_json()
                || raw_str.trim_start().starts_with('{')
                || raw_str.trim_start().starts_with('['));

        let is_xml = !oversized
            && !is_json
            && (resp.is_xml()
                || raw_str.trim_start().starts_with("<?xml")
                || (raw_str.trim_start().starts_with('<') && raw_str.trim_end().ends_with('>')));

        let pretty_str = if oversized {
            raw_str.clone()
        } else if let Some(parsed) = &parsed_json {
            serde_json::to_string_pretty(parsed).unwrap_or_else(|_| raw_str.clone())
        } else if is_xml {
            crate::components::body_editor::format_xml(&raw_str).unwrap_or_else(|_| raw_str.clone())
        } else {
            raw_str.clone()
        };

        let raw_lines: Arc<Vec<String>> = Arc::new(bounded_lines(&raw_str));
        let pretty_lines: Arc<Vec<String>> = if pretty_str == raw_str {
            raw_lines.clone()
        } else {
            Arc::new(bounded_lines(&pretty_str))
        };

        CachedResponseFormat {
            response_ptr: Arc::as_ptr(resp) as usize,
            raw_text: Arc::new(raw_str),
            raw_lines,
            pretty_text: Arc::new(pretty_str),
            pretty_lines,
            parsed_json: parsed_json.map(Arc::new),
            is_json,
            is_xml,
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
        self.jsonpath_query.clear();
        self.scroll_handle = gpui_kit::UniformListScrollHandle::new();
        self.refresh_formatted_body();
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

    pub fn toggle_examples_dropdown(&mut self, cx: &mut Context<Self>) {
        self.show_examples_dropdown = !self.show_examples_dropdown;
        cx.notify();
    }

    pub fn toggle_timing_popover(&mut self, cx: &mut Context<Self>) {
        self.show_timing_popover = !self.show_timing_popover;
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
        self.jsonpath_query = query;
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
        if self.show_jsonpath_bar {
            self.close_jsonpath_bar(cx);
            false
        } else {
            self.show_jsonpath_bar = true;
            cx.notify();
            true
        }
    }

    /// Closes the JSONPath filter bar and clears any active query.
    pub fn close_jsonpath_bar(&mut self, cx: &mut Context<Self>) {
        self.show_jsonpath_bar = false;
        if !self.jsonpath_query.is_empty() {
            self.jsonpath_query.clear();
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
        self.ws_status = status;
        cx.notify();
    }

    pub fn push_frame(&mut self, message: TransportFrame, cx: &mut Context<Self>) {
        let old_count = self.frames.len();
        self.frames.push(message);
        if self.frames.len() > MAX_VIEW_FRAMES {
            let overflow = self.frames.len() - MAX_VIEW_FRAMES;
            self.frames.drain(..overflow);
            // Every index shifted: rebuild the virtual list state.
            self.frames_list.reset(MAX_VIEW_FRAMES);
        } else {
            self.frames_list.splice(old_count..old_count, 1);
        }
        cx.notify();
    }

    pub fn set_grpc_last_call(&mut self, summary: Option<GrpcCallSummary>, cx: &mut Context<Self>) {
        self.grpc_last_call = summary;
        cx.notify();
    }

    pub fn set_ws_handshake_headers(
        &mut self,
        headers: Vec<(String, String)>,
        cx: &mut Context<Self>,
    ) {
        self.ws_handshake_headers = headers;
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
        self.ws_status = status;
        self.frames = messages;
        self.frames_list.reset(self.frames.len());
        self.ws_handshake_headers = handshake_headers;
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
#[path = "response_state_tests.rs"]
mod tests;
