use std::collections::HashMap;
use std::time::Duration;
use zest_core::models::{Environment, HttpRequestItem};
use zest_core::storage::models::{
    EnvironmentFile, FolderFile, LocalState, RequestFile, WorkspaceMeta,
};
use zest_core::storage::WorkspaceStore;

/// Key identifying a persistence target within a workspace.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PersistTarget {
    Request(String),     // rel_path, e.g. "auth/login.yaml"
    Environment(String), // env_name, e.g. "dev"
    Globals,
    WorkspaceMeta,
    FolderMeta(String), // folder_rel, e.g. "auth"
    Pins,
    LocalState,
}

impl PersistTarget {
    pub fn key(&self) -> String {
        match self {
            Self::Request(p) => format!("req:{}", p),
            Self::Environment(n) => format!("env:{}", n),
            Self::Globals => "globals".to_string(),
            Self::WorkspaceMeta => "ws_meta".to_string(),
            Self::FolderMeta(f) => format!("folder:{}", f),
            Self::Pins => "pins".to_string(),
            Self::LocalState => "local_state".to_string(),
        }
    }
}

/// Immutable snapshot payload captured at mutation time and serialized on background threads.
#[derive(Debug, Clone)]
pub enum PersistPayload {
    Request(Box<HttpRequestItem>),
    Environment {
        name: String,
        env: Box<Environment>,
    },
    Globals(Box<Environment>),
    WorkspaceMeta(Box<WorkspaceMeta>),
    FolderMeta {
        folder_rel: String,
        meta: Box<FolderFile>,
    },
    Pins(Vec<String>),
    LocalState(Box<LocalState>),
}

impl PersistPayload {
    /// Executes atomic write to the workspace store on a background worker thread.
    /// Returns Ok(()) only when all files of the logical unit succeed.
    pub fn write_to_store(&self, store: &dyn WorkspaceStore) -> anyhow::Result<()> {
        match self {
            Self::Request(item) => {
                let mut req_file = store
                    .read_request(&item.id)
                    .unwrap_or_else(|_| RequestFile {
                        name: item.name.clone(),
                        method: item.method,
                        url: item.url.clone(),
                        ..Default::default()
                    });
                req_file.update_from_item(item);
                store.write_request(&item.id, &req_file)?;
            }
            Self::Environment { name, env } => {
                let (file, secrets) = EnvironmentFile::from_environment(env);
                store.save_environment(name, &file)?;
                store.save_secrets(name, &secrets)?;
            }
            Self::Globals(env) => {
                let (file, secrets) = EnvironmentFile::from_environment(env);
                store.save_global_environment(&file)?;
                store.save_global_secrets(&secrets)?;
            }
            Self::WorkspaceMeta(meta) => {
                store.save_workspace_meta(meta)?;
            }
            Self::FolderMeta { folder_rel, meta } => {
                store.save_folder_meta(folder_rel, meta)?;
            }
            Self::Pins(pins) => {
                store.save_pins(pins)?;
            }
            Self::LocalState(state) => {
                store.save_state(state)?;
            }
        }
        Ok(())
    }
}

/// Persistence status of a given target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetStatus {
    Clean,
    Pending,
    InFlight,
    Failed,
}

#[derive(Debug, Clone)]
pub struct PendingWrite {
    pub revision: u64,
    pub workspace_gen: u64,
    pub payload: PersistPayload,
    pub debounce_duration: Duration,
}

#[derive(Debug, Default)]
pub struct TargetEntry {
    pub current_revision: u64,
    pub in_flight_revision: Option<u64>,
    pub last_saved_revision: u64,
    pub pending: Option<PendingWrite>,
    pub last_error: Option<String>,
}

impl TargetEntry {
    pub fn status(&self) -> TargetStatus {
        if self.last_error.is_some() && self.in_flight_revision.is_none() {
            TargetStatus::Failed
        } else if self.in_flight_revision.is_some() {
            TargetStatus::InFlight
        } else if self.pending.is_some() || self.current_revision > self.last_saved_revision {
            TargetStatus::Pending
        } else {
            TargetStatus::Clean
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.status() != TargetStatus::Clean
    }
}

/// Coordinator that manages ordered, coalesced, write-behind persistence.
pub struct PersistenceCoordinator {
    workspace_gen: u64,
    entries: HashMap<PersistTarget, TargetEntry>,
}

impl Default for PersistenceCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl PersistenceCoordinator {
    pub fn new() -> Self {
        Self {
            workspace_gen: 1,
            entries: HashMap::new(),
        }
    }

    pub fn workspace_generation(&self) -> u64 {
        self.workspace_gen
    }

    pub fn bump_workspace_generation(&mut self) -> u64 {
        self.workspace_gen = self.workspace_gen.wrapping_add(1);
        self.entries.clear();
        self.workspace_gen
    }

