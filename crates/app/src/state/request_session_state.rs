use super::{ComposerSyncScope, SessionEvent};
use gpui_kit::{Context, Task};
use tokio::sync::oneshot;
use zest_core::models::HttpRequestItem;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RequestTabKind {
    #[default]
    Body,
    Params,
    Headers,
    Auth,
    PreRequestScript,
    Tests,
}

pub struct RequestSessionState {
    pub active_request_id: Option<String>,
    pub active_tab_kind: RequestTabKind,
    pub is_sending: bool,
    pub running_request_id: Option<String>,
    pub send_progress: Option<String>,
    pub cancel_tx: Option<oneshot::Sender<()>>,
    pub script_error: Option<String>,
    input_refresh_task: Option<Task<()>>,
}

impl RequestSessionState {
    pub fn new() -> Self {
        Self {
            active_request_id: Some("scratch".to_string()),
            active_tab_kind: RequestTabKind::Body,
            is_sending: false,
            running_request_id: None,
            send_progress: None,
            cancel_tx: None,
            script_error: None,
            input_refresh_task: None,
        }
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

    // Sending state
    pub fn start_sending(
        &mut self,
        req_id: Option<String>,
        cancel_tx: oneshot::Sender<()>,
        cx: &mut Context<Self>,
    ) {
        self.is_sending = true;
        self.running_request_id = req_id;
        self.send_progress = Some("Connecting...".to_string());
        self.cancel_tx = Some(cancel_tx);
        cx.notify();
    }

    pub fn update_progress(&mut self, progress: String, cx: &mut Context<Self>) {
        self.send_progress = Some(progress);
        cx.notify();
    }

    pub fn stop_sending(&mut self, cx: &mut Context<Self>) {
        self.is_sending = false;
        self.running_request_id = None;
        self.send_progress = None;
        self.cancel_tx = None;
        cx.notify();
    }

    pub fn set_script_error(&mut self, err: Option<String>, cx: &mut Context<Self>) {
        self.script_error = err;
        cx.notify();
    }
}

pub const MAX_DEFAULT_DOMAIN_LEN: usize = 28;
pub const MAX_DISPLAY_TITLE_LEN: usize = 36;

pub fn truncate_text(s: &str, max_chars: usize) -> String {
    let trimmed = s.trim();
    if trimmed.chars().count() > max_chars {
        let truncated: String = trimmed.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    } else {
        trimmed.to_string()
    }
}

/// Extracts the domain (host) from a URL and limits its length.
pub fn default_name_from_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return "Untitled Request".to_string();
    }

    let domain = if let Ok(parsed) = url::Url::parse(trimmed) {
        parsed.host_str().unwrap_or("").to_string()
    } else {
        let without_scheme = trimmed
            .trim_start_matches("https://")
            .trim_start_matches("http://");
        without_scheme
            .split(['/', '?', '#', ':'])
            .next()
            .unwrap_or("")
            .to_string()
    };

    let domain = domain.trim();
    if domain.is_empty() {
        return "Untitled Request".to_string();
    }

    truncate_text(domain, MAX_DEFAULT_DOMAIN_LEN)
}

#[allow(dead_code)]
pub fn is_legacy_url_name(name: &str, url: &str) -> bool {
    let clean = url
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    if clean.is_empty() {
        return false;
    }
    let without_query = clean.split('?').next().unwrap_or(clean);
    let default_dom = default_name_from_url(url);
    (name == clean || name == without_query) && name != default_dom
}

#[allow(dead_code)]
pub fn is_auto_name(name: &str, old_url: &str) -> bool {
    let trimmed = name.trim();
    trimmed.is_empty()
        || trimmed == "New Request"
        || trimmed == "Untitled Request"
        || trimmed == default_name_from_url(old_url)
        || is_legacy_url_name(trimmed, old_url)
}

/// Returns the display name of a request (custom name if set, otherwise "Untitled Request"),
/// capped at `MAX_DISPLAY_TITLE_LEN` characters.
pub fn effective_request_name(req: &HttpRequestItem) -> String {
    effective_request_name_str(&req.name)
}

pub fn effective_request_name_str(raw_name: &str) -> String {
    let raw = raw_name.trim();
    if raw.is_empty() {
        "Untitled Request".to_string()
    } else {
        truncate_text(raw, MAX_DISPLAY_TITLE_LEN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zest_core::models::HttpMethod;

    #[test]
    fn test_editor_state_initial_and_active_request() {
        let mut editor = RequestSessionState::new();
        assert_eq!(editor.active_request_id(), "scratch");

        editor.active_request_id = Some("req_1".to_string());
        assert_eq!(editor.active_request_id(), "req_1");
    }

    #[test]
    fn test_default_name_from_url() {
        assert_eq!(
            default_name_from_url("https://speedtest.vn/get-servers?loc=vn"),
            "speedtest.vn"
        );
        assert_eq!(
            default_name_from_url("http://api.github.com/users/octocat"),
            "api.github.com"
        );
        assert_eq!(
            default_name_from_url("https://localhost:8080/api/v1"),
            "localhost"
        );
        assert_eq!(
            default_name_from_url("speedtest.vn/get-servers"),
            "speedtest.vn"
        );
        assert_eq!(default_name_from_url(""), "Untitled Request");
        assert_eq!(default_name_from_url("   "), "Untitled Request");
    }

    #[test]
    fn test_domain_length_truncation() {
        let long_url =
            "https://very-long-subdomain-name-that-exceeds-maximum-allowed-length.example.com/api";
        let name = default_name_from_url(long_url);
        assert!(name.chars().count() <= MAX_DEFAULT_DOMAIN_LEN);
        assert!(name.ends_with('…'));
    }

    #[test]
    fn test_effective_request_name() {
        let mut req = HttpRequestItem::new(
            "test",
            "New Request",
            HttpMethod::Get,
            "https://speedtest.vn/api",
        );
        assert_eq!(effective_request_name(&req), "New Request");

        req.name = "Get Speedtest Servers".to_string();
        assert_eq!(effective_request_name(&req), "Get Speedtest Servers");

        req.name = "".to_string();
        assert_eq!(effective_request_name(&req), "Untitled Request");

        req.name = "   ".to_string();
        assert_eq!(effective_request_name(&req), "Untitled Request");
    }
}
