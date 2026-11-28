use super::*;
use crate::state::persistence::tests::setup_test_workspace;
use gpui_kit::AppContext as _;
use zest_core::models::{HttpMethod, Protocol};
use zest_core::storage::{WorkspaceMeta, WorkspaceStore};

#[gpui_kit::test]
fn external_delete_preserves_pending_edits_and_keep_mine_restores_them(
    cx: &mut gpui_kit::TestAppContext,
) {
    for full_rescan in [false, true] {
        let (dir, store) = setup_test_workspace("Delete Conflict");
        let path = dir.path().to_path_buf();
        let (_, id) = store
            .create_request(
                None,
                "Keep me",
                HttpMethod::Get,
                "https://example.test/old",
                Protocol::Http,
            )
            .unwrap();
        let collection = cx.new(|_| CollectionState::new_for_path(path.clone(), store.clone()));
        let environment =
            cx.new(|_| EnvironmentState::new(store.clone(), "Delete Conflict".into(), None));
        let git = cx.new(|_| GitState::new(path.clone()));
        let persistence = cx.new(|_| PersistenceState::new(store.clone(), path.clone()));
        collection.update(cx, |collection, _| {
            collection.mutate_request(&id, |request| {
                request.url = "https://example.test/latest".into()
            });
        });
        persistence.update(cx, |state, cx| {
            state.schedule_save_active_request(id.clone(), collection.clone(), cx);
        });
        std::fs::remove_file(path.join("collections").join(&id)).unwrap();
        persistence.update(cx, |state, cx| {
            let event = if full_rescan {
                FsEvent::TreeChanged
            } else {
                FsEvent::RequestDeleted(id.clone())
            };
            state.handle_fs_event(event, collection.clone(), environment, git, cx);
        });
        cx.run_until_parked();
        assert!(persistence.read_with(cx, |state, _| state.request_conflict(&id).is_some()));
        assert_eq!(
            collection.read_with(cx, |state, _| state.find_request(&id).unwrap().url.clone()),
            "https://example.test/latest"
        );
        persistence.update(cx, |state, cx| {
            state.resolve_conflict_keep(&id, collection.clone(), cx)
        });
        cx.run_until_parked();
        assert_eq!(
            store.read_request(&id).unwrap().url,
            "https://example.test/latest"
        );
    }
}

#[gpui_kit::test]
fn workspace_meta_changes_are_emitted_only_when_clean(cx: &mut gpui_kit::TestAppContext) {
    use std::cell::RefCell;
    use std::rc::Rc;
    use zest_core::storage::FsEvent;

    let (dir, store) = setup_test_workspace("Original");
    let path = dir.path().to_path_buf();
    let collection = cx.new(|_| CollectionState::new_for_path(path.clone(), store.clone()));
    let env = cx.new(|_| EnvironmentState::new(store.clone(), "Original".into(), None));
    let git = cx.new(|_| GitState::new(path.clone()));
    let persistence = cx.new(|_| PersistenceState::new(store.clone(), path));
    let reloads = Rc::new(RefCell::new(Vec::new()));
    let recorded = reloads.clone();
    let _listener = cx.new(|cx| {
        cx.subscribe(&collection, move |_, _, event: &CollectionEvent, _| {
            if let CollectionEvent::WorkspaceMetaReloaded { meta, .. } = event {
                recorded.borrow_mut().push(meta.name.clone());
            }
        })
        .detach();
    });

    store
        .save_workspace_meta(&WorkspaceMeta {
            name: "External".into(),
            ..Default::default()
        })
        .unwrap();
    let dispatch = |cx: &mut gpui_kit::TestAppContext| {
        persistence.update(cx, |state, cx| {
            state.handle_fs_event(
                FsEvent::WorkspaceMetaChanged,
                collection.clone(),
                env.clone(),
                git.clone(),
                cx,
            );
        });
    };
    dispatch(cx);
    assert_eq!(&*reloads.borrow(), &["External"]);

    persistence.update(cx, |state, cx| {
        state.schedule_save_workspace_meta(
            WorkspaceMeta {
                name: "Unsaved".into(),
                ..Default::default()
            },
            cx,
        );
    });
    dispatch(cx);
    assert_eq!(&*reloads.borrow(), &["External"]);
}