    /// Enqueues a new payload snapshot for target. Coalesces with any pending write,
    /// bounding memory to exactly 1 pending write per target.
    pub fn enqueue(
        &mut self,
        target: PersistTarget,
        payload: PersistPayload,
        debounce: Duration,
    ) -> u64 {
        let entry = self.entries.entry(target).or_default();
        entry.current_revision = entry.current_revision.wrapping_add(1);
        let rev = entry.current_revision;

        entry.pending = Some(PendingWrite {
            revision: rev,
            workspace_gen: self.workspace_gen,
            payload,
            debounce_duration: debounce,
        });

        // Reset transient error when a new edit arrives
        entry.last_error = None;
        rev
    }

    /// Checks if a write is currently in-flight for this target.
    pub fn is_in_flight(&self, target: &PersistTarget) -> bool {
        self.entries
            .get(target)
            .is_some_and(|e| e.in_flight_revision.is_some())
    }

    /// Transitions a pending write to in-flight.
    /// Returns the snapshot to write, enforcing the single-writer invariant per target.
    pub fn begin_in_flight(&mut self, target: &PersistTarget) -> Option<PendingWrite> {
        let entry = self.entries.get_mut(target)?;
        if entry.in_flight_revision.is_some() {
            return None;
        }

        let pending = entry.pending.take()?;
        entry.in_flight_revision = Some(pending.revision);
        Some(pending)
    }

    /// Records successful write completion for revision.
    /// Returns true if the target is now clean.
    pub fn mark_success(&mut self, target: &PersistTarget, revision: u64) -> bool {
        let Some(entry) = self.entries.get_mut(target) else {
            return true;
        };

        if entry.in_flight_revision == Some(revision) {
            entry.in_flight_revision = None;
        }
        if revision > entry.last_saved_revision {
            entry.last_saved_revision = revision;
        }
        entry.last_error = None;

        !entry.is_dirty()
    }

    /// Records write failure. Retains dirty status and restores snapshot to pending
    /// if no newer edit has arrived in the meantime.
    pub fn mark_failure(
        &mut self,
        target: &PersistTarget,
        revision: u64,
        payload: PersistPayload,
        error: String,
        debounce: Duration,
    ) {
        let Some(entry) = self.entries.get_mut(target) else {
            return;
        };

        if entry.in_flight_revision == Some(revision) {
            entry.in_flight_revision = None;
        }
        entry.last_error = Some(error);

        // If no newer edit was queued during the write, restore the failed payload for retry
        if entry.pending.is_none() && entry.current_revision == revision {
            entry.pending = Some(PendingWrite {
                revision,
                workspace_gen: self.workspace_gen,
                payload,
                debounce_duration: debounce,
            });
        }
    }

    /// Clears any pending write and removes tracking for a deleted target.
    pub fn cancel_target(&mut self, target: &PersistTarget) {
        self.entries.remove(target);
    }

    /// Remaps tracking from old target to new target (e.g. after rename or move).
    pub fn remap_target(&mut self, old: &PersistTarget, new: PersistTarget) {
        if let Some(entry) = self.entries.remove(old) {
            self.entries.insert(new, entry);
        }
    }

    pub fn status(&self, target: &PersistTarget) -> TargetStatus {
        self.entries
            .get(target)
            .map(|e| e.status())
            .unwrap_or(TargetStatus::Clean)
    }

    pub fn is_dirty(&self, target: &PersistTarget) -> bool {
        self.status(target) != TargetStatus::Clean
    }

    pub fn has_any_dirty(&self) -> bool {
        self.entries.values().any(|e| e.is_dirty())
    }

    pub fn dirty_targets(&self) -> Vec<PersistTarget> {
        self.entries
            .iter()
            .filter(|(_, e)| e.is_dirty())
            .map(|(t, _)| t.clone())
            .collect()
    }

    pub fn last_error(&self, target: &PersistTarget) -> Option<&str> {
        self.entries
            .get(target)
            .and_then(|e| e.last_error.as_deref())
    }

    /// Returns targets that currently have a pending write ready to be flushed.
    pub fn pending_targets(&self) -> Vec<PersistTarget> {
        self.entries
            .iter()
            .filter(|(_, e)| e.pending.is_some() && e.in_flight_revision.is_none())
            .map(|(t, _)| t.clone())
            .collect()
    }

