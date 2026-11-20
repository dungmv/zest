use super::ModalEvent;
use crate::settings::AppSettings;
use gpui_kit::Context;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceSettingTarget {
    Workspace,
    Folder(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalKind {
    Environment,
    CommandPalette,
    Settings,
    WorkspaceSetting,
    Import,
    Export,
    About,
}

/// Optional open-time parameters for [`ModalState::open`]. Opening without a
/// payload keeps sticky modal state untouched and only resets transient
/// fields (see [`ModalState::reset_transient_ui`]).
#[derive(Debug, Clone)]
pub enum ModalPayload {
    /// Environment modal targeted at an env id and/or variable name.
    Env {
        env_id: Option<String>,
        target_variable: Option<String>,
    },
    /// Workspace settings scoped to the whole workspace or one folder.
    WorkspaceSetting(WorkspaceSettingTarget),
    /// Import scoped to a target folder relative path.
    Import { target_folder_rel: Option<String> },
    /// Export scoped to a folder subtree (`folder_id`, `folder_name`).
    Export { folder: Option<(String, String)> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    General,
    Network,
    Cookies,
    Shortcuts,
    About,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum UpdateStatus {
    #[default]
    Idle,
    Checking,
    UpToDate {
        checked_version: String,
    },
    UpdateAvailable {
        current_version: String,
        latest_version: String,
        release_url: String,
        release_name: Option<String>,
    },
    Error(String),
}

/// User-configurable network execution preferences, persisted via
/// [`AppSettings`].
#[derive(Debug, Clone)]
pub struct NetworkPrefs {
    pub request_timeout_secs: u64,
    pub follow_redirects: bool,
    pub validate_tls: bool,
}

impl Default for NetworkPrefs {
    fn default() -> Self {
        Self {
            request_timeout_secs: 30,
            follow_redirects: true,
            validate_tls: true,
        }
    }
}

/// Environment-modal selection state. `selected_id` is sticky and survives
/// close/reopen; `target_variable` is transient and cleared on open/close.
#[derive(Debug, Clone, Default)]
pub struct EnvModalUi {
    pub selected_id: Option<String>,
    pub target_variable: Option<String>,
}

/// Import-modal local UI state.
#[derive(Debug, Clone, Default)]
pub struct ImportModalUi {
    pub target_folder_rel: Option<String>,
}

/// Export-modal local UI state. `target_folder_id` scopes the export to one
/// folder subtree; `None` exports the whole workspace.
#[derive(Debug, Clone, Default)]
pub struct ExportModalUi {
    pub target_folder_id: Option<String>,
    pub target_folder_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ModalState {
    pub env_modal: EnvModalUi,
    pub active_modal: Option<ModalKind>,
    pub update_status: UpdateStatus,
    pub settings_tab: SettingsTab,
    pub cookie_filter_query: String,
    pub workspace_setting_target: Option<WorkspaceSettingTarget>,
    pub import_modal: ImportModalUi,
    pub export_modal: ExportModalUi,
    pub theme_preference: crate::theme::ThemePreference,
    pub split_layout_vertical: bool,
    pub network: NetworkPrefs,
}

impl Default for ModalState {
    fn default() -> Self {
        Self {
            env_modal: EnvModalUi::default(),
            active_modal: None,
            update_status: UpdateStatus::Idle,
            settings_tab: SettingsTab::General,
            cookie_filter_query: String::new(),
            workspace_setting_target: None,
            import_modal: ImportModalUi::default(),
            export_modal: ExportModalUi::default(),
            theme_preference: crate::theme::ThemePreference::System,
            split_layout_vertical: false,
            network: NetworkPrefs::default(),
        }
    }
}

impl ModalState {
    pub fn new() -> Self {
        let mut state = Self::default();
        let settings = AppSettings::load();
        state.theme_preference =
            crate::theme::ThemePreference::from_str_opt(settings.theme.as_deref());
        state.network.request_timeout_secs = settings.request_timeout_secs;
        state.network.follow_redirects = settings.follow_redirects;
        state.network.validate_tls = settings.validate_tls;
        state
    }

    pub fn set_request_timeout_secs(&mut self, secs: u64, cx: &mut Context<Self>) {
        self.network.request_timeout_secs = secs;
        let mut settings = AppSettings::load();
        settings.request_timeout_secs = secs;
        let _ = settings.save();
        cx.notify();
    }

    pub fn set_follow_redirects(&mut self, follow: bool, cx: &mut Context<Self>) {
        self.network.follow_redirects = follow;
        let mut settings = AppSettings::load();
        settings.follow_redirects = follow;
        let _ = settings.save();
        cx.notify();
    }

    pub fn set_validate_tls(&mut self, validate: bool, cx: &mut Context<Self>) {
        self.network.validate_tls = validate;
        let mut settings = AppSettings::load();
        settings.validate_tls = validate;
        let _ = settings.save();
        cx.notify();
    }

    pub fn toggle_split_layout(&mut self, cx: &mut Context<Self>) {
        self.split_layout_vertical = !self.split_layout_vertical;
        cx.notify();
    }

    pub fn set_split_layout_vertical(&mut self, vertical: bool, cx: &mut Context<Self>) {
        self.split_layout_vertical = vertical;
        cx.notify();
    }

    /// Registry of transient UI fields reset on every open/close, per kind:
    /// - `Environment`: `env_modal.target_variable`
    /// - `WorkspaceSetting`: `workspace_setting_target`
    /// - `Import`: `import_modal.target_folder_rel`
    /// - `Export`: `export_modal.target_folder_id/name`
    ///
    /// Sticky state (`env_modal.selected_id`, `settings_tab`,
    /// `cookie_filter_query`, `update_status`) is never cleared here.
    fn reset_transient_ui(&mut self, kind: ModalKind) {
        match kind {
            ModalKind::Environment => self.env_modal.target_variable = None,
            ModalKind::WorkspaceSetting => self.workspace_setting_target = None,
            ModalKind::Import => self.import_modal.target_folder_rel = None,
            ModalKind::Export => {
                self.export_modal.target_folder_id = None;
                self.export_modal.target_folder_name = None;
            }
            ModalKind::CommandPalette | ModalKind::Settings | ModalKind::About => {}
        }
    }

    fn apply_payload(&mut self, payload: ModalPayload) {
        match payload {
            ModalPayload::Env {
                env_id,
                target_variable,
            } => {
                self.env_modal.selected_id = env_id;
                self.env_modal.target_variable = target_variable;
            }
            ModalPayload::WorkspaceSetting(target) => self.workspace_setting_target = Some(target),
            ModalPayload::Import { target_folder_rel } => {
                self.import_modal.target_folder_rel = target_folder_rel;
            }
            ModalPayload::Export { folder } => {
                if let Some((folder_id, folder_name)) = folder {
                    self.export_modal.target_folder_id = Some(folder_id);
                    self.export_modal.target_folder_name = Some(folder_name);
                }
            }
        }
    }

    /// Opens `kind` and applies `payload` (when provided) after resetting its
    /// transient UI fields. Opening the Environment modal always emits
    /// `ModalEvent::EnvModalSelectionChanged` so `EnvVarTable` reconciles its
    /// rows and re-applies the target-variable focus.
    pub fn open(&mut self, kind: ModalKind, payload: Option<ModalPayload>, cx: &mut Context<Self>) {
        self.active_modal = Some(kind);
        self.reset_transient_ui(kind);
        if let Some(payload) = payload {
            self.apply_payload(payload);
        }
        if kind == ModalKind::Environment {
            cx.emit(ModalEvent::EnvModalSelectionChanged);
        }
        cx.notify();
    }

    /// Closes `kind` if it is the active modal and resets its transient UI
    /// fields. Sticky state survives close/reopen.
    pub fn close(&mut self, kind: ModalKind, cx: &mut Context<Self>) {
        if self.active_modal == Some(kind) {
            self.active_modal = None;
        }
        self.reset_transient_ui(kind);
        cx.notify();
    }

    /// Closes `kind` when it is the active modal, opens it otherwise.
    pub fn toggle(&mut self, kind: ModalKind, cx: &mut Context<Self>) {
        if self.active_modal == Some(kind) {
            self.close(kind, cx);
        } else {
            self.open(kind, None, cx);
        }
    }

    pub fn select_modal_environment(&mut self, env_id: Option<String>, cx: &mut Context<Self>) {
        self.env_modal.selected_id = env_id;
        cx.emit(ModalEvent::EnvModalSelectionChanged);
        cx.notify();
    }

    pub fn set_settings_tab(&mut self, tab: SettingsTab, cx: &mut Context<Self>) {
        self.settings_tab = tab;
        cx.notify();
    }

    pub fn set_cookie_filter_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.cookie_filter_query = query;
        cx.notify();
    }

    pub fn set_theme_preference(
        &mut self,
        pref: crate::theme::ThemePreference,
        cx: &mut Context<Self>,
    ) {
        self.theme_preference = pref;
        let appearance = cx.window_appearance();
        let mode = crate::theme::resolve_theme_mode(pref, appearance);
        crate::theme::set_theme(pref, mode);
        crate::theme::sync_component_theme(mode, cx);

        let mut settings = AppSettings::load();
        settings.theme = Some(pref.as_str().to_string());
        let _ = settings.save();

        cx.refresh_windows();
        cx.notify();
    }

    pub fn check_for_updates(&mut self, cx: &mut Context<Self>) {
        if self.update_status == UpdateStatus::Checking {
            return;
        }
        self.update_status = UpdateStatus::Checking;
        cx.notify();

        let current_version = crate::version::APP_VERSION;
        let (tx, rx) = tokio::sync::oneshot::channel();

        crate::services::global_tokio_runtime().spawn(async move {
            let res =
                zest_core::updater::check_github_release("dungmv", "zest", current_version).await;
            let _ = tx.send(res);
        });

        cx.spawn(async move |this, cx| {
            let result = rx
                .await
                .unwrap_or_else(|_| Err("Background update task was cancelled".to_string()));

            let _ = this.update(cx, |state, cx| {
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

    /// Dismisses the currently active modal, if any. Returns whether a modal
    /// was open.
    pub fn dismiss_top_modal(&mut self, cx: &mut Context<Self>) -> bool {
        match self.active_modal {
            Some(kind) => {
                self.close(kind, cx);
                true
            }
            None => false,
        }
    }

    pub fn has_any_modal_open(&self) -> bool {
        self.active_modal.is_some()
    }
}
