use super::*;
use crate::editor_scheme::{editor_scheme, scheme_for};

#[test]
fn test_theme_mode_switching() {
    set_theme(ThemePreference::Light, ThemeMode::Light);
    assert_eq!(current_theme_mode(), ThemeMode::Light);
    assert_eq!(current_theme_preference(), ThemePreference::Light);
    assert_eq!(theme().bg_app, LIGHT_THEME.bg_app);
    assert_eq!(theme().text_primary, hex_to_hsla(0x1e293b));
    assert!(std::ptr::eq(editor_scheme(), scheme_for(ThemeMode::Light)));

    set_theme(ThemePreference::Dark, ThemeMode::Dark);
    assert_eq!(current_theme_mode(), ThemeMode::Dark);
    assert_eq!(current_theme_preference(), ThemePreference::Dark);
    assert_eq!(theme().bg_app, DARK_THEME.bg_app);
    assert_eq!(theme().text_primary, hex_to_hsla(0xcdd3df));
    assert!(std::ptr::eq(editor_scheme(), scheme_for(ThemeMode::Dark)));

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

fn luminance(color: Hsla) -> f32 {
    let Rgba { r, g, b, .. } = Rgba::from(color);
    let linear = |channel: f32| {
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

#[test]
fn dark_sidebar_colors_stay_readable_across_row_states() {
    let dark = AppTheme::dark();
    let icon = luminance(dark.icon_muted);
    let label = luminance(dark.sidebar_foreground);
    let active_label = luminance(dark.text_primary);
    assert!(icon < label && label < active_label);

    for (background, text) in [
        (dark.bg_sidebar, label),
        (dark.bg_hover, label),
        (dark.bg_card, label),
        (dark.bg_selected, active_label),
    ] {
        let background = luminance(background);
        assert!((text + 0.05) / (background + 0.05) >= 4.5);
        assert!((icon + 0.05) / (background + 0.05) >= 3.0);
    }
}

fn contrast(foreground: Hsla, background: Hsla) -> f32 {
    let foreground = luminance(foreground) + 0.05;
    let background = luminance(background) + 0.05;
    foreground.max(background) / foreground.min(background)
}

#[test]
fn editor_scheme_keeps_readable_contrast_on_panel_backgrounds() {
    let dark = AppTheme::dark();
    let scheme = scheme_for(ThemeMode::Dark);
    for background in [dark.bg_editor, dark.bg_response] {
        for foreground in [
            scheme.foreground,
            scheme.key,
            scheme.string,
            scheme.number,
            scheme.null,
            scheme.punctuation,
        ] {
            assert!(contrast(foreground, background) >= 9.0);
        }
        assert!(contrast(scheme.keyword, background) >= 11.0);
    }
    assert!(contrast(scheme.line_number, scheme.gutter_background) >= 7.0);

    let light = AppTheme::light();
    let scheme = scheme_for(ThemeMode::Light);
    for background in [light.bg_editor, light.bg_response] {
        assert!(contrast(scheme.keyword, background) >= 8.0);
    }
}

#[test]
fn dark_ui_colors_keep_readable_contrast() {
    let dark = AppTheme::dark();
    for background in [
        dark.bg_editor,
        dark.bg_response,
        dark.bg_card,
        dark.bg_selected,
    ] {
        for foreground in [
            dark.text_primary,
            dark.text_secondary,
            dark.text_muted,
            dark.accent_text,
        ] {
            assert!(contrast(foreground, background) >= 4.5);
        }
        assert!(contrast(dark.icon_muted, background) >= 3.0);
    }

    for foreground in [
        dark.status_2xx,
        dark.status_3xx,
        dark.status_4xx,
        dark.status_5xx,
    ] {
        let badge = dark.bg_response.blend(foreground.alpha(0.15));
        assert!(contrast(foreground, badge) >= 4.5);
    }

    for (foreground, backgrounds) in [
        (
            dark.primary_foreground,
            [dark.primary, dark.primary_hover, dark.primary_active],
        ),
        (
            dark.danger_foreground,
            [dark.danger, dark.danger_hover, dark.danger_active],
        ),
    ] {
        for background in backgrounds {
            assert!(contrast(foreground, background) >= 4.5);
        }
    }
}
