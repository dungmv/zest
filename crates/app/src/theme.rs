use gpui_kit::{App, Hsla, Rgba, WindowAppearance};
use serde::{Deserialize, Serialize};
use zest_core::models::HttpMethod;

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

impl AppTheme {
    pub fn dark() -> Self {
        Self {
            // Modern Slate / Zinc Dark Theme: ~#15161c (soft, readable, comfortable)
            bg_app: hex_to_hsla(0x15161c),
            bg_titlebar: hex_to_hsla(0x181920),
            bg_sidebar: hex_to_hsla(0x16171e),
            bg_editor: hex_to_hsla(0x1a1b23),
            bg_response: hex_to_hsla(0x1a1b23),
            bg_panel: hex_to_hsla(0x1e1f29),
            bg_card: hex_to_hsla(0x22232e),
            bg_input: hex_to_hsla(0x1e1f29),
            bg_selected: hex_to_hsla(0x2b2d3c),
            bg_hover: hex_to_hsla(0x262836),
            bg_active: hex_to_hsla(0x313445),

            // Fine, crisp borders
            border: hex_to_hsla(0x2a2b38),
            border_subtle: hex_to_hsla(0x22232f),
            border_focus: hex_to_hsla(0x3b82f6),

            // Typography: crisp, soft off-white text comfortable on dark backgrounds (Slate 200)
            text_primary: hex_to_hsla(0xe2e8f0),
            text_secondary: hex_to_hsla(0x94a3b8),
            text_muted: hex_to_hsla(0x64748b),
            text_disabled: hex_to_hsla(0x475569),

            // Professional vibrant blue
            primary: hex_to_hsla(0x3b82f6),
            primary_hover: hex_to_hsla(0x2563eb),
            primary_active: hex_to_hsla(0x1d4ed8),
            primary_foreground: hex_to_hsla(0xffffff),

            // Secondary action
            secondary: hex_to_hsla(0x262836),
            secondary_hover: hex_to_hsla(0x313445),
            secondary_active: hex_to_hsla(0x3d4054),
            secondary_foreground: hex_to_hsla(0xe2e8f0),

            // Danger action
            danger: hex_to_hsla(0xef4444),
            danger_hover: hex_to_hsla(0xdc2626),
            danger_active: hex_to_hsla(0xb91c1c),
            danger_foreground: hex_to_hsla(0xffffff),

            // Warning action
            warning: hex_to_hsla(0xf59e0b),
            warning_hover: hex_to_hsla(0xd97706),
            warning_foreground: hex_to_hsla(0x0f172a),

            // Success action
            success: hex_to_hsla(0x10b981),
            success_hover: hex_to_hsla(0x059669),
            success_foreground: hex_to_hsla(0xffffff),

            // Info action
            info: hex_to_hsla(0x06b6d4),
            info_hover: hex_to_hsla(0x0891b2),
            info_foreground: hex_to_hsla(0xffffff),

            // Git status
            git_added: hex_to_hsla(0x4ade80),
            git_modified: hex_to_hsla(0xfacc15),
            git_deleted: hex_to_hsla(0xef4444),
            git_untracked: hex_to_hsla(0x4ade80),
            git_ignored: hex_to_hsla(0x64748b),

            // Diff highlights
            diff_added_bg: hex_alpha_to_hsla(0x10b981, 0.15),
            diff_added_text: hex_to_hsla(0x4ade80),
            diff_deleted_bg: hex_alpha_to_hsla(0xef4444, 0.15),
            diff_deleted_text: hex_to_hsla(0xef4444),

            // Timing waterfall
            timing_dns: hex_to_hsla(0x06b6d4),
            timing_tcp: hex_to_hsla(0x3b82f6),
            timing_tls: hex_to_hsla(0xa855f7),
            timing_ttfb: hex_to_hsla(0xeab308),
            timing_download: hex_to_hsla(0x22c55e),

            // Console logs
            log_info: hex_to_hsla(0x3b82f6),
            log_warn: hex_to_hsla(0xf59e0b),
            log_error: hex_to_hsla(0xef4444),
            log_pre_request: hex_to_hsla(0x06b6d4),
            log_post_response: hex_to_hsla(0xa855f7),

            // Autocomplete variable tags
            tag_variable: hex_to_hsla(0x38bdf8),
            tag_dynamic: hex_to_hsla(0xf59e0b),

            // Method colors
            method_get: hex_to_hsla(0x10b981),
            method_post: hex_to_hsla(0x3b82f6),
            method_put: hex_to_hsla(0xf59e0b),
            method_delete: hex_to_hsla(0xef4444),
            method_patch: hex_to_hsla(0x8b5cf6),
            method_head: hex_to_hsla(0x06b6d4),
            method_options: hex_to_hsla(0x64748b),

            // Status colors
            status_2xx: hex_to_hsla(0x10b981),
            status_3xx: hex_to_hsla(0x3b82f6),
            status_4xx: hex_to_hsla(0xf59e0b),
            status_5xx: hex_to_hsla(0xef4444),

            // Interpolation variable highlight
            var_resolved: hex_to_hsla(0x38bdf8),
            var_unresolved: hex_to_hsla(0xef4444),

            // JSON Syntax Highlighting
            json_key: hex_to_hsla(0x38bdf8),         // Sky blue
            json_string: hex_to_hsla(0x34d399),      // Emerald green
            json_number: hex_to_hsla(0xfbbf24),      // Amber / Gold
            json_boolean: hex_to_hsla(0xc084fc),     // Purple / Violet
            json_null: hex_to_hsla(0xf87171),        // Soft coral red
            json_punctuation: hex_to_hsla(0x94a3b8), // Slate / Muted gray

            transparent: gpui_kit::transparent_black(),
        }
    }