    /// Drains all pending writes across targets (barrier before git checkout, shutdown, etc.)
    pub fn take_all_pending(&mut self) -> Vec<(PersistTarget, PendingWrite)> {
        let mut list = Vec::new();
        for (target, entry) in &mut self.entries {
            if entry.in_flight_revision.is_none() {
                if let Some(p) = entry.pending.take() {
                    entry.in_flight_revision = Some(p.revision);
                    list.push((target.clone(), p));
                }
            }
        }
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zest_core::models::HttpMethod;

    fn dummy_request_item(id: &str, url: &str) -> HttpRequestItem {
        let mut item = HttpRequestItem::new("ws", "Req", HttpMethod::Get, url);
        item.id = id.to_string();
        item
    }

    #[test]
    fn test_coordinator_coalesces_pending_writes() {
        let mut coord = PersistenceCoordinator::new();
        let target = PersistTarget::Request("users.yaml".to_string());

        let rev1 = coord.enqueue(
            target.clone(),
            PersistPayload::Request(Box::new(dummy_request_item("users.yaml", "http://a"))),
            Duration::from_millis(500),
        );
        let rev2 = coord.enqueue(
            target.clone(),
            PersistPayload::Request(Box::new(dummy_request_item("users.yaml", "http://b"))),
            Duration::from_millis(500),
        );

        assert_eq!(rev1, 1);
        assert_eq!(rev2, 2);
        assert_eq!(coord.status(&target), TargetStatus::Pending);

        // Beginning in-flight takes the latest coalesced write (revision 2)
        let write = coord.begin_in_flight(&target).expect("write ready");
        assert_eq!(write.revision, 2);
        if let PersistPayload::Request(r) = write.payload {
            assert_eq!(r.url, "http://b");
        } else {
            panic!("Expected request payload");
        }

        assert_eq!(coord.status(&target), TargetStatus::InFlight);
        assert!(coord.is_in_flight(&target));

        // Mark success cleans target
        let is_clean = coord.mark_success(&target, 2);
        assert!(is_clean);
        assert_eq!(coord.status(&target), TargetStatus::Clean);
    }

    #[test]
    fn test_coordinator_interleaved_write_race() {
        let mut coord = PersistenceCoordinator::new();
        let target = PersistTarget::Request("race.yaml".to_string());

        // First write begins
        coord.enqueue(
            target.clone(),
            PersistPayload::Request(Box::new(dummy_request_item("race.yaml", "http://v1"))),
            Duration::from_millis(500),
        );
        let write1 = coord.begin_in_flight(&target).unwrap();
        assert_eq!(write1.revision, 1);

        // Edit arrives while write1 is in-flight
        coord.enqueue(
            target.clone(),
            PersistPayload::Request(Box::new(dummy_request_item("race.yaml", "http://v2"))),
            Duration::from_millis(500),
        );

        // In-flight guard prevents starting a second concurrent write
        assert!(coord.is_in_flight(&target));
        assert!(coord.begin_in_flight(&target).is_none());

        // write1 finishes
        let is_clean = coord.mark_success(&target, 1);
        assert!(!is_clean, "Target must remain dirty because v2 is pending");
        assert_eq!(coord.status(&target), TargetStatus::Pending);

        // Now write2 can begin
        let write2 = coord.begin_in_flight(&target).unwrap();
        assert_eq!(write2.revision, 2);
        let is_clean2 = coord.mark_success(&target, 2);
        assert!(is_clean2);
        assert_eq!(coord.status(&target), TargetStatus::Clean);
    }

    #[test]
    fn test_coordinator_failure_retains_dirty_and_retries() {
        let mut coord = PersistenceCoordinator::new();
        let target = PersistTarget::Environment("prod".to_string());
        let env = Environment::new("ws", "prod");

        coord.enqueue(
            target.clone(),
            PersistPayload::Environment {
                name: "prod".to_string(),
                env: Box::new(env.clone()),
            },
            Duration::from_millis(500),
        );

        let write = coord.begin_in_flight(&target).unwrap();
        coord.mark_failure(
            &target,
            write.revision,
            write.payload,
            "Disk full".to_string(),
            Duration::from_millis(500),
        );

        assert_eq!(coord.status(&target), TargetStatus::Failed);
        assert_eq!(coord.last_error(&target), Some("Disk full"));
        assert!(coord.is_dirty(&target));

        // Retry succeeds
        let retry_write = coord.begin_in_flight(&target).expect("restored for retry");
        assert_eq!(retry_write.revision, 1);
        coord.mark_success(&target, 1);

        assert_eq!(coord.status(&target), TargetStatus::Clean);
        assert!(coord.last_error(&target).is_none());
    }

    #[test]
    fn test_coordinator_remap_and_cancel() {
        let mut coord = PersistenceCoordinator::new();
        let old_t = PersistTarget::Request("old.yaml".to_string());
        let new_t = PersistTarget::Request("new.yaml".to_string());

        coord.enqueue(
            old_t.clone(),
            PersistPayload::Request(Box::new(dummy_request_item("old.yaml", "http://x"))),
            Duration::from_millis(500),
        );

        coord.remap_target(&old_t, new_t.clone());
        assert_eq!(coord.status(&old_t), TargetStatus::Clean);
        assert_eq!(coord.status(&new_t), TargetStatus::Pending);

        coord.cancel_target(&new_t);
        assert_eq!(coord.status(&new_t), TargetStatus::Clean);
    }

    #[test]
    fn test_coordinator_workspace_generation_switch() {
        let mut coord = PersistenceCoordinator::new();
        let target = PersistTarget::Request("req.yaml".to_string());
        coord.enqueue(
            target.clone(),
            PersistPayload::Request(Box::new(dummy_request_item("req.yaml", "http://x"))),
            Duration::from_millis(500),
        );

        let gen1 = coord.workspace_generation();
        let gen2 = coord.bump_workspace_generation();
        assert_eq!(gen2, gen1 + 1);
        assert_eq!(coord.status(&target), TargetStatus::Clean);
    }
}
