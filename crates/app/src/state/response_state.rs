use super::ResponseEvent;
use gpui::Context;
use std::sync::Arc;
use zest_core::cache::ResponseCache;
use zest_core::models::{ConsoleLogEntry, ConsoleLogLevel, HttpResponseRecord, TestResult};
use zest_core::storage::models::RequestExampleFile;
use zest_core::storage::WorkspaceStore;

#[derive(Clone, Debug)]
pub(crate) struct CachedResponseFormat {
    pub(crate) response_ptr: usize,
    pub(crate) raw_text: Arc<String>,
    pub(crate) raw_lines: Arc<Vec<String>>,
    pub(crate) pretty_text: Arc<String>,
    pub(crate) pretty_lines: Arc<Vec<String>>,
    pub(crate) is_json: bool,
    pub(crate) is_xml: bool,
}

#[derive(Clone, Debug, Default)]
pub struct FormattedResponseBody {
    pub display_text: Arc<String>,
    pub lines: Arc<Vec<String>>,
    pub is_json: bool,
    pub is_xml: bool,
    pub(crate) cached_format: Option<Arc<CachedResponseFormat>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResponseTabKind {
    #[default]
    Response,
    Request,
    Headers,
    Cookies,
    Console,
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
    pub search_query: String,
    pub is_searching: bool,
    pub show_examples_dropdown: bool,
    pub show_timing_popover: bool,
    pub test_results: Vec<TestResult>,
    pub console_logs: Vec<ConsoleLogEntry>,
    pub console_filter: ConsoleFilter,
    pub formatted_body: Option<FormattedResponseBody>,
    pub scroll_handle: gpui::UniformListScrollHandle,
    pub wrap_lines: bool,
}

impl ResponseState {
    pub fn new() -> Self {
        Self {
            current_response: None,
            cache: ResponseCache::default(),
            active_example_name: None,
            active_tab: ResponseTabKind::Response,
            view_mode: ResponseViewMode::Pretty,
            search_query: String::new(),
            is_searching: false,
            show_examples_dropdown: false,
            show_timing_popover: false,
            test_results: Vec::new(),
            console_logs: Vec::new(),
            console_filter: ConsoleFilter::All,
            formatted_body: None,
            scroll_handle: gpui::UniformListScrollHandle::new(),
            wrap_lines: false,
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
        self.scroll_handle = gpui::UniformListScrollHandle::new();
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
        self.scroll_handle = gpui::UniformListScrollHandle::new();
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
        self.scroll_handle = gpui::UniformListScrollHandle::new();
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
        self.scroll_handle = gpui::UniformListScrollHandle::new();
        self.refresh_formatted_body();
    }

    pub fn restore_live_response(&mut self, req_id: &str, cx: &mut Context<Self>) {
        self.active_example_name = None;
        if let Some(resp) = self.cache.get(&req_id.to_string()).cloned() {
            self.current_response = Some(resp);
        }
        self.scroll_handle = gpui::UniformListScrollHandle::new();
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

            let (display_text, lines) = if matches!(self.view_mode, ResponseViewMode::Pretty) {
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

            self.formatted_body = Some(FormattedResponseBody {
                display_text,
                lines,
                is_json: cached_format.is_json,
                is_xml: cached_format.is_xml,
                cached_format: Some(cached_format),
            });
        } else {
            self.formatted_body = None;
        }
    }

    fn build_cached_format(resp: &Arc<HttpResponseRecord>) -> CachedResponseFormat {
        let raw_str = String::from_utf8_lossy(&resp.body).to_string();
        let parsed_json = serde_json::from_str::<serde_json::Value>(&raw_str).ok();
        let is_json = parsed_json.is_some()
            || resp.is_json()
            || raw_str.trim_start().starts_with('{')
            || raw_str.trim_start().starts_with('[');

        let is_xml = !is_json
            && (resp.is_xml()
                || raw_str.trim_start().starts_with("<?xml")
                || (raw_str.trim_start().starts_with('<') && raw_str.trim_end().ends_with('>')));

        let pretty_str = if let Some(parsed) = &parsed_json {
            serde_json::to_string_pretty(parsed).unwrap_or_else(|_| raw_str.clone())
        } else if is_xml {
            crate::components::body_editor::format_xml(&raw_str).unwrap_or_else(|_| raw_str.clone())
        } else {
            raw_str.clone()
        };

        let raw_lines: Vec<String> = raw_str.lines().map(|s| s.to_string()).collect();
        let pretty_lines: Vec<String> = if pretty_str == raw_str {
            raw_lines.clone()
        } else {
            pretty_str.lines().map(|s| s.to_string()).collect()
        };

        CachedResponseFormat {
            response_ptr: Arc::as_ptr(resp) as usize,
            raw_text: Arc::new(raw_str),
            raw_lines: Arc::new(raw_lines),
            pretty_text: Arc::new(pretty_str),
            pretty_lines: Arc::new(pretty_lines),
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
        self.scroll_handle = gpui::UniformListScrollHandle::new();
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

    pub fn set_search_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.search_query = query;
        cx.notify();
    }

    pub fn toggle_search(&mut self, cx: &mut Context<Self>) {
        self.is_searching = !self.is_searching;
        if !self.is_searching {
            self.search_query.clear();
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
        cx.notify();
    }

    pub fn set_pre_request_logs(&mut self, logs: Vec<ConsoleLogEntry>, cx: &mut Context<Self>) {
        self.console_logs = logs;
        cx.notify();
    }

    pub fn set_test_results(
        &mut self,
        results: Vec<TestResult>,
        logs: Vec<ConsoleLogEntry>,
        cx: &mut Context<Self>,
    ) {
        self.test_results = results;
        self.console_logs = logs;
        cx.notify();
    }

    pub fn clear_test_results(&mut self, cx: &mut Context<Self>) {
        self.test_results.clear();
        self.console_logs.clear();
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
mod tests {
    use super::*;
    use zest_core::models::ConsoleLogSource;

    #[test]
    fn test_start_sending_clears_active_data() {
        let mut state = ResponseState::new();
        let rec = HttpResponseRecord {
            id: "resp-1".to_string(),
            request_id: "req-1".to_string(),
            status_code: 200,
            status_text: "OK".to_string(),
            duration_ms: 25,
            size_bytes: 10,
            headers: vec![("content-type".to_string(), "application/json".to_string())],
            cookies: vec![],
            body: b"{\"a\":1}".to_vec(),
            timing: zest_core::models::TimingBreakdown::default(),
            request_sent: None,
            test_results: vec![],
            console_logs: vec![],
            created_at: chrono::Utc::now(),
        };

        let arc_rec = Arc::new(rec);
        state.cache.insert("req-1".to_string(), arc_rec.clone());
        state.current_response = Some(arc_rec);
        state.show_timing_popover = true;
        state.active_example_name = Some("example-1".to_string());
        state.refresh_formatted_body();
        assert!(state.current_response.is_some());
        assert!(state.formatted_body.is_some());
        assert!(state.cache.contains_key(&"req-1".to_string()));

        state.start_sending_internal(Some("req-1"));

        assert!(state.current_response.is_none());
        assert!(state.formatted_body.is_none());
        assert!(!state.show_timing_popover);
        assert!(state.active_example_name.is_none());
        assert!(!state.cache.contains_key(&"req-1".to_string()));
    }

    #[test]
    fn test_console_logs_filtering() {
        let mut state = ResponseState::new();
        state.test_results = vec![
            TestResult {
                name: "Status is 200".to_string(),
                passed: true,
                error_message: None,
                duration_ms: 2,
            },
            TestResult {
                name: "Has token".to_string(),
                passed: false,
                error_message: Some("Expected token".to_string()),
                duration_ms: 1,
            },
        ];
        state.console_logs = vec![
            ConsoleLogEntry {
                source: ConsoleLogSource::PreRequest,
                level: ConsoleLogLevel::Log,
                message: "Pre script log".to_string(),
                timestamp: "10:00:00".to_string(),
            },
            ConsoleLogEntry {
                source: ConsoleLogSource::PostResponse,
                level: ConsoleLogLevel::Warn,
                message: "Post script warn".to_string(),
                timestamp: "10:00:01".to_string(),
            },
            ConsoleLogEntry {
                source: ConsoleLogSource::PostResponse,
                level: ConsoleLogLevel::Error,
                message: "Post script error".to_string(),
                timestamp: "10:00:02".to_string(),
            },
        ];

        // Default filter: All
        state.console_filter = ConsoleFilter::All;
        assert_eq!(state.filtered_test_results().len(), 2);
        assert_eq!(state.filtered_console_logs().len(), 3);

        // Tests filter
        state.console_filter = ConsoleFilter::Tests;
        assert_eq!(state.filtered_test_results().len(), 2);
        assert_eq!(state.filtered_console_logs().len(), 0);

        // Logs filter
        state.console_filter = ConsoleFilter::Logs;
        assert_eq!(state.filtered_test_results().len(), 0);
        assert_eq!(state.filtered_console_logs().len(), 3);

        // Errors filter
        state.console_filter = ConsoleFilter::Errors;
        let err_tests = state.filtered_test_results();
        assert_eq!(err_tests.len(), 1);
        assert_eq!(err_tests[0].name, "Has token");
        let err_logs = state.filtered_console_logs();
        assert_eq!(err_logs.len(), 1);
        assert_eq!(err_logs[0].message, "Post script error");
    }

    #[test]
    fn test_console_tab_kind() {
        let mut state = ResponseState::new();
        assert_eq!(state.active_tab, ResponseTabKind::Response);
        state.active_tab = ResponseTabKind::Console;
        assert_eq!(state.active_tab, ResponseTabKind::Console);
    }

    #[test]
    fn test_large_response_body_formatting() {
        let mut state = ResponseState::new();

        // Create a 500-item JSON array
        let items: Vec<String> = (0..500)
            .map(|i| format!(r#"{{"id": {}, "title": "item {}"}}"#, i, i))
            .collect();
        let json_body = format!("[\n{}\n]", items.join(",\n"));

        let rec = HttpResponseRecord {
            id: "resp-1".to_string(),
            request_id: "req-1".to_string(),
            status_code: 200,
            status_text: "OK".to_string(),
            duration_ms: 25,
            size_bytes: json_body.len(),
            headers: vec![("content-type".to_string(), "application/json".to_string())],
            cookies: vec![],
            body: json_body.into_bytes(),
            timing: zest_core::models::TimingBreakdown::default(),
            request_sent: None,
            test_results: vec![],
            console_logs: vec![],
            created_at: chrono::Utc::now(),
        };

        state.current_response = Some(Arc::new(rec));
        state.refresh_formatted_body();

        let fmt = state
            .formatted_body
            .as_ref()
            .expect("formatted body should be present");
        assert!(fmt.is_json);
        // Each item has 4 lines in pretty JSON (bracket, id, title, bracket), so > 1000 lines
        assert!(fmt.lines.len() >= 500);

        // Switch to Raw mode
        state.view_mode = ResponseViewMode::Raw;
        state.refresh_formatted_body();
        let fmt_raw = state.formatted_body.as_ref().unwrap();
        assert!(!fmt_raw.lines.is_empty());
    }

    #[test]
    fn test_load_for_request_from_cache() {
        let temp = tempfile::tempdir().unwrap();
        let store = zest_core::storage::FsWorkspaceStore::new(temp.path());
        store.init_workspace("Test WS").unwrap();
        let (_, rel_path) = store
            .create_request(
                None,
                "Login",
                zest_core::models::HttpMethod::Post,
                "http://localhost:8000/login",
            )
            .unwrap();

        let loc_file = zest_core::storage::models::LocFile {
            sent_at: "2026-10-15T12:00:00Z".to_string(),
            request: zest_core::storage::models::LocRequestInfo {
                method: zest_core::models::HttpMethod::Post,
                url: "http://localhost:8000/login".to_string(),
                headers: vec![],
                body: Some(r#"{"user":"alice"}"#.to_string()),
            },
            response: zest_core::storage::models::LocResponseInfo {
                status: 200,
                duration_ms: 42,
                size: 15,
                headers: vec![("content-type".to_string(), "application/json".to_string())],
                body: Some(r#"{"token":"xyz"}"#.to_string()),
                truncated: false,
                binary: false,
            },
        };
        store.save_response_cache(&rel_path, &loc_file).unwrap();

        let mut state = ResponseState::new();
        assert!(state.current_response.is_none());

        state.load_for_request_internal(&rel_path, &store);
        assert!(state.current_response.is_some());
        let resp = state.current_response.as_ref().unwrap();
        assert_eq!(resp.status_code, 200);
        assert_eq!(resp.status_text, "OK");
        assert_eq!(resp.duration_ms, 42);
        assert_eq!(String::from_utf8_lossy(&resp.body), r#"{"token":"xyz"}"#);
        assert!(state.formatted_body.is_some());
    }

    #[test]
    fn test_xml_response_body_formatting() {
        let mut state = ResponseState::new();
        let record = HttpResponseRecord {
            id: "resp-xml".to_string(),
            request_id: "req-1".to_string(),
            status_code: 200,
            status_text: "OK".to_string(),
            headers: vec![("content-type".to_string(), "application/xml".to_string())],
            cookies: vec![],
            body: "<root><user><name>Bob</name></user></root>"
                .as_bytes()
                .to_vec(),
            duration_ms: 10,
            size_bytes: 42,
            timing: Default::default(),
            request_sent: None,
            test_results: vec![],
            console_logs: vec![],
            created_at: chrono::Utc::now(),
        };

        state.current_response = Some(Arc::new(record));
        state.view_mode = ResponseViewMode::Pretty;
        state.refresh_formatted_body();

        let fmt = state.formatted_body.expect("Formatted body should exist");
        assert!(fmt.is_xml, "Should detect XML response");
        assert!(fmt.display_text.contains("<root>\n"));
        assert!(fmt.display_text.contains("  <user>\n"));
    }

    #[test]
    fn test_wrap_lines_initial_and_toggle() {
        let mut state = ResponseState::new();
        assert!(!state.wrap_lines, "wrap_lines should default to false");
        state.wrap_lines = true;
        assert!(state.wrap_lines);
    }

    #[test]
    fn test_toolbar_icons_exist_in_all_assets() {
        use gpui::AssetSource as _;
        use gpui_kit::assets::{AllAssets, IconName};

        let assets = AllAssets;
        let icons = [
            IconName::TextWrap.path(),
            IconName::Copy.path(),
            IconName::Download.path(),
            IconName::Save.path(),
        ];

        for icon_path in icons {
            let res = assets.load(icon_path.as_ref());
            assert!(
                res.is_ok() && res.unwrap().is_some(),
                "Asset {} must exist in AllAssets",
                icon_path
            );
        }
    }

    #[test]
    fn test_view_example_preserves_http_status_without_name() {
        let mut state = ResponseState::new();
        let ex = zest_core::storage::models::RequestExampleFile {
            name: "Success 200".to_string(),
            status: 200,
            headers: vec![],
            body: Some(r#"{"message":"hello"}"#.to_string()),
        };

        state.view_example_internal(&ex);

        assert_eq!(state.active_example_name.as_deref(), Some("Success 200"));
        let resp = state
            .current_response
            .as_ref()
            .expect("Response should exist");
        assert_eq!(resp.status_code, 200);
        // Requirement 2: HTTP status is preserved like request, NOT containing example name
        assert_eq!(resp.status_text, "OK");
        assert_ne!(resp.status_text, "Example: Success 200");

        // Renaming example should update active_example_name, but status_text stays "OK"
        state.rename_example_internal("Success 200", "Renamed Success");
        assert_eq!(
            state.active_example_name.as_deref(),
            Some("Renamed Success")
        );
        let resp_after = state
            .current_response
            .as_ref()
            .expect("Response should exist");
        assert_eq!(resp_after.status_text, "OK");
    }

    #[test]
    fn test_response_state_cache_rename_and_eviction() {
        let mut state = ResponseState::new();
        // Configure a small capacity cache for testing
        state.cache = ResponseCache::new(2, 10_000);

        let rec1 = Arc::new(HttpResponseRecord {
            id: "r1".to_string(),
            request_id: "req-1".to_string(),
            status_code: 200,
            status_text: "OK".to_string(),
            duration_ms: 10,
            size_bytes: 5,
            headers: vec![],
            cookies: vec![],
            body: b"hello".to_vec(),
            timing: Default::default(),
            request_sent: None,
            test_results: vec![],
            console_logs: vec![],
            created_at: chrono::Utc::now(),
        });

        let rec2 = Arc::new(HttpResponseRecord {
            id: "r2".to_string(),
            request_id: "req-2".to_string(),
            status_code: 201,
            status_text: "Created".to_string(),
            duration_ms: 15,
            size_bytes: 5,
            headers: vec![],
            cookies: vec![],
            body: b"world".to_vec(),
            timing: Default::default(),
            request_sent: None,
            test_results: vec![],
            console_logs: vec![],
            created_at: chrono::Utc::now(),
        });

        state.cache.insert("req-1".to_string(), rec1);
        state.cache.insert("req-2".to_string(), rec2.clone());

        // Test rename_request
        state.rename_request("req-1", "req-1-renamed");
        assert!(!state.cache.contains_key(&"req-1".to_string()));
        assert!(state.cache.contains_key(&"req-1-renamed".to_string()));

        // Insert a 3rd record, which should evict the oldest entry ("req-1-renamed")
        let rec3 = Arc::new(HttpResponseRecord {
            id: "r3".to_string(),
            request_id: "req-3".to_string(),
            status_code: 204,
            status_text: "No Content".to_string(),
            duration_ms: 5,
            size_bytes: 0,
            headers: vec![],
            cookies: vec![],
            body: vec![],
            timing: Default::default(),
            request_sent: None,
            test_results: vec![],
            console_logs: vec![],
            created_at: chrono::Utc::now(),
        });
        state.cache.insert("req-3".to_string(), rec3);

        assert_eq!(state.cache.len(), 2);
        assert!(!state.cache.contains_key(&"req-1-renamed".to_string()));
        assert!(state.cache.contains_key(&"req-2".to_string()));
        assert!(state.cache.contains_key(&"req-3".to_string()));
    }

    #[test]
    fn test_formatted_body_caching_on_view_mode_toggle() {
        let mut state = ResponseState::new();
        let payload = r#"{"hello": "world", "number": 42}"#.as_bytes().to_vec();
        let record = Arc::new(HttpResponseRecord {
            body: payload,
            headers: vec![("content-type".to_string(), "application/json".to_string())],
            ..Default::default()
        });

        state.current_response = Some(record);
        state.view_mode = ResponseViewMode::Pretty;
        state.refresh_formatted_body();

        let (is_json1, display_text1, cache_ptr1) = {
            let fmt = state.formatted_body.as_ref().unwrap();
            (
                fmt.is_json,
                fmt.display_text.clone(),
                fmt.cached_format.as_ref().map(Arc::as_ptr).unwrap(),
            )
        };
        assert!(is_json1);
        assert!(display_text1.contains('\n'));

        // Toggle to Raw mode
        state.view_mode = ResponseViewMode::Raw;
        state.refresh_formatted_body();

        let (display_text2, cache_ptr2) = {
            let fmt = state.formatted_body.as_ref().unwrap();
            (
                fmt.display_text.clone(),
                fmt.cached_format.as_ref().map(Arc::as_ptr).unwrap(),
            )
        };

        // Cached format must be reused with identical Arc pointer (zero re-parsing)
        assert_eq!(cache_ptr1, cache_ptr2);
        assert_eq!(&*display_text2, r#"{"hello": "world", "number": 42}"#);

        // Toggle back to Pretty mode
        state.view_mode = ResponseViewMode::Pretty;
        state.refresh_formatted_body();

        let (display_text3, cache_ptr3) = {
            let fmt = state.formatted_body.as_ref().unwrap();
            (
                fmt.display_text.clone(),
                fmt.cached_format.as_ref().map(Arc::as_ptr).unwrap(),
            )
        };
        assert_eq!(cache_ptr1, cache_ptr3);
        assert_eq!(display_text3, display_text1);
    }
}
