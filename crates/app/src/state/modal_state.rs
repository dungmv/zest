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

/// Environment-modal local UI state.
#[derive(Debug, Clone, Default)]
pub struct EnvModalUi {
    pub selected_id: Option<String>,
    pub filter_query: String,
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

    pub fn toggle_env_modal(&mut self, cx: &mut Context<Self>) {
        if self.active_modal == Some(ModalKind::Environment) {
            self.close_env_modal(cx);
        } else {
            self.open_env_modal(cx);
        }
    }

    pub fn open_env_modal(&mut self, cx: &mut Context<Self>) {
        self.active_modal = Some(ModalKind::Environment);
        self.env_modal.target_variable = None;
        cx.emit(ModalEvent::EnvModalSelectionChanged);
        cx.notify();
    }

    pub fn close_env_modal(&mut self, cx: &mut Context<Self>) {
        if self.active_modal == Some(ModalKind::Environment) {
            self.active_modal = None;
        }
        self.env_modal.target_variable = None;
        cx.notify();
    }

    pub fn open_env_modal_with_id(&mut self, env_id: Option<String>, cx: &mut Context<Self>) {
        self.active_modal = Some(ModalKind::Environment);
        self.env_modal.selected_id = env_id;
        self.env_modal.target_variable = None;
        cx.emit(ModalEvent::EnvModalSelectionChanged);
        cx.notify();
    }

    pub fn open_env_modal_for_variable(
        &mut self,
        env_id: Option<String>,
        var_name: String,
        cx: &mut Context<Self>,
    ) {
        self.active_modal = Some(ModalKind::Environment);
        self.env_modal.selected_id = env_id;
        self.env_modal.target_variable = Some(var_name);
        cx.emit(ModalEvent::EnvModalSelectionChanged);
        cx.notify();
    }

    pub fn select_modal_environment(&mut self, env_id: Option<String>, cx: &mut Context<Self>) {
        self.env_modal.selected_id = env_id;
        cx.emit(ModalEvent::EnvModalSelectionChanged);
        cx.notify();
    }

    pub fn set_env_filter_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.env_modal.filter_query = query;
        cx.notify();
    }

    pub fn set_cookie_filter_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.cookie_filter_query = query;
        cx.notify();
    }

    pub fn toggle_command_palette(&mut self, cx: &mut Context<Self>) {
        if self.active_modal == Some(ModalKind::CommandPalette) {
            self.close_command_palette(cx);
        } else {
            self.open_command_palette(cx);
        }
    }

    pub fn open_command_palette(&mut self, cx: &mut Context<Self>) {
        self.active_modal = Some(ModalKind::CommandPalette);
        cx.notify();
    }

    pub fn close_command_palette(&mut self, cx: &mut Context<Self>) {
        if self.active_modal == Some(ModalKind::CommandPalette) {
            self.active_modal = None;
        }
        cx.notify();
    }

    pub fn toggle_settings_modal(&mut self, cx: &mut Context<Self>) {
        if self.active_modal == Some(ModalKind::Settings) {
            self.close_settings_modal(cx);
        } else {
            self.open_settings_modal(cx);
        }
    }

    pub fn open_settings_modal(&mut self, cx: &mut Context<Self>) {
        self.active_modal = Some(ModalKind::Settings);
        cx.notify();
    }

    pub fn open_settings_tab(&mut self, tab: SettingsTab, cx: &mut Context<Self>) {
        self.settings_tab = tab;
        self.active_modal = Some(ModalKind::Settings);
        cx.notify();
    }

    pub fn set_settings_tab(&mut self, tab: SettingsTab, cx: &mut Context<Self>) {
        self.settings_tab = tab;
        cx.notify();
    }

    pub fn close_settings_modal(&mut self, cx: &mut Context<Self>) {
        if self.active_modal == Some(ModalKind::Settings) {
            self.active_modal = None;
        }
        cx.notify();
    }

    pub fn open_workspace_setting(
        &mut self,
        target: WorkspaceSettingTarget,
        cx: &mut Context<Self>,
    ) {
        self.workspace_setting_target = Some(target);
        self.active_modal = Some(ModalKind::WorkspaceSetting);
        cx.notify();
    }

    pub fn close_workspace_setting(&mut self, cx: &mut Context<Self>) {
        self.workspace_setting_target = None;
        if self.active_modal == Some(ModalKind::WorkspaceSetting) {
            self.active_modal = None;
        }
        cx.notify();
    }

    pub fn open_import_modal(&mut self, target_folder_rel: Option<String>, cx: &mut Context<Self>) {
        self.import_modal.target_folder_rel = target_folder_rel;
        self.active_modal = Some(ModalKind::Import);
        cx.notify();
    }

    pub fn close_import_modal(&mut self, cx: &mut Context<Self>) {
        self.import_modal.target_folder_rel = None;
        if self.active_modal == Some(ModalKind::Import) {
            self.active_modal = None;
        }
        cx.notify();
    }

    pub fn open_export_modal(&mut self, cx: &mut Context<Self>) {
        self.active_modal = Some(ModalKind::Export);
        self.export_modal.target_folder_id = None;
        self.export_modal.target_folder_name = None;
        cx.notify();
    }

    /// Opens the export modal scoped to one folder subtree.
    pub fn open_export_modal_for_folder(
        &mut self,
        folder_id: String,
        folder_name: String,
        cx: &mut Context<Self>,
    ) {
        self.active_modal = Some(ModalKind::Export);
        self.export_modal.target_folder_id = Some(folder_id);
        self.export_modal.target_folder_name = Some(folder_name);
        cx.notify();
    }

    pub fn close_export_modal(&mut self, cx: &mut Context<Self>) {
        if self.active_modal == Some(ModalKind::Export) {
            self.active_modal = None;
        }
        self.export_modal.target_folder_id = None;
        self.export_modal.target_folder_name = None;
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

    pub fn open_about_modal(&mut self, cx: &mut Context<Self>) {
        self.active_modal = Some(ModalKind::About);
        cx.notify();
    }

    pub fn close_about_modal(&mut self, cx: &mut Context<Self>) {
        if self.active_modal == Some(ModalKind::About) {
            self.active_modal = None;
        }
        cx.notify();
    }

    pub fn toggle_about_modal(&mut self, cx: &mut Context<Self>) {
        if self.active_modal == Some(ModalKind::About) {
            self.close_about_modal(cx);
        } else {
            self.open_about_modal(cx);
        }
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

    pub fn dismiss_top_modal(&mut self, cx: &mut Context<Self>) -> bool {
        let active = self.active_modal;
        match active {
            Some(ModalKind::Environment) => self.close_env_modal(cx),
            Some(ModalKind::CommandPalette) => self.close_command_palette(cx),
            Some(ModalKind::Settings) => self.close_settings_modal(cx),
            Some(ModalKind::WorkspaceSetting) => self.close_workspace_setting(cx),
            Some(ModalKind::Import) => self.close_import_modal(cx),
            Some(ModalKind::Export) => self.close_export_modal(cx),
            Some(ModalKind::About) => self.close_about_modal(cx),
            None => return false,
        }
        true
    }

    pub fn has_any_modal_open(&self) -> bool {
        self.active_modal.is_some()
    }
}

pub fn open_browser_url(url: &str) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}
