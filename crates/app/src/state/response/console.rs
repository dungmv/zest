use super::{ConsoleFilter, ResponseState};
use gpui_kit::Context;
use zest_core::models::{ConsoleLogEntry, ConsoleLogLevel, HttpResponseRecord, TestResult};

/// View-side cap for script console logs within a single run (a runaway
/// `console.log` loop must not grow the panel without bound).
const MAX_CONSOLE_LOGS: usize = 1000;

pub struct ResponseConsole {
    pub test_results: Vec<TestResult>,
    pub logs: Vec<ConsoleLogEntry>,
    /// Virtualized-list state backing the Console panel (tests then logs).
    pub list: gpui_kit::ListState,
    pub(super) filter: ConsoleFilter,
}

impl Default for ResponseConsole {
    fn default() -> Self {
        let list = gpui_kit::ListState::new(0, gpui_kit::ListAlignment::Top, gpui_kit::px(600.));
        list.set_follow_mode(gpui_kit::FollowMode::Tail);
        Self {
            test_results: Vec::new(),
            logs: Vec::new(),
            list,
            filter: ConsoleFilter::All,
        }
    }
}

impl ResponseConsole {
    pub(super) fn clear(&mut self) {
        self.logs.clear();
        self.clear_results();
    }

    pub(super) fn clear_results(&mut self) {
        self.test_results.clear();
        self.sync_list();
    }

    fn set_logs(&mut self, logs: Vec<ConsoleLogEntry>) {
        self.logs = Self::cap_console_logs(logs);
        self.sync_list();
    }

    /// Caps the log vec to [`MAX_CONSOLE_LOGS`], keeping the most recent
    /// entries (chronological order preserved, oldest dropped).
    fn cap_console_logs(mut logs: Vec<ConsoleLogEntry>) -> Vec<ConsoleLogEntry> {
        if logs.len() > MAX_CONSOLE_LOGS {
            let overflow = logs.len() - MAX_CONSOLE_LOGS;
            logs.drain(..overflow);
        }
        logs
    }

    /// Rebuilds the Console panel's virtual list state after the tests/logs
    /// vecs change.
    pub(super) fn sync_list(&mut self) {
        self.list.reset(self.test_results.len() + self.logs.len());
    }

    pub(super) fn load_output(&mut self, response: &HttpResponseRecord) {
        self.test_results.clone_from(&response.test_results);
        let start = response.console_logs.len().saturating_sub(MAX_CONSOLE_LOGS);
        self.logs = response.console_logs[start..].to_vec();
        self.sync_list();
    }

    pub fn filtered_test_results(&self) -> Vec<&TestResult> {
        self.test_results
            .iter()
            .filter(|res| match self.filter {
                ConsoleFilter::All | ConsoleFilter::Tests => true,
                ConsoleFilter::Logs => false,
                ConsoleFilter::Errors => !res.passed,
            })
            .collect()
    }

    pub fn filtered_console_logs(&self) -> Vec<&ConsoleLogEntry> {
        self.logs
            .iter()
            .filter(|entry| match self.filter {
                ConsoleFilter::All | ConsoleFilter::Logs => true,
                ConsoleFilter::Tests => false,
                ConsoleFilter::Errors => entry.level == ConsoleLogLevel::Error,
            })
            .collect()
    }
}

impl ResponseState {
    pub fn set_console_filter(&mut self, filter: ConsoleFilter, cx: &mut Context<Self>) {
        self.console.filter = filter;
        cx.notify();
    }

    pub fn clear_console_logs(&mut self, cx: &mut Context<Self>) {
        self.console.clear();
        cx.notify();
    }

    pub fn set_pre_request_logs(&mut self, logs: Vec<ConsoleLogEntry>, cx: &mut Context<Self>) {
        self.console.set_logs(logs);
        cx.notify();
    }

    pub fn clear_test_results(&mut self, cx: &mut Context<Self>) {
        self.clear_console_logs(cx);
    }
}
