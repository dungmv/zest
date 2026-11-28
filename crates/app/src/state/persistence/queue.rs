use super::{PersistPayload, PersistTarget, PersistenceState};
use gpui_kit::{Context, Task};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::oneshot;
use zest_core::storage::coordinator::PersistenceCoordinator;

#[derive(Default)]
pub(super) struct WriteQueue {
    pub(super) coordinator: PersistenceCoordinator,
    pub(super) debounce_tasks: HashMap<PersistTarget, Task<()>>,
    workspace_flush: Option<oneshot::Sender<Result<(), String>>>,
}

impl WriteQueue {
    pub(super) fn cancel(&mut self, target: &PersistTarget) {
        self.coordinator.cancel_target(target);
        self.debounce_tasks.remove(target);
    }

    pub(super) fn reset_for_workspace(&mut self) {
        self.debounce_tasks.clear();
        self.coordinator.bump_workspace_generation();
    }

    fn has_flush_waiter(&self) -> bool {
        self.workspace_flush
            .as_ref()
            .is_some_and(|waiter| !waiter.is_closed())
    }

    fn request_flush(&mut self) -> oneshot::Receiver<Result<(), String>> {
        let (sender, receiver) = oneshot::channel();
        self.workspace_flush = Some(sender);
        receiver
    }

    fn finish_flush(&mut self, result: Result<(), String>) {
        if let Some(waiter) = self.workspace_flush.take() {
            let _ = waiter.send(result);
        }
    }
}

struct WriteCompletion {
    target: PersistTarget,
    gen: u64,
    revision: u64,
    payload: PersistPayload,
    debounce: Duration,
    result: anyhow::Result<()>,
}

impl PersistenceState {
    pub(super) fn enqueue_write(
        &mut self,
        target: PersistTarget,
        payload: PersistPayload,
        debounce: Duration,
        cx: &mut Context<Self>,
    ) {
        if let PersistTarget::Request(ref req_id) = target {
            self.changes.unsaved_requests.insert(req_id.clone());
        }
        self.changes.bump();
        self.writes
            .coordinator
            .enqueue(target.clone(), payload, debounce);
        cx.notify();

        if self.writes.has_flush_waiter() {
            self.flush_target_if_ready(&target, cx);
            return;
        }
        let target_clone = target.clone();
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(debounce).await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |p, cx| {
                    p.flush_target_if_ready(&target_clone, cx);
                });
            }
        });
        self.writes.debounce_tasks.insert(target, task);
    }

    pub(super) fn flush_target_if_ready(&mut self, target: &PersistTarget, cx: &mut Context<Self>) {
        if matches!(target, PersistTarget::Request(id) if self.changes.deleted_requests.contains(id))
        {
            return;
        }
        if self.writes.coordinator.is_in_flight(target) {
            return;
        }
        if let PersistTarget::Environment(name) = target {
            if self.environments.contains(name) {
                self.start_environment_operation(name, cx);
                return;
            }
        }

        let Some(pending) = self.writes.coordinator.begin_in_flight(target) else {
            return;
        };

        let store = self.store.clone();
        let payload = pending.payload;
        let target_clone = target.clone();
        let revision = pending.revision;
        let gen = pending.workspace_gen;
        let debounce = pending.debounce_duration;

        cx.spawn(async move |this, cx| {
            let write_res = cx
                .background_executor()
                .spawn({
                    let payload = payload.clone();
                    let store = store.clone();
                    async move { payload.write_to_store(store.as_ref()) }
                })
                .await;

            if let Some(this) = this.upgrade() {
                this.update(cx, |p, cx| {
                    p.handle_write_result(
                        WriteCompletion {
                            target: target_clone,
                            gen,
                            revision,
                            payload,
                            debounce,
                            result: write_res,
                        },
                        cx,
                    );
                });
            }
        })
        .detach();
    }

    fn handle_write_result(&mut self, completion: WriteCompletion, cx: &mut Context<Self>) {
        if completion.gen != self.writes.coordinator.workspace_generation()
            || !self
                .writes
                .coordinator
                .is_writing_revision(&completion.target, completion.revision)
        {
            return;
        }

        if let PersistTarget::Environment(name) = &completion.target {
            if self.environments.contains(name) {
                match completion.result {
                    Ok(()) => {
                        self.writes
                            .coordinator
                            .mark_success(&completion.target, completion.revision);
                    }
                    Err(error) => self.writes.coordinator.mark_failure(
                        &completion.target,
                        completion.revision,
                        completion.payload,
                        error.to_string(),
                        completion.debounce,
                    ),
                }
                self.start_environment_operation(name, cx);
                return;
            }
        }

        match completion.result {
            Ok(()) => {
                if let PersistPayload::RestoreRequest(request) = &completion.payload {
                    self.changes.restoring_requests.remove(&request.id);
                }
                let is_clean = self
                    .writes
                    .coordinator
                    .mark_success(&completion.target, completion.revision);
                if is_clean {
                    if let PersistTarget::Request(ref req_id) = completion.target {
                        if !self.changes.deleted_requests.contains(req_id) {
                            self.changes.unsaved_requests.remove(req_id);
                        }
                    }
                    self.writes.debounce_tasks.remove(&completion.target);
                    self.changes.bump();
                } else {
                    self.flush_target_if_ready(&completion.target, cx);
                }
                self.finish_workspace_flush(Ok(()));
                cx.notify();
            }
            Err(e) => {
                let error = e.to_string();
                self.writes.coordinator.mark_failure(
                    &completion.target,
                    completion.revision,
                    completion.payload,
                    error.clone(),
                    completion.debounce,
                );
                self.changes.bump();
                self.finish_workspace_flush(Err(error.clone()));
                crate::notify::error("Could Not Save Changes", error, cx);
                cx.notify();
            }
        }
    }

    /// A workspace can be replaced only after pending and in-flight writes settle.
    pub(crate) fn flush_before_switch(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<oneshot::Receiver<Result<(), String>>> {
        if !self.changes.deleted_requests.is_empty() {
            let (sender, receiver) = oneshot::channel();
            let _ = sender.send(Err(
                "Resolve deleted request conflicts before switching workspaces.".into(),
            ));
            return Some(receiver);
        }
        if !self.writes.coordinator.has_any_dirty() && self.environments.is_empty() {
            return None;
        }
        let receiver = self.writes.request_flush();
        self.flush_all(cx);
        Some(receiver)
    }

    pub(super) fn finish_workspace_flush(&mut self, result: Result<(), String>) {
        if result.is_err()
            || (!self.writes.coordinator.has_any_dirty() && self.environments.is_empty())
        {
            self.writes.finish_flush(result);
        }
    }

    pub(super) fn flush_all(&mut self, cx: &mut Context<Self>) {
        let targets = self.writes.coordinator.pending_targets();
        for target in targets {
            self.writes.debounce_tasks.remove(&target);
            self.flush_target_if_ready(&target, cx);
        }
    }
}
