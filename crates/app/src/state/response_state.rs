use super::ResponseEvent;
use gpui_kit::Context;
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
    pub scroll_handle: gpui_kit::UniformListScrollHandle,
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
            scroll_handle: gpui_kit::UniformListScrollHandle::new(),
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
        self.scroll_handle = gpui_kit::UniformListScrollHandle::new();
        self.refresh_formatted_body();
    }

    pub fn restore_live_response(&mut self, req_id: &str, cx: &mut Context<Self>) {
        self.active_example_name = None;
        if let Some(resp) = self.cache.get(&req_id.to_string()).cloned() {
            self.current_response = Some(resp);
        }
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
#[path = "response_state_tests.rs"]
mod tests;
