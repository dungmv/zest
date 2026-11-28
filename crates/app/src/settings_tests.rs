use super::*;

#[test]
fn test_theme_preference_loads_from_persisted_settings() {
    let temp_cfg =
        std::env::temp_dir().join(format!("zest_test_cfg_{}.json", uuid::Uuid::new_v4()));
    crate::settings::set_test_config_path(Some(temp_cfg.clone()));

    let mut settings = crate::settings::AppSettings::load();
    settings.theme = Some("dark".to_string());
    let _ = settings.save();
    let preferences = AppPreferences::from_settings(&AppSettings::load());
    assert_eq!(preferences.theme, crate::theme::ThemePreference::Dark);

    settings.theme = Some("light".to_string());
    let _ = settings.save();
    let preferences = AppPreferences::from_settings(&AppSettings::load());
    assert_eq!(preferences.theme, crate::theme::ThemePreference::Light);

    crate::settings::set_test_config_path(None);
    let _ = std::fs::remove_file(temp_cfg);
}
