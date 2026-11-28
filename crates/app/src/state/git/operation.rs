#[derive(Debug, Clone, Default)]
pub struct GitOperation {
    loading: bool,
    syncing: bool,
    sync_message: Option<String>,
    error: Option<String>,
}

impl GitOperation {
    pub fn is_loading(&self) -> bool {
        self.loading
    }

    pub fn is_syncing(&self) -> bool {
        self.syncing
    }

    pub fn sync_message(&self) -> Option<&str> {
        self.sync_message.as_deref()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(crate) fn clear_error(&mut self) {
        self.error = None;
    }

    pub(crate) fn set_error(&mut self, error: String) {
        self.error = Some(error);
    }

    pub(crate) fn start_command(&mut self) {
        self.loading = true;
        self.clear_error();
    }

    pub(crate) fn finish_command(&mut self) {
        self.loading = false;
    }

    pub(crate) fn start_sync(&mut self, message: &str) {
        self.syncing = true;
        self.sync_message = Some(message.to_string());
        self.clear_error();
    }

    pub(crate) fn finish_sync(&mut self) {
        self.syncing = false;
        self.sync_message = None;
    }
}
