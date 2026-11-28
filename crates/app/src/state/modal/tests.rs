use super::{ModalKind, ModalPayload, ModalState, SettingsTab, WorkspaceSettingTarget};

#[test]
fn test_modal_state_defaults() {
    let mut modals = ModalState::default();
    assert!(modals.active_modal.is_none());
    assert!(!modals.has_any_modal_open());
    assert_eq!(modals.settings_tab, SettingsTab::General);
    assert!(modals.cookie_filter_query.is_empty());

    modals.active_modal = Some(crate::state::ModalKind::Settings);
    modals.settings_tab = SettingsTab::Cookies;
    modals.workspace_setting_target = Some(WorkspaceSettingTarget::Workspace);
    assert!(modals.has_any_modal_open());
    assert_eq!(modals.settings_tab, SettingsTab::Cookies);
    assert_eq!(
        modals.workspace_setting_target,
        Some(WorkspaceSettingTarget::Workspace)
    );
}

#[gpui_kit::test]
fn test_modal_state_switches_one_active_modal(cx: &mut gpui_kit::TestAppContext) {
    use gpui_kit::AppContext as _;

    let modal_state = cx.new(|_| ModalState::default());

    modal_state.update(cx, |modal, cx| modal.open(ModalKind::Settings, None, cx));
    assert!(modal_state.read_with(cx, |modal, _| {
        modal.active_modal == Some(crate::state::ModalKind::Settings)
    }));

    modal_state.update(cx, |modal, cx| {
        modal.open(
            ModalKind::Import,
            Some(ModalPayload::Import {
                target_folder_rel: Some("folder".to_string()),
            }),
            cx,
        )
    });
    assert!(modal_state.read_with(cx, |modal, _| {
        modal.active_modal == Some(crate::state::ModalKind::Import)
            && modal.import_modal.target_folder_rel.as_deref() == Some("folder")
    }));

    modal_state.update(cx, |modal, cx| {
        assert!(modal.dismiss_top_modal(cx));
    });
    assert!(modal_state.read_with(cx, |modal, _| {
        modal.active_modal.is_none()
            && modal.import_modal.target_folder_rel.is_none()
            && !modal.has_any_modal_open()
    }));
}