#[gpui_kit::test]
fn test_fs_batch_recreated_request_not_dropped(cx: &mut gpui_kit::TestAppContext) {
    use zest_core::storage::FsEvent;

    let (dir, store) = setup_test_workspace("WatcherRecreated");
    let ws_state =
        cx.new(|_| CollectionState::new_for_path(dir.path().to_path_buf(), store.clone()));
    let env_state =
        cx.new(|_| EnvironmentState::new(store.clone(), "WatcherRecreated".to_string(), None));
    let per_state = cx.new(|_| PersistenceState::new(store.clone(), dir.path().to_path_buf()));

    // Create a request file on disk and load it into workspace state
    let req_id = store
        .create_request(
            None,
            "Get Users",
            HttpMethod::Get,
            "https://api.com/users",
            Protocol::Http,
        )
        .unwrap()
        .1;
    ws_state.update(cx, |ws, _| ws.reload_tree());
    let req_file_path = dir.path().join("collections").join(&req_id);
    assert!(req_file_path.is_file());
    let in_memory = |cx: &mut gpui_kit::TestAppContext| {
        ws_state.read_with(cx, |ws, _| ws.find_request(&req_id).is_some())
    };
    assert!(in_memory(cx));

    let git_state = cx.new(|_| GitState::new(dir.path().to_path_buf()));
    let dispatch = |events: Vec<FsEvent>, cx: &mut gpui_kit::TestAppContext| {
        let ws = ws_state.clone();
        let env = env_state.clone();
        let git = git_state.clone();
        per_state.update(cx, |per, cx| {
            per.handle_fs_events_batch(events, ws, env, git, cx);
        });
        // Tree rescans run on the background executor; flush pending tasks so
        // a triggered rescan applies before the caller asserts.
        cx.run_until_parked();
    };

    // 1. Atomic replace in one batch (`git restore`): Remove + Create. The
    //    file still exists, so the request must never be dropped.
    dispatch(
        vec![
            FsEvent::RequestDeleted(req_id.clone()),
            FsEvent::RequestChanged(req_id.clone()),
        ],
        cx,
    );
    assert!(in_memory(cx), "replaced-in-place request was dropped");

    // 2. Split batches: real deletion first...
    let yaml = std::fs::read_to_string(&req_file_path).unwrap();
    std::fs::remove_file(&req_file_path).unwrap();
    dispatch(vec![FsEvent::RequestDeleted(req_id.clone())], cx);
    assert!(!in_memory(cx), "truly deleted request was kept");

    // 3. ...then the file reappears on disk (late `git restore` finish): the
    //    follow-up change event must re-add it via a tree rescan.
    std::fs::write(&req_file_path, yaml).unwrap();
    dispatch(vec![FsEvent::RequestChanged(req_id.clone())], cx);
    assert!(in_memory(cx), "recreated request was not re-added");
}

