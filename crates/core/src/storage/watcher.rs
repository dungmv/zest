use anyhow::Result;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FsEvent {
    RequestChanged(String),     // rel_path of request e.g. "auth/login.yaml"
    RequestDeleted(String),     // rel_path of request
    EnvironmentChanged(String), // env name
    TreeChanged,                // collections folder structure or workspace.yaml
}

pub struct WorkspaceWatcher {
    _watcher: RecommendedWatcher,
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

fn strip_unc_str(s: &str) -> &str {
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        stripped
    } else {
        s
    }
}

fn strip_unc_prefix(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        p.to_path_buf()
    }
}

fn path_matches(p1: &Path, p2: &Path) -> bool {
    let s1 = p1.to_string_lossy();
    let s2 = p2.to_string_lossy();
    let t1 = strip_unc_str(&s1);
    let t2 = strip_unc_str(&s2);

    let normalize_ch = |c: char| -> char {
        if c == '/' {
            '\\'
        } else {
            c.to_ascii_lowercase()
        }
    };

    t1.chars()
        .map(normalize_ch)
        .eq(t2.chars().map(normalize_ch))
}

pub fn start_workspace_watcher(
    root_dir: &Path,
    suppressed_paths: Arc<Mutex<HashSet<PathBuf>>>,
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
                        // Check if path is in suppressed set (normalized for Windows UNC prefixes and case)
                        if let Ok(set) = suppressed_paths.lock() {
                            if set.iter().any(|p| path_matches(p, path)) {
                                continue;
                            }
                        }

                        let clean_path = strip_unc_prefix(path);
                        let Ok(rel) = clean_path.strip_prefix(&root_buf) else {
                            continue;
                        };
                        let rel_str = rel.to_string_lossy().replace('\\', "/");

                        // Ignore internal / non-collection files
                        if rel_str.starts_with(".git")
                            || rel_str.starts_with(".local")
                            || rel_str.ends_with(".loc")
                            || rel_str.ends_with(".tmp")
                            || rel_str.ends_with(".bak")
                        {
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
}
