//! Typed commands/events emitted by state entities (`WorkspaceState`,
//! `CollectionState`, `RequestSessionState`, `ResponseState`, `EnvironmentState`)
//! so downstream states and UI editors receive change payloads in a single
//! synchronous dispatch instead of polling intermediate state fields on `cx.notify()`.

use super::{
    CollectionState, EnvironmentSnapshot, EnvironmentState, ModalState, RequestSessionState,
    ResponseState, WorkspaceState,
};
use gpui_kit::EventEmitter;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use zest_core::models::StoredCookie;
use zest_core::storage::models::{LocalState, WorkspaceMeta};
use zest_core::storage::WorkspaceStore;

/// Events emitted by `WorkspaceState`.
#[derive(Clone)]
pub enum WorkspaceEvent {
    /// User initiated opening or creating a workspace; `CollectionState` should
    /// switch its store and run an async background scan.
    WorkspaceSwitchRequested {
        path: PathBuf,
        store: Arc<dyn WorkspaceStore>,
        folder_name: String,
    },
    /// Workspace-level metadata, headers, or auth were modified.
    WorkspaceMetaModified,
}

impl fmt::Debug for WorkspaceEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorkspaceSwitchRequested {
                path, folder_name, ..
            } => f
                .debug_struct("WorkspaceSwitchRequested")
                .field("path", path)
                .field("folder_name", folder_name)
                .finish(),
            Self::WorkspaceMetaModified => write!(f, "WorkspaceMetaModified"),
        }
    }
}

impl PartialEq for WorkspaceEvent {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::WorkspaceSwitchRequested {
                    path: p1,
                    folder_name: n1,
                    ..
                },
                Self::WorkspaceSwitchRequested {
                    path: p2,
                    folder_name: n2,
                    ..
                },
            ) => p1 == p2 && n1 == n2,
            (Self::WorkspaceMetaModified, Self::WorkspaceMetaModified) => true,
            _ => false,
        }
    }
}

impl Eq for WorkspaceEvent {}

impl EventEmitter<WorkspaceEvent> for WorkspaceState {}

/// Events emitted by `CollectionState`.
#[derive(Clone)]
pub enum CollectionEvent {
    /// A background workspace scan completed; carries workspace metadata and
    /// cookies so `WorkspaceState` hydrates from the same single disk pass.
    WorkspaceScanLoaded {
        workspace_meta: Option<WorkspaceMeta>,
        local_state: Box<LocalState>,
        environment: Box<EnvironmentSnapshot>,
        cookies: Vec<StoredCookie>,
        folder_name: String,
        store: Arc<dyn WorkspaceStore>,
        announce_opened: bool,
        workspace_path: PathBuf,
    },
    WorkspaceMetaReloaded {
        meta: Box<WorkspaceMeta>,
        store: Arc<dyn WorkspaceStore>,
    },
    /// Request/folder ids changed on disk (request rename, folder rename/move,
    /// cross-folder request move). `(old_id, new_id)` pairs.
    IdsRemapped(Vec<(String, String)>),
    /// Request items were reloaded from disk (e.g. external fs watcher, conflict reload, import).
    RescanRequested,
    RequestsReloaded {
        /// `None` means the entire tree was reloaded; `Some(ids)` lists specific request IDs.
        req_ids: Option<Vec<String>>,
    },
    ImportApplied(Vec<String>),
    RequestDeleted(String),
    RequestContentModified(String),
    /// A saved example of `req_id` was renamed.
    ExampleRenamed {
        req_id: String,
        old_name: String,
        new_name: String,
    },
    /// A user-initiated CRUD operation was rejected; surfaced as an error toast.
    CrudFailed {
        title: &'static str,
        message: String,
    },
    /// Folder-level metadata, headers, auth, or order were modified.
    FolderMetaModified(String),
    /// Request pin list was modified.
    PinsModified,
}

impl fmt::Debug for CollectionEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorkspaceScanLoaded {
                folder_name,
                announce_opened,
                workspace_path,
                ..
            } => f
                .debug_struct("WorkspaceScanLoaded")
                .field("folder_name", folder_name)
                .field("announce_opened", announce_opened)
                .field("workspace_path", workspace_path)
                .finish(),
            Self::WorkspaceMetaReloaded { .. } => write!(f, "WorkspaceMetaReloaded"),
            Self::IdsRemapped(pairs) => f.debug_tuple("IdsRemapped").field(pairs).finish(),
            Self::RescanRequested => f.write_str("RescanRequested"),
            Self::RequestsReloaded { req_ids } => f
                .debug_struct("RequestsReloaded")
                .field("req_ids", req_ids)
                .finish(),
            Self::ImportApplied(names) => f.debug_tuple("ImportApplied").field(names).finish(),
            Self::RequestDeleted(id) => f.debug_tuple("RequestDeleted").field(id).finish(),
            Self::RequestContentModified(id) => {
                f.debug_tuple("RequestContentModified").field(id).finish()
            }
            Self::ExampleRenamed {
                req_id,
                old_name,
                new_name,
            } => f
                .debug_struct("ExampleRenamed")
                .field("req_id", req_id)
                .field("old_name", old_name)
                .field("new_name", new_name)
                .finish(),
            Self::CrudFailed { title, message } => f
                .debug_struct("CrudFailed")
                .field("title", title)
                .field("message", message)
                .finish(),
            Self::FolderMetaModified(folder_rel) => f
                .debug_tuple("FolderMetaModified")
                .field(folder_rel)
                .finish(),
            Self::PinsModified => write!(f, "PinsModified"),
        }
    }
}

