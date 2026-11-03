use crate::storage::fingerprints::{strip_unc_prefix, DiskFingerprints};
use anyhow::Result;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::Path;
use std::sync::{mpsc, Arc};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FsEvent {
    RequestChanged(String),     // rel_path of request e.g. "auth/login.yaml"
    RequestDeleted(String),     // rel_path of request
    EnvironmentChanged(String), // env name
    TreeChanged,                // collections folder structure or workspace.yaml
    GitChanged,                 // Git index, HEAD, refs, or repository status changed
}

pub struct WorkspaceWatcher {
    _watcher: RecommendedWatcher,
}

/// Dot-prefixed files are internal artifacts (atomic-write fallback temp files
/// `.tmp_<uuid>_<name>.yaml`, editor swap files, `.DS_Store`) and never
/// represent user-visible workspace content.
pub fn is_hidden_artifact(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with('.'))
}

/// A Remove event may be followed by the same file being recreated (atomic
/// replace, e.g. `git restore` or an editor save-by-rename). At drain time the
/// filesystem is the source of truth: a pending deletion whose file still
/// exists is downgraded to a change so consumers never drop a live request.
fn downgrade_existing_deletions(pending: &mut HashSet<FsEvent>, root: &Path) {
    let deleted: Vec<String> = pending
        .iter()
        .filter_map(|ev| match ev {
            FsEvent::RequestDeleted(rel) => Some(rel.clone()),
            _ => None,
        })
        .collect();
    for rel in deleted {
        if root.join("collections").join(&rel).is_file() {
            pending.remove(&FsEvent::RequestDeleted(rel.clone()));
            pending.insert(FsEvent::RequestChanged(rel));
        }
    }
}