#[gpui_kit::test]
fn test_external_change_reconciles_clean_vs_dirty(cx: &mut gpui_kit::TestAppContext) {
    use zest_core::storage::FsEvent;

    let (dir, store) = setup_test_workspace("test_reconcile");
    let ws_state =
        cx.new(|_| CollectionState::new_for_path(dir.path().to_path_buf(), store.clone()));
    let per_state = cx.new(|_| PersistenceState::new(store.clone(), dir.path().to_path_buf()));
    let env_state =
        cx.new(|_| EnvironmentState::new(store.clone(), "test_reconcile".to_string(), None));
    let git_state = cx.new(|_| GitState::new(dir.path().to_path_buf()));

    let req_id = store
        .create_request(
            None,
            "Original Name",
            HttpMethod::Get,
            "https://api.com/users",
            Protocol::Http,
        )
        .unwrap()
        .1;
    ws_state.update(cx, |ws, _| ws.reload_tree());

    // 1. Clean request: external edit on disk is automatically reloaded into memory
    let req_file_path = dir.path().join("collections").join(&req_id);
    let mut modified_yaml = std::fs::read_to_string(&req_file_path).unwrap();
    modified_yaml = modified_yaml.replace("Original Name", "External Name");
    std::fs::write(&req_file_path, &modified_yaml).unwrap();

    let ws = ws_state.clone();
    let env = env_state.clone();
    let git = git_state.clone();
    per_state.update(cx, |per, cx| {
        per.handle_fs_events_batch(
            vec![FsEvent::RequestChanged(req_id.clone())],
            ws,
            env,
            git,
            cx,
        );
    });

    let current_name =
        ws_state.read_with(cx, |ws, _| ws.find_request(&req_id).map(|r| r.name.clone()));
    assert_eq!(current_name.as_deref(), Some("External Name"));
    assert!(!per_state.read_with(cx, |per, _| per.changes.requests.contains_key(&req_id)));

    // 2. Dirty request: external edit triggers conflict, keeps in-memory edits intact
    per_state.update(cx, |per, cx| {
        per.mark_dirty(req_id.clone(), cx);
    });
    ws_state.update(cx, |ws, _| {
        if let Some(r) = ws.requests.iter_mut().find(|r| r.id == req_id) {
            r.name = "In-Memory Edits".to_string();
        }
    });

    let mut external_cli_yaml = std::fs::read_to_string(&req_file_path).unwrap();
    external_cli_yaml = external_cli_yaml.replace("External Name", "CLI Branch Edit");
    std::fs::write(&req_file_path, &external_cli_yaml).unwrap();

    let ws2 = ws_state.clone();
    let env2 = env_state.clone();
    let git2 = git_state.clone();
    per_state.update(cx, |per, cx| {
        per.handle_fs_events_batch(
            vec![FsEvent::RequestChanged(req_id.clone())],
            ws2,
            env2,
            git2,
            cx,
        );
    });

    // In-memory edits preserved
    let memory_name =
        ws_state.read_with(cx, |ws, _| ws.find_request(&req_id).map(|r| r.name.clone()));
    assert_eq!(memory_name.as_deref(), Some("In-Memory Edits"));
    // Conflict registered
    assert!(per_state.read_with(cx, |per, _| per.changes.requests.contains_key(&req_id)));

    // 3. Discard cancellation: user discards changes
    per_state.update(cx, |per, cx| {
        per.cancel_request(&req_id, cx);
        per.mark_clean(&req_id, cx);
        per.changes.requests.remove(&req_id);
    });
    assert!(!per_state.read_with(cx, |per, _| per.is_dirty(&req_id)));
    assert!(!per_state.read_with(cx, |per, _| per.changes.requests.contains_key(&req_id)));
}

