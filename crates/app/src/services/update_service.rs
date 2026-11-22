use crate::state::{ModalState, UpdateStatus};
use gpui_kit::{App, Entity};

/// Runner-style command router for the app updater. Owns no state itself; the
/// check outcome lives in [`ModalState::update_status`] so the About UI stays
/// a pure observer of state.
pub struct UpdateService {
    state: Entity<ModalState>,
}

impl UpdateService {
    pub fn new(state: Entity<ModalState>) -> Self {
        Self { state }
    }

    /// Checks GitHub for a newer release and stores the outcome in
    /// [`ModalState::update_status`]. No-op while a check is already running.
    pub fn check_for_updates(&self, cx: &mut App) {
        if self.state.read(cx).update_status == UpdateStatus::Checking {
            return;
        }
        self.state.update(cx, |state, cx| {
            state.update_status = UpdateStatus::Checking;
            cx.notify();
        });

        let current_version = crate::version::APP_VERSION;
        let state = self.state.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();

        crate::services::global_tokio_runtime().spawn(async move {
            let res =
                zest_core::updater::check_github_release("dungmv", "zest", current_version).await;
            let _ = tx.send(res);
        });

        cx.spawn(async move |cx| {
            let result = rx
                .await
                .unwrap_or_else(|_| Err("Background update task was cancelled".to_string()));

            state.update(cx, |state, cx| {
                state.update_status = match result {
                    Ok(zest_core::updater::UpdateCheckResult::UpToDate { current_version }) => {
                        UpdateStatus::UpToDate {
                            checked_version: current_version,
                        }
                    }
                    Ok(zest_core::updater::UpdateCheckResult::UpdateAvailable {
                        current_version,
                        latest_version,
                        release_url,
                        release_name,
                    }) => UpdateStatus::UpdateAvailable {
                        current_version,
                        latest_version,
                        release_url,
                        release_name,
                    },
                    Err(err) => UpdateStatus::Error(err),
                };
                cx.notify();
            });
        })
        .detach();
    }
}
