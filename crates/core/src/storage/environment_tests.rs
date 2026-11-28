use super::*;
use crate::storage::FsWorkspaceStore;

#[test]
fn snapshot_surfaces_the_warning_from_the_read_that_backs_up_corrupt_secrets() {
    let directory = tempfile::tempdir().unwrap();
    let store = FsWorkspaceStore::new(directory.path());
    store.init_workspace("Test").unwrap();
    let secrets = store.secrets_dir().join("local.yaml");
    std::fs::write(&secrets, "invalid: [").unwrap();

    let snapshot = EnvironmentSnapshot::load(&store, "Test");
    assert_eq!(snapshot.environments[0].id, "local");
    assert!(snapshot
        .corrupt_secrets_warning
        .unwrap()
        .contains("local.yaml.bak"));
    assert!(secrets.with_extension("yaml.bak").is_file());
}
