use super::ModalEvent;
use crate::settings::{save_settings_change, AppPreferences};
use gpui_kit::Context;
use zest_core::git::CloneCancellation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceSettingTarget {
    Workspace,
    Folder(String),
}

/// Kinds of modal routed by `attach_modal_overlays` (`crate::app::modals`)
/// and registered in [`ModalState::active_modal`].
///
/// Adding a variant touches 6 points — the compiler catches most, but on-open
/// side effects are manual:
/// 1. this enum;
/// 2. a [`ModalPayload`] variant (when the modal takes open-time params);
/// 3. an arm in [`ModalState::reset_transient_ui`];
/// 4. an arm in [`ModalState::apply_payload`];
/// 5. a match arm in `attach_modal_overlays`;
/// 6. manual side effects on open (e.g. `Environment` emits
///    `ModalEvent::EnvModalSelectionChanged` inside [`ModalState::open`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalKind {
    Environment,
    CommandPalette,
    Settings,
    WorkspaceSetting,
    Import,
    Export,
    OpenGitWorkspace,
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

#[derive(Debug, Clone, Default)]
pub struct OpenGitWorkspaceUi {
    pub operation: Option<CloneCancellation>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ModalState {
    pub env_modal: EnvModalUi,
    pub active_modal: Option<ModalKind>,
    pub settings_tab: SettingsTab,
    pub cookie_filter_query: String,
    pub workspace_setting_target: Option<WorkspaceSettingTarget>,
    pub import_modal: ImportModalUi,
    pub export_modal: ExportModalUi,
    pub open_git_workspace: OpenGitWorkspaceUi,
}

impl Default for ModalState {
    fn default() -> Self {
        Self {
            env_modal: EnvModalUi::default(),
            active_modal: None,
            settings_tab: SettingsTab::General,
            cookie_filter_query: String::new(),
            workspace_setting_target: None,
            import_modal: ImportModalUi::default(),
            export_modal: ExportModalUi::default(),
            open_git_workspace: OpenGitWorkspaceUi::default(),
        }
    }
}

impl ModalState {
    pub fn new() -> Self {
        Self::default()
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
            ModalKind::OpenGitWorkspace => self.open_git_workspace = OpenGitWorkspaceUi::default(),
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
        if self.open_git_workspace.operation.is_some() {
            return;
        }
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
        if kind == ModalKind::OpenGitWorkspace {
            if let Some(operation) = &self.open_git_workspace.operation {
                operation.cancel();
                cx.notify();
                return;
            }
        }
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
        AppPreferences::change(cx, |preferences| preferences.theme = pref);
        let appearance = cx.window_appearance();
        let mode = crate::theme::resolve_theme_mode(pref, appearance);
        crate::theme::set_theme(pref, mode);
        crate::theme::sync_component_theme(mode, cx);

        let pref_str = pref.as_str().to_string();
        save_settings_change(cx, move |settings| settings.theme = Some(pref_str));

        cx.emit(ModalEvent::ThemeChanged);
        cx.refresh_windows();
        cx.notify();
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

#[cfg(test)]
mod tests;
