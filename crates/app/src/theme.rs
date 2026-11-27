#[path = "theme_palette.rs"]
mod palette;

use gpui_kit::component::highlighter::StatusColors;
use gpui_kit::{App, Hsla, Rgba, WindowAppearance};
use serde::{Deserialize, Serialize};
use zest_core::models::{HttpMethod, Protocol};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    #[default]
    System,
    Dark,
    Light,
}

impl ThemePreference {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    pub fn from_str_opt(s: Option<&str>) -> Self {
        match s {
            Some("light") => Self::Light,
            Some("dark") => Self::Dark,
            _ => Self::System,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

pub fn resolve_theme_mode(pref: ThemePreference, appearance: WindowAppearance) -> ThemeMode {
    match pref {
        ThemePreference::Dark => ThemeMode::Dark,
        ThemePreference::Light => ThemeMode::Light,
        ThemePreference::System => match appearance {
            WindowAppearance::Light | WindowAppearance::VibrantLight => ThemeMode::Light,
            WindowAppearance::Dark | WindowAppearance::VibrantDark => ThemeMode::Dark,
        },
    }
}

// Complete semantic design-token palette; some tokens are reserved for upcoming surfaces.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct AppTheme {
    // Backgrounds
    pub bg_app: Hsla,
    pub bg_titlebar: Hsla,
    pub bg_sidebar: Hsla,
    pub bg_editor: Hsla,
    pub bg_response: Hsla,
    pub bg_panel: Hsla,
    pub bg_card: Hsla,
    pub bg_input: Hsla,
    pub bg_selected: Hsla,
    pub bg_hover: Hsla,
    pub bg_active: Hsla,

    // Borders
    pub border: Hsla,
    pub border_subtle: Hsla,
    pub border_focus: Hsla,

    // Text & Foregrounds
    pub text_primary: Hsla,
    pub text_secondary: Hsla,
    pub text_muted: Hsla,
    pub text_disabled: Hsla,

    // Action variants (Button & interactive)
    pub primary: Hsla,
    pub primary_hover: Hsla,
    pub primary_active: Hsla,
    pub primary_foreground: Hsla,

    pub secondary: Hsla,
    pub secondary_hover: Hsla,
    pub secondary_active: Hsla,
    pub secondary_foreground: Hsla,

    pub danger: Hsla,
    pub danger_hover: Hsla,
    pub danger_active: Hsla,
    pub danger_foreground: Hsla,

    pub warning: Hsla,
    pub warning_hover: Hsla,
    pub warning_foreground: Hsla,

    pub success: Hsla,
    pub success_hover: Hsla,
    pub success_foreground: Hsla,

    pub info: Hsla,
    pub info_hover: Hsla,
    pub info_foreground: Hsla,

    // Git status
    pub git_added: Hsla,
    pub git_modified: Hsla,
    pub git_deleted: Hsla,
    pub git_untracked: Hsla,
    pub git_ignored: Hsla,

    // Diff highlights
    pub diff_added_bg: Hsla,
    pub diff_added_text: Hsla,
    pub diff_deleted_bg: Hsla,
    pub diff_deleted_text: Hsla,

    // Timing waterfall
    pub timing_dns: Hsla,
    pub timing_tcp: Hsla,
    pub timing_tls: Hsla,
    pub timing_ttfb: Hsla,
    pub timing_download: Hsla,

    // Console logs
    pub log_info: Hsla,
    pub log_warn: Hsla,
    pub log_error: Hsla,
    pub log_pre_request: Hsla,
    pub log_post_response: Hsla,

    // Autocomplete variable tags
    pub tag_variable: Hsla,
    pub tag_dynamic: Hsla,

    // Method badge colors
    pub method_get: Hsla,
    pub method_post: Hsla,
    pub method_put: Hsla,
    pub method_delete: Hsla,
    pub method_patch: Hsla,
    pub method_head: Hsla,
    pub method_options: Hsla,
    pub method_websocket: Hsla,
    pub method_grpc: Hsla,

    // Status colors
    pub status_2xx: Hsla,
    pub status_3xx: Hsla,
    pub status_4xx: Hsla,
    pub status_5xx: Hsla,

    // Variable highlights
    pub var_resolved: Hsla,
    pub var_unresolved: Hsla,

    // JSON Syntax Highlighting
    pub json_key: Hsla,
    pub json_string: Hsla,
    pub json_number: Hsla,
    pub json_boolean: Hsla,
    pub json_null: Hsla,
    pub json_punctuation: Hsla,

    // Transparent
    pub transparent: Hsla,
}

pub fn hex_to_hsla(hex: u32) -> Hsla {
    let r = ((hex >> 16) & 0xFF) as f32 / 255.0;
    let g = ((hex >> 8) & 0xFF) as f32 / 255.0;
    let b = (hex & 0xFF) as f32 / 255.0;
    Hsla::from(Rgba { r, g, b, a: 1.0 })
}

pub fn hex_alpha_to_hsla(hex: u32, alpha: f32) -> Hsla {
    let r = ((hex >> 16) & 0xFF) as f32 / 255.0;
    let g = ((hex >> 8) & 0xFF) as f32 / 255.0;
    let b = (hex & 0xFF) as f32 / 255.0;
    Hsla::from(Rgba { r, g, b, a: alpha })
}

impl Default for AppTheme {
    fn default() -> Self {
        Self::dark()
    }
}

// 0 = Dark, 1 = Light
static CURRENT_MODE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
// 0 = System, 1 = Dark, 2 = Light
static CURRENT_PREF: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

static DARK_THEME: std::sync::LazyLock<AppTheme> = std::sync::LazyLock::new(AppTheme::dark);
static LIGHT_THEME: std::sync::LazyLock<AppTheme> = std::sync::LazyLock::new(AppTheme::light);

pub fn theme() -> &'static AppTheme {
    match CURRENT_MODE.load(std::sync::atomic::Ordering::Relaxed) {
        1 => &LIGHT_THEME,
        _ => &DARK_THEME,
    }
}

pub fn current_theme_mode() -> ThemeMode {
    match CURRENT_MODE.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ThemeMode::Light,
        _ => ThemeMode::Dark,
    }
}

