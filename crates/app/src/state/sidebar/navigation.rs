#[derive(Default)]
pub(super) struct SidebarNavigation {
    pub(super) history: Vec<String>,
    index: usize,
}

impl SidebarNavigation {
    pub fn record_navigation(&mut self, req_id: &str) {
        if self.history.get(self.index).map(|s| s.as_str()) == Some(req_id) {
            return;
        }
        if self.index + 1 < self.history.len() {
            self.history.truncate(self.index + 1);
        }
        self.history.push(req_id.to_string());
        self.index = self.history.len() - 1;
    }

    pub fn navigate_back(&mut self) -> Option<String> {
        if self.index > 0 {
            self.index -= 1;
            self.history.get(self.index).cloned()
        } else {
            None
        }
    }

    pub fn navigate_forward(&mut self) -> Option<String> {
        if self.index + 1 < self.history.len() {
            self.index += 1;
            self.history.get(self.index).cloned()
        } else {
            None
        }
    }
}
