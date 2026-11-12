use super::*;

#[test]
fn test_git_availability() {
    assert!(is_git_available());
}

#[test]
fn test_git_status_in_current_repo() {
    let repo_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = find_git_root(repo_dir);
    assert!(root.is_some());

    let status = git_status(repo_dir).unwrap();
    assert!(status.is_repo);
    assert!(!status.branch.is_empty());
}

#[test]
fn test_git_history_in_current_repo() {
    let repo_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let history = git_history(repo_dir, 5).unwrap();
    assert!(!history.is_empty());
    assert!(!history[0].hash.is_empty());
    assert!(!history[0].short_hash.is_empty());
}

#[test]
fn test_git_branches_in_current_repo() {
    let repo_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let branches = git_branches(repo_dir).unwrap();
    assert!(!branches.is_empty());
    assert!(branches.iter().any(|b| b.is_current));
    // Ensure remote HEAD symbolic refs like "origin" are filtered out
    assert!(!branches
        .iter()
        .any(|b| b.name == "origin" || b.name.ends_with("/HEAD")));
}

#[test]
fn test_git_operations_lifecycle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "test_git_lifecycle_{}",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    // 1. init
    assert!(git_init(&temp_dir).is_ok());
    // Configure author for commit in test environment
    let _ = run_git_cmd(&temp_dir, &["config", "user.name", "Test Runner"]);
    let _ = run_git_cmd(&temp_dir, &["config", "user.email", "test@example.com"]);

    // 2. create file
    let file_path = temp_dir.join("request.yaml");
    std::fs::write(&file_path, "name: Test Request\nmethod: GET\n").unwrap();

    // 3. status - untracked
    let status = git_status(&temp_dir).unwrap();
    assert!(status.is_repo);
    assert_eq!(status.untracked_files.len(), 1);
    assert_eq!(status.untracked_files[0].path, "request.yaml");

    // 4. stage file
    assert!(git_stage_file(&temp_dir, "request.yaml").is_ok());
    let status = git_status(&temp_dir).unwrap();
    assert_eq!(status.staged_files.len(), 1);
    assert_eq!(status.staged_files[0].path, "request.yaml");

    // 5. unstage file
    assert!(git_unstage_file(&temp_dir, "request.yaml").is_ok());
    let status = git_status(&temp_dir).unwrap();
    assert_eq!(status.staged_files.len(), 0);

    // 5b. batch stage and unstage files
    assert!(git_stage_files(&temp_dir, &["request.yaml"]).is_ok());
    let status = git_status(&temp_dir).unwrap();
    assert_eq!(status.staged_files.len(), 1);
    assert!(git_unstage_files(&temp_dir, &["request.yaml"]).is_ok());
    let status = git_status(&temp_dir).unwrap();
    assert_eq!(status.staged_files.len(), 0);

    // 6. stage all and commit
    assert!(git_stage_all(&temp_dir).is_ok());
    assert!(git_commit(&temp_dir, "feat: initial test request").is_ok());

    // 7. history
    let history = git_history(&temp_dir, 10).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].message, "feat: initial test request");

    // 7b. commit only selected files (untracked path is staged implicitly)
    std::fs::write(temp_dir.join("extra.yaml"), "name: Extra\n").unwrap();
    std::fs::write(&file_path, "name: Test Request\nmethod: DELETE\n").unwrap();
    assert!(git_commit_files(&temp_dir, "feat: add extra request", &["extra.yaml"]).is_ok());
    let status = git_status(&temp_dir).unwrap();
    assert!(status
        .unstaged_files
        .iter()
        .any(|f| f.path == "request.yaml"));
    assert!(!status
        .staged_files
        .iter()
        .chain(&status.unstaged_files)
        .chain(&status.untracked_files)
        .any(|f| f.path == "extra.yaml"));
    let history = git_history(&temp_dir, 10).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].message, "feat: add extra request");

    // 8. modify file and check diff
    std::fs::write(&file_path, "name: Test Request\nmethod: POST\n").unwrap();
    let diff = git_diff(&temp_dir, Some("request.yaml"), false).unwrap();
    assert!(diff.contains("-method: GET"));
    assert!(diff.contains("+method: POST"));

    // 9. branch and checkout
    assert!(git_create_branch(&temp_dir, "feature-branch").is_ok());
    let branches = git_branches(&temp_dir).unwrap();
    assert!(branches
        .iter()
        .any(|b| b.name == "feature-branch" && b.is_current));

    // 10. discard modified file
    assert!(git_discard_file(&temp_dir, "request.yaml", false).is_ok());
    let content_after_discard = std::fs::read_to_string(&file_path).unwrap();
    assert!(content_after_discard.contains("method: GET"));

    // 11. discard untracked new file
    let new_untracked = temp_dir.join("new_req.yaml");
    std::fs::write(&new_untracked, "name: Untracked\n").unwrap();
    assert!(new_untracked.is_file());

    // Check diff on untracked new file
    let untracked_diff = git_diff(&temp_dir, Some("new_req.yaml"), false).unwrap();
    assert!(untracked_diff.contains("+name: Untracked"));

    // Workspace-data semantics (delete_untracked = false): file is kept
    assert!(git_discard_file(&temp_dir, "new_req.yaml", false).is_ok());
    assert!(new_untracked.is_file());
    assert!(std::fs::read_to_string(&new_untracked)
        .unwrap()
        .contains("name: Untracked"));

    // Git-panel semantics (delete_untracked = true): untracked file is removed
    assert!(git_discard_file(&temp_dir, "new_req.yaml", true).is_ok());
    assert!(!new_untracked.exists());

    // 11b. discard path validation
    assert!(git_discard_file(&temp_dir, "", false).is_err());
    assert!(git_discard_file(&temp_dir, "../outside.yaml", false).is_err());
    assert!(git_discard_file(&temp_dir, "/abs/path.yaml", false).is_err());

    // 11c. discard staged-added file (in index, not in HEAD)
    let staged_added = temp_dir.join("staged_added.yaml");
    std::fs::write(&staged_added, "name: Staged Added\n").unwrap();
    assert!(git_stage_files(&temp_dir, &["staged_added.yaml"]).is_ok());
    assert!(git_status(&temp_dir)
        .unwrap()
        .staged_files
        .iter()
        .any(|f| f.path == "staged_added.yaml"));

    // delete_untracked = false: unstage, but keep file on disk
    assert!(git_discard_file(&temp_dir, "staged_added.yaml", false).is_ok());
    assert!(staged_added.is_file());
    let st = git_status(&temp_dir).unwrap();
    assert!(!st
        .staged_files
        .iter()
        .any(|f| f.path == "staged_added.yaml"));
    assert!(st
        .untracked_files
        .iter()
        .any(|f| f.path == "staged_added.yaml"));

    // Re-stage and discard with delete_untracked = true: unstage and remove file
    assert!(git_stage_files(&temp_dir, &["staged_added.yaml"]).is_ok());
    assert!(git_discard_file(&temp_dir, "staged_added.yaml", true).is_ok());
    assert!(!staged_added.exists());

    // 12. remote management
    assert!(git_add_remote(&temp_dir, "origin", "https://github.com/test/repo.git").is_ok());
    let remotes = git_get_remotes(&temp_dir).unwrap();
    assert_eq!(remotes.len(), 1);
    assert_eq!(remotes[0].name, "origin");
    assert_eq!(remotes[0].fetch_url, "https://github.com/test/repo.git");

    assert!(
        git_set_remote_url(&temp_dir, "origin", "https://github.com/test/new-repo.git").is_ok()
    );
    let remotes = git_get_remotes(&temp_dir).unwrap();
    assert_eq!(remotes[0].fetch_url, "https://github.com/test/new-repo.git");

    assert!(git_remove_remote(&temp_dir, "origin").is_ok());
    let remotes = git_get_remotes(&temp_dir).unwrap();
    assert!(remotes.is_empty());

    // 13. detect_git_repo_fast
    let fast = detect_git_repo_fast(&temp_dir);
    assert!(fast.is_some());
    let (root, branch) = fast.unwrap();
    assert_eq!(root, temp_dir);
    assert!(branch.is_some());

    // Non-repo path
    let non_repo = std::env::temp_dir().join(format!("non_repo_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&non_repo);
    assert!(detect_git_repo_fast(&non_repo).is_none());
    let _ = std::fs::remove_dir_all(&non_repo);

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}
