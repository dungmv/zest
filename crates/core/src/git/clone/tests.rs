use super::*;
use crate::git::{git_get_remotes, git_status, run_git_cmd};
use tempfile::TempDir;

fn source_repo(metadata: Option<&str>) -> TempDir {
    let source = tempfile::tempdir().unwrap();
    run_git_cmd(source.path(), &["init", "--initial-branch=team-default"]).unwrap();
    fs::write(source.path().join("README.md"), "Team repository\n").unwrap();
    if let Some(metadata) = metadata {
        fs::write(source.path().join("workspace.yaml"), metadata).unwrap();
    }
    run_git_cmd(source.path(), &["add", "."]).unwrap();
    commit(source.path());
    source
}

fn commit(repo: &Path) {
    run_git_cmd(
        repo,
        &[
            "-c",
            "user.name=Zest Tests",
            "-c",
            "user.email=zest@example.test",
            "-c",
            "commit.gpgSign=false",
            "commit",
            "-m",
            "Workspace fixture",
        ],
    )
    .unwrap();
}

fn clone_local(
    source: &Path,
    destination: &Path,
    cancel: &CloneCancellation,
) -> Result<PathBuf, CloneWorkspaceError> {
    clone_from_source(source.to_str().unwrap(), destination, cancel, "file")
}

#[test]
fn accepts_https_and_ssh_clone_urls_without_secrets() {
    for source in [
        "https://git.example.test/team/workspace.git",
        "ssh://git@git.example.test:2222/team/workspace.git",
        "git@git.example.test:team/workspace.git",
        "git@git.example.test:/srv/workspace.git",
        "work-git:team/workspace.git",
        "git@[::1]:team/workspace.git",
    ] {
        assert!(validate_source(source).is_ok(), "{source}");
    }
    for source in [
        "https://token@git.example.test/team/workspace.git",
        "https://git.example.test/team/workspace.git?token=secret",
        "ssh://git:password@git.example.test/team/workspace.git",
        "git:password@host:team/workspace.git",
        "https:///git.example.test/team/workspace.git",
        "https://git.example.test",
        "file:///tmp/workspace",
        "http://git.example.test/workspace",
        "ext::some-command",
        "-oProxyCommand=helper:workspace",
    ] {
        assert!(validate_source(source).is_err(), "{source}");
    }
}

#[test]
fn derives_repository_folder_from_supported_url_forms() {
    let parent = Path::new("parent");
    for source in [
        "git@example.test:team/apidocs.git",
        "git@example.test:apidocs.git",
        "git@[::1]:apidocs.git",
        "ssh://git@example.test:2222/team/apidocs.git/",
        "https://example.test/team/apidocs",
        "https://example.test/team/apidocs.git",
        "https://example.test/team/apidocs/.git/",
    ] {
        validate_source(source).unwrap();
        assert_eq!(
            clone_destination(source, parent).unwrap(),
            parent.join("apidocs")
        );
    }
    for source in ["git@example.test:..", "git@example.test:.git"] {
        assert!(matches!(
            clone_destination(source, parent),
            Err(CloneWorkspaceError::InvalidRepositoryName)
        ));
    }
}

#[test]
fn clones_default_branch_with_origin_and_valid_empty_workspace() {
    let source = source_repo(Some("name: Team workspace\n"));
    let parent = tempfile::tempdir().unwrap();
    let existing_file = parent.path().join("keep.txt");
    fs::write(&existing_file, "existing data").unwrap();
    for name in ["apidocs", "existing-empty"] {
        let url = format!("git@example.test:team/{name}.git");
        let destination = clone_destination(&url, parent.path()).unwrap();
        if name == "existing-empty" {
            fs::create_dir(&destination).unwrap();
        }
        let path = clone_local(source.path(), &destination, &CloneCancellation::default())
            .expect("valid checkout");
        let store = FsWorkspaceStore::new(&path);
        assert_eq!(store.load_workspace_meta().unwrap().name, "Team workspace");
        assert_eq!(store.scan_tree().unwrap().count_requests(), 0);
        assert_eq!(git_status(&path).unwrap().branch, "team-default");
        assert_eq!(git_get_remotes(&path).unwrap()[0].name, "origin");
        assert_eq!(path, destination.canonicalize().unwrap());
        assert!(matches!(
            clone_workspace(&url, parent.path(), &CloneCancellation::default()),
            Err(CloneWorkspaceError::DestinationOccupied)
        ));
    }
    assert_eq!(fs::read_to_string(existing_file).unwrap(), "existing data");
    assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 3);
}

#[test]
fn rejects_missing_or_invalid_metadata_without_publishing() {
    let parent = tempfile::tempdir().unwrap();
    let destination = parent.path().join("empty");
    fs::create_dir(&destination).unwrap();
    for metadata in [None, Some("name: Team\nunexpected_field: true\n")] {
        let source = source_repo(metadata);
        let result = clone_local(source.path(), &destination, &CloneCancellation::default());
        if metadata.is_some() {
            assert!(matches!(result, Err(CloneWorkspaceError::InvalidMetadata)));
        } else {
            assert!(matches!(result, Err(CloneWorkspaceError::MissingMetadata)));
        }
        assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
        assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 1);
    }
}

#[test]
fn occupied_destination_and_cancelled_clone_preserve_local_data() {
    let source = source_repo(Some("name: Team\n"));
    let parent = tempfile::tempdir().unwrap();
    let destination = parent.path().join("occupied");
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join("keep.txt"), "existing data").unwrap();
    let cancel = CloneCancellation::default();
    assert!(matches!(
        clone_local(source.path(), &destination, &cancel),
        Err(CloneWorkspaceError::DestinationOccupied)
    ));
    assert_eq!(
        fs::read_to_string(destination.join("keep.txt")).unwrap(),
        "existing data"
    );
    cancel.cancel();
    let new_destination = parent.path().join("new");
    assert!(matches!(
        clone_local(source.path(), &new_destination, &cancel),
        Err(CloneWorkspaceError::Cancelled)
    ));
    assert!(!new_destination.exists());
    assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 1);
}

#[test]
fn rejects_local_only_data_in_checkout() {
    let source = source_repo(Some("name: Team\n"));
    fs::create_dir(source.path().join(".local")).unwrap();
    fs::write(source.path().join(".local/state.yaml"), "local state").unwrap();
    run_git_cmd(source.path(), &["add", "-f", ".local/state.yaml"]).unwrap();
    commit(source.path());
    let parent = tempfile::tempdir().unwrap();
    assert!(matches!(
        clone_local(
            source.path(),
            &parent.path().join("destination"),
            &CloneCancellation::default(),
        ),
        Err(CloneWorkspaceError::UnsafeLayout)
    ));
    assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
}
