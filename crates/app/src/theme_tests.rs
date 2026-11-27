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