// Public theme preference accessor; currently exercised by unit tests.
#[allow(dead_code)]
pub fn current_theme_preference() -> ThemePreference {
    match CURRENT_PREF.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ThemePreference::Dark,
        2 => ThemePreference::Light,
        _ => ThemePreference::System,
    }
}

pub fn set_theme(preference: ThemePreference, resolved_mode: ThemeMode) {
    let pref_val = match preference {
        ThemePreference::System => 0,
        ThemePreference::Dark => 1,
        ThemePreference::Light => 2,
    };
    CURRENT_PREF.store(pref_val, std::sync::atomic::Ordering::Relaxed);

    let mode_val = match resolved_mode {
        ThemeMode::Dark => 0,
        ThemeMode::Light => 1,
    };
    CURRENT_MODE.store(mode_val, std::sync::atomic::Ordering::Relaxed);
}

/// Switch the gpui-component theme to `mode` and synchronize its full color palette
/// with [`AppTheme`]. This ensures all gpui-kit UI components (Button variants,
/// Input, Popover, TabBar, Sidebar, Table, DropdownMenu) use Zest's exact palette
/// instead of the library's neutral black/white defaults. Call this after [`set_theme`]
/// and in place of every `gpui_kit::component::Theme::change` call.
pub fn sync_component_theme(mode: ThemeMode, cx: &mut App) {
    let gpui_mode = match mode {
        ThemeMode::Dark => gpui_kit::component::ThemeMode::Dark,
        ThemeMode::Light => gpui_kit::component::ThemeMode::Light,
    };
    gpui_kit::component::Theme::change(gpui_mode, None, cx);
    let t = theme();
    gpui_kit::component::Theme::update(cx, |c| {
        // Core surfaces & text
        c.background = t.bg_app;
        c.foreground = t.text_primary;
        c.border = t.border;
        c.input = t.border;
        c.ring = t.border_focus;
        c.popover = t.bg_card;
        c.popover_foreground = t.text_primary;

        // Primary action
        c.primary = t.primary;
        c.primary_hover = t.primary_hover;
        c.primary_active = t.primary_active;
        c.primary_foreground = t.primary_foreground;
        c.button_primary = t.primary;
        c.button_primary_hover = t.primary_hover;
        c.button_primary_active = t.primary_active;
        c.button_primary_foreground = t.primary_foreground;

        // Secondary action
        c.secondary = t.secondary;
        c.secondary_hover = t.secondary_hover;
        c.secondary_active = t.secondary_active;
        c.secondary_foreground = t.secondary_foreground;
        c.button_secondary = t.secondary;
        c.button_secondary_hover = t.secondary_hover;
        c.button_secondary_active = t.secondary_active;
        c.button_secondary_foreground = t.secondary_foreground;

        // Danger action
        c.danger = t.danger;
        c.danger_hover = t.danger_hover;
        c.danger_active = t.danger_active;
        c.danger_foreground = t.danger_foreground;
        c.button_danger = t.danger;
        c.button_danger_hover = t.danger_hover;
        c.button_danger_active = t.danger_active;
        c.button_danger_foreground = t.danger_foreground;

        // Warning & Success & Info
        c.warning = t.warning;
        c.warning_foreground = t.warning_foreground;
        c.button_warning = t.warning;
        c.button_warning_hover = t.warning_hover;
        c.button_warning_foreground = t.warning_foreground;

        c.success = t.success;
        c.success_foreground = t.success_foreground;
        c.button_success = t.success;
        c.button_success_hover = t.success_hover;
        c.button_success_foreground = t.success_foreground;

        c.info = t.info;
        c.info_foreground = t.info_foreground;
        c.button_info = t.info;
        c.button_info_hover = t.info_hover;
        c.button_info_foreground = t.info_foreground;

        // Default button
        c.button = t.bg_card;
        c.button_hover = t.bg_hover;
        c.button_active = t.bg_active;
        c.button_foreground = t.text_primary;

        // Accent & Muted
        c.accent = t.bg_hover;
        c.accent_foreground = t.text_primary;
        c.muted = t.bg_input;
        c.muted_foreground = t.text_muted;

        // Sidebar
        c.sidebar = t.bg_sidebar;
        c.sidebar_border = t.border;
        c.sidebar_foreground = t.text_primary;
        c.sidebar_accent = t.bg_hover;
        c.sidebar_accent_foreground = t.text_primary;
        c.sidebar_primary = t.primary;
        c.sidebar_primary_foreground = t.primary_foreground;

        // Titlebar
        c.title_bar = t.bg_titlebar;
        c.title_bar_border = t.border;

        // Tabs
        c.tab_bar = t.bg_panel;
        c.tab = t.transparent;
        c.tab_active = t.bg_card;
        c.tab_active_foreground = t.text_primary;
        c.tab_foreground = t.text_secondary;

        // Table
        c.table = t.bg_editor;
        c.table_head = t.bg_panel;
        c.table_head_foreground = t.text_muted;
        c.table_hover = t.bg_hover;
        c.table_active = t.bg_selected;
        c.table_row_border = t.border_subtle;

        // Editor diagnostics: opaque, high-contrast token backgrounds so
        // hover tooltips stay readable over any content. StatusColors has
        // private fields — its serde JSON form is the configuration path.
        let mut highlight = (*c.highlight_theme).clone();
        let status_token = |bg: gpui_kit::Hsla, accent: gpui_kit::Hsla, alpha: f32| -> String {
            let blended = bg.blend(accent.alpha(alpha));
            let rgba = gpui_kit::Rgba::from(blended);
            format!(
                "#{:02x}{:02x}{:02x}",
                (rgba.r * 255.0).round() as u8,
                (rgba.g * 255.0).round() as u8,
                (rgba.b * 255.0).round() as u8
            )
        };
        let error_bg = status_token(t.bg_app, t.status_5xx, 0.30);
        let warning_bg = status_token(t.bg_app, t.warning, 0.22);
        let info_bg = status_token(t.bg_app, t.info, 0.18);
        let success_bg = status_token(t.bg_app, t.success, 0.18);
        if let Ok(status) = serde_json::from_str::<StatusColors>(&format!(
            "{{\"error.background\": \"{error_bg}\", \"warning.background\": \"{warning_bg}\", \"info.background\": \"{info_bg}\", \"success.background\": \"{success_bg}\"}}"
        )) {
            highlight.style.status = status;
            c.highlight_theme = std::sync::Arc::new(highlight);
        }

        c.notification.placement = gpui_kit::Anchor::BottomRight;
        c.notification.max_items = 5;
    });
}

