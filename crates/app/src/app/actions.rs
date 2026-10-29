use gpui_kit::actions;

actions!(
    zest,
    [
        SendRequest,
        ToggleCommandPalette,
        ToggleSidebar,
        ToggleSettings,
        ToggleGitModal,
        OpenImportModal,
        NavigateBack,
        NavigateForward,
        DismissModal,
    ]
);