pub fn start_workspace_watcher(
    root_dir: &Path,
    fingerprints: Arc<DiskFingerprints>,
    event_sender: UnboundedSender<FsEvent>,
) -> Result<WorkspaceWatcher> {
    let (raw_tx, raw_rx) = mpsc::channel::<Event>();

    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                let _ = raw_tx.send(event);
            }
        },
        Config::default(),
    )?;

    watcher.watch(root_dir, RecursiveMode::Recursive)?;

    let root_buf = strip_unc_prefix(root_dir);
    let canon_root_buf = root_dir
        .canonicalize()
        .map(|p| strip_unc_prefix(&p))
        .unwrap_or_else(|_| root_buf.clone());
    std::thread::Builder::new()
        .name("workspace-watcher-debouncer".to_string())
        .spawn(move || {
            let mut pending_events: HashSet<FsEvent> = HashSet::new();

            loop {
                let next_event = if pending_events.is_empty() {
                    match raw_rx.recv() {
                        Ok(event) => Some(event),
                        Err(_) => break,
                    }
                } else {
                    match raw_rx.recv_timeout(Duration::from_millis(200)) {
                        Ok(event) => Some(event),
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            downgrade_existing_deletions(&mut pending_events, &root_buf);
                            for ev in pending_events.drain() {
                                if event_sender.send(ev).is_err() {
                                    return;
                                }
                            }
                            None
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            downgrade_existing_deletions(&mut pending_events, &root_buf);
                            for ev in pending_events.drain() {
                                let _ = event_sender.send(ev);
                            }
                            break;
                        }
                    }
                };

                if let Some(event) = next_event {
                    for path in &event.paths {
                        let clean_path = strip_unc_prefix(path);
                        let Ok(rel) = clean_path
                            .strip_prefix(&root_buf)
                            .or_else(|_| clean_path.strip_prefix(&canon_root_buf))
                        else {
                            continue;
                        };
                        let rel_str = rel.to_string_lossy().replace('\\', "/");

                        // Handle Git metadata changes (index, HEAD, refs, FETCH_HEAD, etc.)
                        // Skip heavy object churn, logs, and lock files.
                        if rel_str == ".git" || rel_str.starts_with(".git/") {
                            if !rel_str.ends_with(".lock")
                                && !rel_str.starts_with(".git/objects")
                                && !rel_str.starts_with(".git/logs")
                            {
                                pending_events.insert(FsEvent::GitChanged);
                            }
                            continue;
                        }

                        // Ignore internal / non-collection files and dot-prefixed artifacts
                        if rel_str.starts_with(".local")
                            || rel_str.ends_with(".loc")
                            || rel_str.ends_with(".tmp")
                            || rel_str.ends_with(".bak")
                            || is_hidden_artifact(&clean_path)
                        {
                            continue;
                        }

                        // Drop self-echo events whose on-disk state matches the
                        // fingerprint recorded by our own reads/writes.
                        if fingerprints.matches_disk(path) {
                            continue;
                        }

                        if rel_str == "workspace.yaml" {
                            pending_events.insert(FsEvent::TreeChanged);
                        } else if rel_str.starts_with("environments/") && rel_str.ends_with(".yaml")
                        {
                            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                                pending_events
                                    .insert(FsEvent::EnvironmentChanged(stem.to_string()));
                            }
                        } else if rel_str.starts_with("collections/") {
                            let sub_rel = rel_str.trim_start_matches("collections/");
                            if sub_rel.ends_with("_folder.yaml") {
                                pending_events.insert(FsEvent::TreeChanged);
                            } else if sub_rel.ends_with(".yaml") {
                                match event.kind {
                                    EventKind::Remove(_) => {
                                        pending_events
                                            .insert(FsEvent::RequestDeleted(sub_rel.to_string()));
                                    }
                                    _ => {
                                        pending_events
                                            .insert(FsEvent::RequestChanged(sub_rel.to_string()));
                                    }
                                }
                            } else if path.is_dir() {
                                match event.kind {
                                    EventKind::Create(_) | EventKind::Remove(_) => {
                                        pending_events.insert(FsEvent::TreeChanged);
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        })?;

    Ok(WorkspaceWatcher { _watcher: watcher })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::fingerprints::path_matches;
    use crate::storage::{EnvironmentFile, FsWorkspaceStore, WorkspaceStore};

    #[test]
    fn test_path_matches_cross_platform() {
        assert!(path_matches(
            Path::new(r"C:\workspace\collections\req.yaml"),
            Path::new("C:/workspace/collections/req.yaml")
        ));
        assert!(path_matches(
            Path::new(r"\\?\C:\workspace\collections\req.yaml"),
            Path::new(r"C:\Workspace\Collections\Req.yaml")
        ));
        assert!(!path_matches(
            Path::new(r"C:\workspace\collections\req.yaml"),
            Path::new(r"C:\workspace\collections\other.yaml")
        ));
    }

    #[test]
    fn test_is_hidden_artifact() {
        assert!(is_hidden_artifact(Path::new(
            "environments/.tmp_123_dev.yaml"
        )));
        assert!(is_hidden_artifact(Path::new("collections/.DS_Store")));
        assert!(is_hidden_artifact(Path::new("collections/.req.yaml.swp")));
        assert!(!is_hidden_artifact(Path::new("environments/dev.yaml")));
        assert!(!is_hidden_artifact(Path::new(
            "collections/users/_folder.yaml"
        )));
    }

    #[test]
    fn test_existing_deletions_downgraded_to_changes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("collections")).unwrap();
        std::fs::write(dir.path().join("collections").join("a.yaml"), "name: A\n").unwrap();

        let mut pending = HashSet::new();
        pending.insert(FsEvent::RequestDeleted("a.yaml".to_string()));
        pending.insert(FsEvent::RequestDeleted("gone.yaml".to_string()));
        downgrade_existing_deletions(&mut pending, dir.path());

        // File replaced on disk (remove + recreate): must surface as a change
        assert!(pending.contains(&FsEvent::RequestChanged("a.yaml".to_string())));
        assert!(!pending.contains(&FsEvent::RequestDeleted("a.yaml".to_string())));
        // File truly gone: stays a deletion
        assert!(pending.contains(&FsEvent::RequestDeleted("gone.yaml".to_string())));
    }

    #[tokio::test]
    async fn test_watcher_git_and_collections_events() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(".git").join("refs").join("heads")).unwrap();
        std::fs::create_dir_all(root.join(".git").join("objects")).unwrap();
        std::fs::create_dir_all(root.join("collections")).unwrap();

        let fp = Arc::new(DiskFingerprints::new());
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let _watcher = start_workspace_watcher(root, fp, tx).unwrap();

        // 1. Modify git index
        std::fs::write(root.join(".git").join("index"), "fake index").unwrap();
        // 2. Modify git objects (should be ignored)
        std::fs::write(root.join(".git").join("objects").join("abc"), "blob").unwrap();
        // 3. Modify git lock (should be ignored)
        std::fs::write(root.join(".git").join("index.lock"), "lock").unwrap();
        // 4. Modify a request externally
        std::fs::write(root.join("collections").join("req.yaml"), "name: Req\n").unwrap();

        let mut received = HashSet::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            if let Ok(Some(event)) =
                tokio::time::timeout(Duration::from_millis(400), rx.recv()).await
            {
                received.insert(event);
                if received.contains(&FsEvent::GitChanged)
                    && received.contains(&FsEvent::RequestChanged("req.yaml".to_string()))
                {
                    break;
                }
            }
        }

        assert!(
            received.contains(&FsEvent::GitChanged),
            "Expected GitChanged event"
        );
        assert!(
            received.contains(&FsEvent::RequestChanged("req.yaml".to_string())),
            "Expected RequestChanged event"
        );
    }

    #[tokio::test]
    async fn test_watcher_ignores_self_writes_and_hidden_files_but_emits_external_changes() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsWorkspaceStore::new(dir.path());
        store.init_workspace("WatcherSelfEcho").unwrap();

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let _watcher = start_workspace_watcher(dir.path(), store.disk_fingerprints(), tx).unwrap();

        // Allow watcher to settle after init
        tokio::time::sleep(Duration::from_millis(250)).await;
        while rx.try_recv().is_ok() {}

        // 1. App saves environment, workspace meta, and hidden temp file -> must produce ZERO events
        let mut env = EnvironmentFile::default();
        env.variables.push(crate::storage::models::EnvVarFile {
            name: "API_URL".to_string(),
            value: Some("https://api.example.com".to_string()),
            secret: false,
            enabled: true,
        });
        store.save_environment("dev", &env).unwrap();
        store
            .save_workspace_meta(&crate::storage::models::WorkspaceMeta {
                version: 1,
                name: "UpdatedName".to_string(),
                headers: Vec::new(),
                auth: crate::storage::models::RequestAuth::None,
            })
            .unwrap();
        std::fs::write(
            dir.path().join("environments").join(".tmp_fake_dev.yaml"),
            "variables: []\n",
        )
        .unwrap();

        let echo = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
        assert!(
            echo.is_err(),
            "Self-writes and dot-files must not emit watcher events, got: {:?}",
            echo
        );

        // 2. External modification to environments/dev.yaml -> MUST emit EnvironmentChanged("dev")
        std::fs::write(
            dir.path().join("environments").join("dev.yaml"),
            "variables:\n  - name: API_URL\n    value: https://changed.example.com\n",
        )
        .unwrap();

        let ext_ev = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("Expected external change event within timeout")
            .expect("Channel open");
        assert_eq!(ext_ev, FsEvent::EnvironmentChanged("dev".to_string()));
    }
}