    pub fn light() -> Self {
        Self {
            // Modern Light Theme: Slate 50 / White
            bg_app: hex_to_hsla(0xf8fafc),
            bg_titlebar: hex_to_hsla(0xf1f5f9),
            bg_sidebar: hex_to_hsla(0xf8fafc),
            bg_editor: hex_to_hsla(0xffffff),
            bg_response: hex_to_hsla(0xffffff),
            bg_panel: hex_to_hsla(0xf1f5f9),
            bg_card: hex_to_hsla(0xffffff),
            bg_input: hex_to_hsla(0xffffff),
            bg_selected: hex_to_hsla(0xe2e8f0),
            bg_hover: hex_to_hsla(0xf1f5f9),
            bg_active: hex_to_hsla(0xe2e8f0),

            // Fine, crisp borders for light mode
            border: hex_to_hsla(0xe2e8f0),
            border_subtle: hex_to_hsla(0xf1f5f9),
            border_focus: hex_to_hsla(0x2563eb),

            // Typography: ergonomic dark charcoal text, avoiding extreme black-on-white glare (Slate 800)
            text_primary: hex_to_hsla(0x1e293b),
            text_secondary: hex_to_hsla(0x475569),
            text_muted: hex_to_hsla(0x64748b),
            text_disabled: hex_to_hsla(0x94a3b8),

            // Professional vibrant blue
            primary: hex_to_hsla(0x2563eb),
            primary_hover: hex_to_hsla(0x1d4ed8),
            primary_active: hex_to_hsla(0x1e40af),
            primary_foreground: hex_to_hsla(0xffffff),

            // Secondary action
            secondary: hex_to_hsla(0xf1f5f9),
            secondary_hover: hex_to_hsla(0xe2e8f0),
            secondary_active: hex_to_hsla(0xcbd5e1),
            secondary_foreground: hex_to_hsla(0x1e293b),

            // Danger action
            danger: hex_to_hsla(0xdc2626),
            danger_hover: hex_to_hsla(0xb91c1c),
            danger_active: hex_to_hsla(0x991b1b),
            danger_foreground: hex_to_hsla(0xffffff),

            // Warning action
            warning: hex_to_hsla(0xd97706),
            warning_hover: hex_to_hsla(0xb45309),
            warning_foreground: hex_to_hsla(0xffffff),

            // Success action
            success: hex_to_hsla(0x059669),
            success_hover: hex_to_hsla(0x047857),
            success_foreground: hex_to_hsla(0xffffff),

            // Info action
            info: hex_to_hsla(0x0891b2),
            info_hover: hex_to_hsla(0x0e7490),
            info_foreground: hex_to_hsla(0xffffff),

            // Git status (high-contrast for light surfaces)
            git_added: hex_to_hsla(0x059669),
            git_modified: hex_to_hsla(0xd97706),
            git_deleted: hex_to_hsla(0xdc2626),
            git_untracked: hex_to_hsla(0x059669),
            git_ignored: hex_to_hsla(0x94a3b8),

            // Diff highlights (contrast-adjusted)
            diff_added_bg: hex_alpha_to_hsla(0x10b981, 0.12),
            diff_added_text: hex_to_hsla(0x059669),
            diff_deleted_bg: hex_alpha_to_hsla(0xef4444, 0.12),
            diff_deleted_text: hex_to_hsla(0xdc2626),

            // Timing waterfall
            timing_dns: hex_to_hsla(0x0891b2),
            timing_tcp: hex_to_hsla(0x2563eb),
            timing_tls: hex_to_hsla(0x9333ea),
            timing_ttfb: hex_to_hsla(0xd97706),
            timing_download: hex_to_hsla(0x16a34a),

            // Console logs
            log_info: hex_to_hsla(0x2563eb),
            log_warn: hex_to_hsla(0xd97706),
            log_error: hex_to_hsla(0xdc2626),
            log_pre_request: hex_to_hsla(0x0891b2),
            log_post_response: hex_to_hsla(0x9333ea),

            // Autocomplete variable tags
            tag_variable: hex_to_hsla(0x0284c7),
            tag_dynamic: hex_to_hsla(0xd97706),

            // Method colors for light mode (rich, solid contrast)
            method_get: hex_to_hsla(0x059669),
            method_post: hex_to_hsla(0x2563eb),
            method_put: hex_to_hsla(0xd97706),
            method_delete: hex_to_hsla(0xdc2626),
            method_patch: hex_to_hsla(0x7c3aed),
            method_head: hex_to_hsla(0x0891b2),
            method_options: hex_to_hsla(0x475569),

            // Status colors
            status_2xx: hex_to_hsla(0x059669),
            status_3xx: hex_to_hsla(0x2563eb),
            status_4xx: hex_to_hsla(0xd97706),
            status_5xx: hex_to_hsla(0xdc2626),

            // Interpolation variable highlight
            var_resolved: hex_to_hsla(0x0284c7),
            var_unresolved: hex_to_hsla(0xdc2626),

            // JSON Syntax Highlighting for light backgrounds
            json_key: hex_to_hsla(0x0284c7),         // Rich Blue/Cyan
            json_string: hex_to_hsla(0x059669),      // Deep Emerald
            json_number: hex_to_hsla(0xd97706),      // Warm Amber
            json_boolean: hex_to_hsla(0x7c3aed),     // Violet
            json_null: hex_to_hsla(0xe11d48),        // Rose Red
            json_punctuation: hex_to_hsla(0x64748b), // Slate gray

            transparent: gpui_kit::transparent_black(),
        }
    }
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
mod tests {
    use super::*;

