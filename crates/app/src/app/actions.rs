use gpui_kit::actions;

actions!(
    zest,
    [
        SendRequest,
        ToggleCommandPalette,
        ToggleSidebar,
        ToggleSettings,
        ToggleGitModal,
        ToggleAboutModal,
        OpenImportModal,
        NavigateBack,
        NavigateForward,
        DismissModal,
        // Native application menu (macOS menubar) actions.
        OpenAbout,
        OpenSettings,
        HideApp,
        HideOtherApps,
        ShowAllApps,
        Quit,
    ]
);
