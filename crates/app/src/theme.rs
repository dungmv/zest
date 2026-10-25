use gpui::{App, Hsla, Rgba, WindowAppearance};
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

    // Primary action
    pub primary: Hsla,
    pub primary_hover: Hsla,
    pub primary_foreground: Hsla,

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
            primary_foreground: hex_to_hsla(0xffffff),

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

            transparent: gpui::transparent_black(),
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
            primary_foreground: hex_to_hsla(0xffffff),

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

            transparent: gpui::transparent_black(),
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

/// Switch the gpui-component theme to `mode` and align its popover surface with
/// the app card background, so dropdown menus match the app palette instead of
/// the library's near-black default. Call this after [`set_theme`] and in place
/// of every `gpui_component::Theme::change` call.
pub fn sync_component_theme(mode: ThemeMode, cx: &mut App) {
    let gpui_mode = match mode {
        ThemeMode::Dark => gpui_component::ThemeMode::Dark,
        ThemeMode::Light => gpui_component::ThemeMode::Light,
    };
    gpui_component::Theme::change(gpui_mode, None, cx);
    let t = theme();
    gpui_component::Theme::update(cx, |component_theme| {
        component_theme.colors.popover = t.bg_card;
        component_theme.colors.popover_foreground = t.text_primary;
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
}