#[gpui_kit::test]
async fn test_watcher_env_conflict_and_tree_changed_preserves_dirty(
    cx: &mut gpui_kit::TestAppContext,
) {
    use zest_core::models::EnvironmentVariable;
    use zest_core::storage::FsEvent;

    let (dir, store) = setup_test_workspace("test_env_conflict");
    let ws_state =
        cx.new(|_| CollectionState::new_for_path(dir.path().to_path_buf(), store.clone()));
    let per_state = cx.new(|_| PersistenceState::new(store.clone(), dir.path().to_path_buf()));
    let env_state =
        cx.new(|_| EnvironmentState::new(store.clone(), "test_env_conflict".to_string(), None));
    let git_state = cx.new(|_| GitState::new(dir.path().to_path_buf()));

    let req_id = store
        .create_request(
            None,
            "Dirty Request",
            HttpMethod::Get,
            "https://api.com/v1",
            Protocol::Http,
        )
        .unwrap()
        .1;
    ws_state.update(cx, |ws, _| ws.reload_tree());

    // 1. User edits request in memory and marks it dirty, then TreeChanged fires
    per_state.update(cx, |per, cx| per.mark_dirty(req_id.clone(), cx));
    ws_state.update(cx, |ws, _| {
        if let Some(r) = ws.requests.iter_mut().find(|r| r.id == req_id) {
            r.url = "https://api.com/v2-unsaved".to_string();
        }
    });

    let ws1 = ws_state.clone();
    let env1 = env_state.clone();
    let git1 = git_state.clone();
    per_state.update(cx, |per, cx| {
        per.handle_fs_events_batch(vec![FsEvent::TreeChanged], ws1, env1, git1, cx);
    });

    let preserved_url =
        ws_state.read_with(cx, |ws, _| ws.find_request(&req_id).map(|r| r.url.clone()));
    assert_eq!(preserved_url.as_deref(), Some("https://api.com/v2-unsaved"));

    // 2. User edits `local` environment and `schedule_save_environment` is pending
    env_state.update(cx, |env, cx| {
        env.add_variable("local", cx);
        env.update_variable_key("local", 0, "TOKEN".to_string(), cx);
        env.update_variable_current_value("local", 0, "typing-fast-value".to_string(), cx);
    });
    let local_snapshot = env_state.read_with(cx, |env, _| env.environments[0].clone());
    per_state.update(cx, |per, cx| {
        per.schedule_save_environment("local".to_string(), local_snapshot, cx);
    });

    // External modification to environments/local.yaml while save is pending
    std::fs::write(
        dir.path().join("environments").join("local.yaml"),
        "variables:\n  - name: TOKEN\n    value: external-disk-value\n    secret: false\n    enabled: true\n",
    )
    .unwrap();

    let ws2 = ws_state.clone();
    let env2 = env_state.clone();
    let git2 = git_state.clone();
    per_state.update(cx, |per, cx| {
        per.handle_fs_events_batch(
            vec![FsEvent::EnvironmentChanged("local".to_string())],
            ws2,
            env2,
            git2,
            cx,
        );
    });

    // In-memory value must NOT be overwritten, and env_conflicts must record "local"
    let mem_val = env_state.read_with(cx, |env, _| {
        env.environments
            .iter()
            .find(|e| e.name == "local")
            .unwrap()
            .variables[0]
            .current_value
            .clone()
    });
    assert_eq!(mem_val, "typing-fast-value");
    assert!(per_state.read_with(cx, |per, _| per.changes.environments.contains_key("local")));

    // 3. Resolve conflict with "Reload" -> loads external disk value and clears conflict
    let env3 = env_state.clone();
    per_state.update(cx, |per, cx| {
        per.resolve_env_conflict_reload("local", env3, cx);
    });
    cx.run_until_parked();
    assert!(!per_state.read_with(cx, |per, _| per.changes.environments.contains_key("local")));
    let reloaded_val = env_state.read_with(cx, |env, _| {
        env.environments
            .iter()
            .find(|e| e.name == "local")
            .unwrap()
            .variables[0]
            .initial_value
            .clone()
    });
    assert_eq!(reloaded_val, "external-disk-value");

    // 4.Globals conflict + "Keep mine" resolution
    env_state.update(cx, |env, _| {
        env.global_environment.variables.push(EnvironmentVariable {
            key: "G".to_string(),
            initial_value: "mine".to_string(),
            current_value: "mine".to_string(),
            secret: false,
        });
    });
    let glob_snap = env_state.read_with(cx, |env, _| env.global_environment.clone());
    per_state.update(cx, |per, cx| {
        per.schedule_save_globals(glob_snap, cx);
        per.changes
            .environments
            .insert("globals".to_string(), "conflict".to_string());
    });
    let env4 = env_state.clone();
    per_state.update(cx, |per, cx| {
        per.resolve_env_conflict_keep("globals", env4, cx);
    });
    assert!(!per_state.read_with(cx, |per, _| per
        .changes
        .environments
        .contains_key("globals")));
}

