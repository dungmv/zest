use super::WorkspaceState;
use gpui_kit::Context;
use zest_core::models::StoredCookie;

impl WorkspaceState {
    /// Refreshes the in-memory cookie snapshot from the executor without
    /// touching disk. Returns the snapshot so the caller can persist it on
    /// the background executor (disk writes must not run on the UI thread).
    pub fn sync_cookies_in_memory(&mut self) -> Vec<StoredCookie> {
        self.cookies = self.executor.get_stored_cookies();
        self.cookies.clone()
    }

    #[cfg(test)]
    pub fn sync_cookies_from_executor(&mut self) {
        let cookies = self.sync_cookies_in_memory();
        let _ = crate::services::workspace_io::save_cookies(self.store.as_ref(), cookies);
    }

    #[cfg(test)]
    pub fn remove_cookie_internal(&mut self, domain: &str, path: &str, name: &str) {
        self.executor.remove_cookie(domain, path, name);
        self.sync_cookies_from_executor();
    }

    pub fn remove_cookie(&mut self, domain: &str, path: &str, name: &str, cx: &mut Context<Self>) {
        self.executor.remove_cookie(domain, path, name);
        let cookies = self.sync_cookies_in_memory();
        crate::services::workspace_io::save_cookies_later(self.store.clone(), cookies, cx);
        cx.notify();
    }

    #[cfg(test)]
    pub fn clear_cookies_internal(&mut self) {
        self.executor.clear_cookies();
        self.cookies.clear();
        let _ = self.store.save_cookies(&serde_json::json!([]));
    }

    pub fn clear_cookies(&mut self, cx: &mut Context<Self>) {
        self.executor.clear_cookies();
        self.cookies.clear();
        crate::services::workspace_io::save_cookies_later(self.store.clone(), Vec::new(), cx);
        cx.notify();
    }
}
