//! Modal dialogs: shared dialog shell plus each concrete modal component.
//!
//! Rendering and open/close state live in [`crate::state::ModalState`]
//! (registry API) and `crate::app::modals` (overlay router).

pub mod about_modal;
pub mod command_palette;
pub mod env_modal;
pub mod export_modal;
pub mod import_modal;
pub mod modal;
pub mod settings_modal;
pub mod workspace_setting_modal;

pub use about_modal::*;
pub use command_palette::*;
pub use env_modal::*;
pub use export_modal::*;
pub use import_modal::*;
pub use settings_modal::*;
pub use workspace_setting_modal::*;
