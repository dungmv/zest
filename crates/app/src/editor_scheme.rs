use crate::theme::{current_theme_mode, hex_to_hsla, ThemeMode};
use gpui_kit::component::highlighter::StatusColors;
use gpui_kit::component::Theme;
use gpui_kit::{Hsla, Rgba};
use std::sync::{Arc, LazyLock};

pub(crate) struct EditorColorScheme {
    pub foreground: Hsla,
    pub line_number: Hsla,
    pub gutter_background: Hsla,
    pub comment: Hsla,
    pub accent: Hsla,
    pub key: Hsla,
    pub string: Hsla,
    pub number: Hsla,
    pub keyword: Hsla,
    pub null: Hsla,
    pub punctuation: Hsla,
    pub diagnostic_error: Hsla,
    diagnostic_warning: Hsla,
    diagnostic_info: Hsla,
    diagnostic_success: Hsla,
}

static DARK: LazyLock<EditorColorScheme> = LazyLock::new(|| EditorColorScheme {
    foreground: hex_to_hsla(0xcdd3df),
    line_number: hex_to_hsla(0xa7b2c4),
    gutter_background: hex_to_hsla(0x22232e),
    comment: hex_to_hsla(0x98a2b4),
    accent: hex_to_hsla(0x86aaf2),
    key: hex_to_hsla(0x9acef4),
    string: hex_to_hsla(0x99d9bc),
    number: hex_to_hsla(0xf2c47f),
    keyword: hex_to_hsla(0xffc4ee),
    null: hex_to_hsla(0xffa5aa),
    punctuation: hex_to_hsla(0xbbc7db),
    diagnostic_error: hex_to_hsla(0xed7880),
    diagnostic_warning: hex_to_hsla(0xf59e0b),
    diagnostic_info: hex_to_hsla(0x06b6d4),
    diagnostic_success: hex_to_hsla(0x10b981),
});

static LIGHT: LazyLock<EditorColorScheme> = LazyLock::new(|| EditorColorScheme {
    foreground: hex_to_hsla(0x1e293b),
    line_number: hex_to_hsla(0x475569),
    gutter_background: hex_to_hsla(0xffffff),
    comment: hex_to_hsla(0x64748b),
    accent: hex_to_hsla(0x2563eb),
    key: hex_to_hsla(0x0284c7),
    string: hex_to_hsla(0x059669),
    number: hex_to_hsla(0xd97706),
    keyword: hex_to_hsla(0x86198f),
    null: hex_to_hsla(0xe11d48),
    punctuation: hex_to_hsla(0x64748b),
    diagnostic_error: hex_to_hsla(0xdc2626),
    diagnostic_warning: hex_to_hsla(0xd97706),
    diagnostic_info: hex_to_hsla(0x0891b2),
    diagnostic_success: hex_to_hsla(0x059669),
});

pub(crate) fn editor_scheme() -> &'static EditorColorScheme {
    scheme_for(current_theme_mode())
}

pub(crate) fn scheme_for(mode: ThemeMode) -> &'static EditorColorScheme {
    match mode {
        ThemeMode::Dark => &DARK,
        ThemeMode::Light => &LIGHT,
    }
}

pub(crate) fn apply_editor_scheme(component: &mut Theme, mode: ThemeMode, tooltip_surface: Hsla) {
    let scheme = scheme_for(mode);
    let mut highlight = (*component.highlight_theme).clone();
    if mode == ThemeMode::Dark {
        highlight.style.editor_foreground = Some(scheme.foreground);
        highlight.style.editor_line_number = Some(scheme.line_number);
        highlight.style.editor_active_line_number = Some(scheme.foreground);
        highlight.style.editor_gutter_background = Some(scheme.gutter_background);
    }

    // The kit exposes StatusColors through serde. Opaque backgrounds keep tooltips readable.
    let blend = |color: Hsla, alpha| {
        let rgba = Rgba::from(tooltip_surface.blend(color.alpha(alpha)));
        format!(
            "#{:02x}{:02x}{:02x}",
            (rgba.r * 255.0).round() as u8,
            (rgba.g * 255.0).round() as u8,
            (rgba.b * 255.0).round() as u8
        )
    };
    if let Ok(status) = serde_json::from_value::<StatusColors>(serde_json::json!({
        "error.background": blend(scheme.diagnostic_error, 0.30),
        "warning.background": blend(scheme.diagnostic_warning, 0.22),
        "info.background": blend(scheme.diagnostic_info, 0.18),
        "success.background": blend(scheme.diagnostic_success, 0.18),
    })) {
        highlight.style.status = status;
    }
    component.highlight_theme = Arc::new(highlight);
}
