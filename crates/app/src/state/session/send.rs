use super::RequestSessionState;
use gpui_kit::Context;
use tokio::sync::oneshot;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SendId(u64);

#[derive(Default)]
pub(super) struct RequestSend {
    active: bool,
    request_id: Option<String>,
    progress: Option<String>,
    cancel: Option<oneshot::Sender<()>>,
}

impl RequestSend {
    fn start(request_id: Option<String>, cancel: oneshot::Sender<()>) -> Self {
        Self {
            active: true,
            request_id,
            progress: Some("Connecting...".to_string()),
            cancel: Some(cancel),
        }
    }

    fn cancel(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
    }
}

impl RequestSessionState {
    pub fn is_sending(&self) -> bool {
        self.send.active
    }

    pub fn is_sending_for(&self, active_id: Option<&str>) -> bool {
        self.send.active
            && (self.send.request_id.is_none() || active_id == self.send.request_id.as_deref())
    }

    pub fn send_progress(&self) -> Option<&str> {
        self.send.progress.as_deref()
    }

    pub fn cancel_send(&mut self) {
        self.send.cancel();
    }

    pub(crate) fn is_current_send(&self, id: SendId) -> bool {
        self.send.active && self.send_id == id.0
    }

    pub(crate) fn start_sending(
        &mut self,
        req_id: Option<String>,
        cancel_tx: oneshot::Sender<()>,
        cx: &mut Context<Self>,
    ) -> SendId {
        self.send_id = self.send_id.wrapping_add(1);
        self.send = RequestSend::start(req_id, cancel_tx);
        cx.notify();
        SendId(self.send_id)
    }

    pub fn update_progress(&mut self, progress: String, cx: &mut Context<Self>) {
        self.send.progress = Some(progress);
        cx.notify();
    }

    pub fn stop_sending(&mut self, cx: &mut Context<Self>) {
        self.send = RequestSend::default();
        cx.notify();
    }
}
