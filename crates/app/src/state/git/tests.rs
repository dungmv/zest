use super::{GitActiveModal, GitState, GitTab};

#[test]
fn test_git_state_defaults_and_toggle() {
    let mut git = GitState::new("/tmp/test-repo".into());
    assert_eq!(git.modal.tab, GitTab::Changes);
    assert!(!git.modal.is_open);
    assert!(git.modal.active.is_none());
    assert!(!git.operation.is_loading());
    assert!(!git.operation.is_syncing());
    assert!(git.operation.error().is_none());

    git.modal.active = Some(GitActiveModal::Commit);
    git.modal.is_open = true;
    assert!(git.modal.is_open);
    assert_eq!(git.modal.active, Some(GitActiveModal::Commit));

    // Test loading & syncing flag states
    git.operation.start_command();
    assert!(git.operation.is_loading());
    git.operation.start_sync("pushing...");
    assert!(git.operation.is_syncing());
    assert_eq!(git.operation.sync_message(), Some("pushing..."));

    // Test error recording
    git.operation
        .set_error("Git authentication failed".to_string());
    assert_eq!(git.operation.error(), Some("Git authentication failed"));
    git.operation.clear_error();
    assert!(git.operation.error().is_none());
}