#[gpui_kit::test]
fn stale_tree_scan_cannot_apply_after_workspace_switch(cx: &mut gpui_kit::TestAppContext) {
    let (old_dir, old_store) = setup_test_workspace("Old");
    let (new_dir, new_store) = setup_test_workspace("New");
    old_store
        .create_request(None, "Old Request", HttpMethod::Get, "", Protocol::Http)
        .unwrap();
    new_store
        .create_request(None, "New Request", HttpMethod::Get, "", Protocol::Http)
        .unwrap();
    let collection =
        cx.new(|_| CollectionState::new_for_path(old_dir.path().to_path_buf(), old_store.clone()));
    let persistence = cx.new(|_| PersistenceState::new(old_store, old_dir.path().to_path_buf()));
    collection.update(cx, |collection, cx| {
        collection.reload_tree_preserving_async(persistence.downgrade(), Default::default(), cx);
        collection.switch_workspace(new_store, new_dir.path().to_path_buf(), "New".into(), cx);
    });

    cx.run_until_parked();
    collection.read_with(cx, |collection, _| {
        assert_eq!(collection.requests.len(), 1);
        assert_eq!(collection.requests[0].name, "New Request");
    });
}

#[gpui_kit::test]
fn overlapping_rescans_keep_the_latest_request_edit(cx: &mut gpui_kit::TestAppContext) {
    let (dir, store) = setup_test_workspace("Rescan");
    let (_, id) = store
        .create_request(
            None,
            "Request",
            HttpMethod::Get,
            "https://example.test/disk",
            Protocol::Http,
        )
        .unwrap();
    let collection =
        cx.new(|_| CollectionState::new_for_path(dir.path().to_path_buf(), store.clone()));
    let persistence = cx.new(|_| PersistenceState::new(store, dir.path().to_path_buf()));
    persistence.update(cx, |persistence, cx| persistence.mark_dirty(id.clone(), cx));
    collection.update(cx, |collection, cx| {
        collection.reload_tree_preserving_async(persistence.downgrade(), Default::default(), cx);
        collection.find_request_mut(&id).unwrap().url = "https://example.test/edited".into();
        collection.reload_tree_preserving_async(persistence.downgrade(), Default::default(), cx);
    });

    cx.run_until_parked();
    collection.read_with(cx, |collection, _| {
        assert_eq!(
            collection.find_request(&id).unwrap().url,
            "https://example.test/edited"
        );
    });
}

#[gpui_kit::test]
fn watcher_applies_import_reconciliation_on_background_tasks(cx: &mut gpui_kit::TestAppContext) {
    let (dir, store) = setup_test_workspace("Import");
    let collection =
        cx.new(|_| CollectionState::new_for_path(dir.path().to_path_buf(), store.clone()));
    let env = cx.new(|_| EnvironmentState::new(store.clone(), "Import".into(), None));
    let git = cx.new(|_| GitState::new(dir.path().to_path_buf()));
    let persistence = cx.new(|_| PersistenceState::new(store.clone(), dir.path().to_path_buf()));
    persistence.update(cx, |persistence, cx| {
        persistence.start_watcher(collection.clone(), env.clone(), git, cx)
    });
    let (_, id) = store
        .create_request(None, "Imported", HttpMethod::Get, "", Protocol::Http)
        .unwrap();
    store
        .save_environment("imported", &Default::default())
        .unwrap();
    persistence.read_with(cx, |persistence, _| {
        persistence.reconcile_import(vec!["imported".into()])
    });

    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    assert!(collection.read_with(cx, |collection, _| collection.find_request(&id).is_some()));
    assert!(env.read_with(cx, |env, _| env
        .environments
        .iter()
        .any(|env| env.name == "imported")));
}