impl PartialEq for CollectionEvent {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::WorkspaceScanLoaded {
                    folder_name: f1,
                    announce_opened: a1,
                    workspace_path: p1,
                    ..
                },
                Self::WorkspaceScanLoaded {
                    folder_name: f2,
                    announce_opened: a2,
                    workspace_path: p2,
                    ..
                },
            ) => f1 == f2 && a1 == a2 && p1 == p2,
            (
                Self::WorkspaceMetaReloaded { meta: a, store: sa },
                Self::WorkspaceMetaReloaded { meta: b, store: sb },
            ) => a == b && Arc::ptr_eq(sa, sb),
            (Self::IdsRemapped(a), Self::IdsRemapped(b)) => a == b,
            (Self::ImportApplied(a), Self::ImportApplied(b)) => a == b,
            (Self::RequestDeleted(a), Self::RequestDeleted(b))
            | (Self::RequestContentModified(a), Self::RequestContentModified(b)) => a == b,
            (Self::RequestsReloaded { req_ids: a }, Self::RequestsReloaded { req_ids: b }) => {
                a == b
            }
            (
                Self::ExampleRenamed {
                    req_id: r1,
                    old_name: o1,
                    new_name: n1,
                },
                Self::ExampleRenamed {
                    req_id: r2,
                    old_name: o2,
                    new_name: n2,
                },
            ) => r1 == r2 && o1 == o2 && n1 == n2,
            (
                Self::CrudFailed {
                    title: t1,
                    message: m1,
                },
                Self::CrudFailed {
                    title: t2,
                    message: m2,
                },
            ) => t1 == t2 && m1 == m2,
            (Self::FolderMetaModified(a), Self::FolderMetaModified(b)) => a == b,
            (Self::PinsModified, Self::PinsModified) => true,
            (Self::RescanRequested, Self::RescanRequested) => true,
            _ => false,
        }
    }
}

impl Eq for CollectionEvent {}

impl EventEmitter<CollectionEvent> for CollectionState {}

/// Outcome of a folder move, returned by `move_folder_internal`.
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderMoveOutcome {
    /// Nothing changed (unknown source, same-parent drop with no target).
    Unchanged,
    /// Same-parent drop: in-memory sibling reorder only.
    Reordered,
    /// Moved on disk; `(old_id, new_id)` pairs of the folder and descendants.
    Moved(Vec<(String, String)>),
}

/// Scope of composer text inputs to synchronize from the active request model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerSyncScope {
    /// Only reload body editors (JSON, XML, Raw).
    BodyOnly,
    /// Reload URL, Auth, Pre-request/Test scripts, and Body editors.
    All,
}

/// Events emitted by `RequestSessionState`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    /// User switched the active request pointer.
    ActiveRequestChanged { req_id: String },
    /// Active request was mutated and requires debounced auto-save.
    /// Optionally synchronizes composer input editors if the mutation happened outside the input.
    RequestModified {
        req_id: String,
        sync_inputs: Option<ComposerSyncScope>,
    },
    /// Explicitly synchronize composer inputs from the in-memory request model without marking dirty.
    SyncComposerInputs { scope: ComposerSyncScope },
}

impl EventEmitter<SessionEvent> for RequestSessionState {}

/// Events emitted by `ResponseState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseEvent {
    /// Displayed response content or view mode changed; inspector editors must update.
    ContentChanged {
        /// `true` when the response/example record itself changed; `false` when only view mode changed.
        sync_sent_preview: bool,
    },
}

impl EventEmitter<ResponseEvent> for ResponseState {}

/// Events emitted by `EnvironmentState`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvironmentEvent {
    /// Active environment selection changed.
    ActiveEnvironmentChanged(Option<String>),
    /// Environment variables or metadata were modified.
    EnvironmentModified(String),
    /// Globals variables were modified.
    GlobalsModified,
    /// Environments were reloaded or externally/programmatically updated; UI inputs must re-sync.
    EnvironmentsReloaded,
    RenameRequested {
        id: String,
        name: String,
    },
    DeleteRequested(String),
    EnvironmentRenamed {
        old_id: String,
        new_id: String,
    },
}

impl EventEmitter<EnvironmentEvent> for EnvironmentState {}

/// Events emitted by `ModalState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalEvent {
    /// Environment modal opened or its selected environment changed.
    EnvModalSelectionChanged,
    /// Active light/dark theme mode changed (either by user preference or OS appearance).
    ThemeChanged,
}

impl EventEmitter<ModalEvent> for ModalState {}

#[cfg(test)]
#[path = "events_tests.rs"]
mod tests;