impl AppTheme {
    pub fn method_color(&self, method: HttpMethod) -> Hsla {
        match method {
            HttpMethod::Get => self.method_get,
            HttpMethod::Post => self.method_post,
            HttpMethod::Put => self.method_put,
            HttpMethod::Delete => self.method_delete,
            HttpMethod::Patch => self.method_patch,
            HttpMethod::Head => self.method_head,
            HttpMethod::Options => self.method_options,
        }
    }

    /// Badge color for non-HTTP transport protocols. GraphQL/SSE map to a
    /// neutral muted tone — the monochrome direction forbids new colored
    /// accents; existing WS/gRPC hues stay until the user asks for a
    /// monochrome refactor of the legacy badges.
    pub fn protocol_color(&self, protocol: Protocol) -> Hsla {
        match protocol {
            Protocol::Http => self.method_get,
            Protocol::WebSocket => self.method_websocket,
            Protocol::Grpc => self.method_grpc,
            Protocol::GraphQL | Protocol::Sse => self.text_muted,
        }
    }

    pub fn status_color(&self, status_code: u16) -> Hsla {
        match status_code {
            200..=299 => self.status_2xx,
            300..=399 => self.status_3xx,
            400..=499 => self.status_4xx,
            500..=599 => self.status_5xx,
            _ => self.text_muted,
        }
    }
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
