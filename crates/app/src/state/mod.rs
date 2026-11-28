//! Application state entities and the event contracts shared by their callers.

mod app_state;
mod collection;
mod environment;
mod events;
mod git;
mod modal;
mod persistence;
mod response;
mod session;
mod sidebar;
mod workspace;

pub use app_state::AppState;
pub use collection::{edit_active_request, CollectionState};
pub use environment::{EnvironmentSnapshot, EnvironmentState};
pub use events::{
    CollectionEvent, ComposerSyncScope, EnvironmentEvent, ModalEvent, ResponseEvent, SessionEvent,
    WorkspaceEvent,
};
pub use git::{GitActiveModal, GitState, UnifiedChangeItem};
pub use modal::{ModalKind, ModalPayload, ModalState, SettingsTab, WorkspaceSettingTarget};
pub use persistence::PersistenceState;
pub use response::{
    ImageZoomMode, ResponseState, ResponseTabKind, ResponseViewMode, MAX_PREVIEW_IMAGE_BYTES,
};
pub(crate) use session::SendId;
pub use session::{RequestSessionState, RequestTabKind};
pub use sidebar::SidebarState;
pub use workspace::WorkspaceState;

#[cfg(test)]
pub use events::FolderMoveOutcome;