    #[test]
    fn test_theme_mode_switching() {
        set_theme(ThemePreference::Light, ThemeMode::Light);
        assert_eq!(current_theme_mode(), ThemeMode::Light);
        assert_eq!(current_theme_preference(), ThemePreference::Light);
        assert_eq!(theme().bg_app, LIGHT_THEME.bg_app);
        assert_eq!(theme().text_primary, hex_to_hsla(0x1e293b));

        set_theme(ThemePreference::Dark, ThemeMode::Dark);
        assert_eq!(current_theme_mode(), ThemeMode::Dark);
        assert_eq!(current_theme_preference(), ThemePreference::Dark);
        assert_eq!(theme().bg_app, DARK_THEME.bg_app);
        assert_eq!(theme().text_primary, hex_to_hsla(0xe2e8f0));

        set_theme(ThemePreference::System, ThemeMode::Dark);
        assert_eq!(current_theme_preference(), ThemePreference::System);
        assert_eq!(current_theme_mode(), ThemeMode::Dark);
    }

    #[test]
    fn test_resolve_theme_mode() {
        assert_eq!(
            resolve_theme_mode(ThemePreference::Dark, WindowAppearance::Light),
            ThemeMode::Dark
        );
        assert_eq!(
            resolve_theme_mode(ThemePreference::Light, WindowAppearance::Dark),
            ThemeMode::Light
        );
        assert_eq!(
            resolve_theme_mode(ThemePreference::System, WindowAppearance::Light),
            ThemeMode::Light
        );
        assert_eq!(
            resolve_theme_mode(ThemePreference::System, WindowAppearance::Dark),
            ThemeMode::Dark
        );
    }

    #[test]
    fn test_semantic_palette_tokens_dark_and_light() {
        let dark = AppTheme::dark();
        let light = AppTheme::light();

        // Ensure actions have distinct contrast
        assert_ne!(dark.primary, light.primary);
        assert_ne!(dark.secondary, light.secondary);
        assert_ne!(dark.danger, light.danger);

        // Ensure git status indicators have high contrast variants
        assert_ne!(dark.git_added, light.git_added);
        assert_ne!(dark.git_modified, light.git_modified);
        assert_eq!(dark.git_added, hex_to_hsla(0x4ade80));
        assert_eq!(light.git_added, hex_to_hsla(0x059669));

        // Ensure diff highlights have distinct background alphas
        assert_ne!(dark.diff_added_bg, light.diff_added_bg);
        assert_ne!(dark.diff_added_text, light.diff_added_text);

        // Ensure timing waterfall tokens are configured
        assert_eq!(dark.timing_dns, hex_to_hsla(0x06b6d4));
        assert_eq!(light.timing_dns, hex_to_hsla(0x0891b2));
    }
}
