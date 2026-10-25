use crate::settings::AppSettings;
use gpui::Context;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeSettingsTarget {
    Workspace,
    Folder(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    General,
    Network,
    Cookies,
    Shortcuts,
}

#[derive(Debug, Clone)]
pub struct ModalState {
    pub show_env_modal: bool,
    pub selected_env_modal_id: Option<String>,
    pub env_filter_query: String,
    pub show_command_palette: bool,
    pub show_settings_modal: bool,
    pub settings_tab: SettingsTab,
    pub cookie_filter_query: String,
    pub scope_settings_target: Option<ScopeSettingsTarget>,
    pub show_import_modal: bool,
    pub import_target_folder_rel: Option<String>,
    pub theme_preference: crate::theme::ThemePreference,
    pub split_layout_vertical: bool,
    pub request_timeout_secs: u64,
    pub follow_redirects: bool,
    pub validate_tls: bool,
}

impl Default for ModalState {
    fn default() -> Self {
        Self {
            show_env_modal: false,
            selected_env_modal_id: None,
            env_filter_query: String::new(),
            show_command_palette: false,
            show_settings_modal: false,
            settings_tab: SettingsTab::General,
            cookie_filter_query: String::new(),
            scope_settings_target: None,
            show_import_modal: false,
            import_target_folder_rel: None,
            theme_preference: crate::theme::ThemePreference::System,
            split_layout_vertical: false,
            request_timeout_secs: 30,
            follow_redirects: true,
            validate_tls: true,
        }
    }
}

impl ModalState {
    pub fn new() -> Self {
        let mut state = Self::default();
        let settings = AppSettings::load();
        state.theme_preference =
            crate::theme::ThemePreference::from_str_opt(settings.theme.as_deref());
        state.request_timeout_secs = settings.request_timeout_secs;
        state.follow_redirects = settings.follow_redirects;
        state.validate_tls = settings.validate_tls;
        state
    }

    pub fn set_request_timeout_secs(&mut self, secs: u64, cx: &mut Context<Self>) {
        self.request_timeout_secs = secs;
        let mut settings = AppSettings::load();
        settings.request_timeout_secs = secs;
        let _ = settings.save();
        cx.notify();
    }

    pub fn set_follow_redirects(&mut self, follow: bool, cx: &mut Context<Self>) {
        self.follow_redirects = follow;
        let mut settings = AppSettings::load();
        settings.follow_redirects = follow;
        let _ = settings.save();
        cx.notify();
    }

    pub fn set_validate_tls(&mut self, validate: bool, cx: &mut Context<Self>) {
        self.validate_tls = validate;
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
        self.show_env_modal = !self.show_env_modal;
        cx.notify();
    }

    pub fn open_env_modal(&mut self, cx: &mut Context<Self>) {
        self.show_env_modal = true;
        cx.notify();
    }

    pub fn close_env_modal(&mut self, cx: &mut Context<Self>) {
        self.show_env_modal = false;
        cx.notify();
    }

    pub fn open_env_modal_with_id(&mut self, env_id: Option<String>, cx: &mut Context<Self>) {
        self.show_env_modal = true;
        self.selected_env_modal_id = env_id;
        cx.notify();
    }

    pub fn select_modal_environment(&mut self, env_id: Option<String>, cx: &mut Context<Self>) {
        self.selected_env_modal_id = env_id;
        cx.notify();
    }

    pub fn set_env_filter_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.env_filter_query = query;
        cx.notify();
    }

    pub fn set_cookie_filter_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.cookie_filter_query = query;
        cx.notify();
    }

    pub fn toggle_command_palette(&mut self, cx: &mut Context<Self>) {
        self.show_command_palette = !self.show_command_palette;
        cx.notify();
    }

    pub fn open_command_palette(&mut self, cx: &mut Context<Self>) {
        self.show_command_palette = true;
        cx.notify();
    }

    pub fn close_command_palette(&mut self, cx: &mut Context<Self>) {
        self.show_command_palette = false;
        cx.notify();
    }

    pub fn toggle_settings_modal(&mut self, cx: &mut Context<Self>) {
        self.show_settings_modal = !self.show_settings_modal;
        cx.notify();
    }

    pub fn open_settings_modal(&mut self, cx: &mut Context<Self>) {
        self.show_settings_modal = true;
        cx.notify();
    }

    pub fn open_settings_tab(&mut self, tab: SettingsTab, cx: &mut Context<Self>) {
        self.settings_tab = tab;
        self.show_settings_modal = true;
        cx.notify();
    }

    pub fn set_settings_tab(&mut self, tab: SettingsTab, cx: &mut Context<Self>) {
        self.settings_tab = tab;
        cx.notify();
    }

    pub fn close_settings_modal(&mut self, cx: &mut Context<Self>) {
        self.show_settings_modal = false;
        cx.notify();
    }

    pub fn open_scope_settings(&mut self, target: ScopeSettingsTarget, cx: &mut Context<Self>) {
        self.scope_settings_target = Some(target);
        cx.notify();
    }

    pub fn open_workspace_settings(&mut self, cx: &mut Context<Self>) {
        self.scope_settings_target = Some(ScopeSettingsTarget::Workspace);
        cx.notify();
    }

    pub fn open_folder_settings(&mut self, folder_id: String, cx: &mut Context<Self>) {
        self.scope_settings_target = Some(ScopeSettingsTarget::Folder(folder_id));
        cx.notify();
    }

    pub fn close_scope_settings(&mut self, cx: &mut Context<Self>) {
        self.scope_settings_target = None;
        cx.notify();
    }

    pub fn open_import_modal(&mut self, target_folder_rel: Option<String>, cx: &mut Context<Self>) {
        self.import_target_folder_rel = target_folder_rel;
        self.show_import_modal = true;
        cx.notify();
    }

    pub fn close_import_modal(&mut self, cx: &mut Context<Self>) {
        self.show_import_modal = false;
        self.import_target_folder_rel = None;
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

    pub fn dismiss_top_modal(&mut self, cx: &mut Context<Self>) -> bool {
        if self.show_command_palette {
            self.show_command_palette = false;
            cx.notify();
            return true;
        }
        if self.show_import_modal {
            self.show_import_modal = false;
            self.import_target_folder_rel = None;
            cx.notify();
            return true;
        }
        if self.show_settings_modal {
            self.show_settings_modal = false;
            cx.notify();
            return true;
        }
        if self.scope_settings_target.is_some() {
            self.scope_settings_target = None;
            cx.notify();
            return true;
        }
        if self.show_env_modal {
            self.show_env_modal = false;
            cx.notify();
            return true;
        }
        false
    }
}
