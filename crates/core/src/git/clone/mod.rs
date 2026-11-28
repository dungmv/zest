use crate::storage::{FsWorkspaceStore, WorkspaceStore};
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
};
use thiserror::Error;

mod process;
mod source;

use process::run_clone;
use source::{clone_destination, validate_source};

const CANCELLED: u8 = 1;
const PUBLISHING: u8 = 2;

/// Coordinates cancellation with the last step that changes the destination.
#[derive(Clone, Debug, Default)]
pub struct CloneCancellation(Arc<AtomicU8>);

impl CloneCancellation {
    pub fn cancel(&self) {
        let _ = self
            .0
            .compare_exchange(0, CANCELLED, Ordering::AcqRel, Ordering::Acquire);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire) == CANCELLED
    }

    fn check(&self) -> Result<(), CloneWorkspaceError> {
        if self.is_cancelled() {
            return Err(CloneWorkspaceError::Cancelled);
        }
        Ok(())
    }

    fn begin_publish(&self) -> Result<(), CloneWorkspaceError> {
        self.0
            .compare_exchange(0, PUBLISHING, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| ())
            .map_err(|_| CloneWorkspaceError::Cancelled)
    }
}

#[derive(Debug, Error)]
pub enum CloneWorkspaceError {
    #[error("Enter an HTTPS or SSH Git clone URL without embedded credentials.")]
    InvalidUrl,
    #[error("Choose an existing parent folder using an absolute path.")]
    InvalidDestination,
    #[error("The repository name cannot be used as a local folder name on this system.")]
    InvalidRepositoryName,
    #[error("The repository folder already contains data or points to another location. Choose another parent folder or use Open Workspace to open an existing clone.")]
    DestinationOccupied,
    #[error("Could not write to the destination. Check its permissions and available disk space.")]
    DestinationWrite,
    #[error("Git is required. Install Git, then make sure it is available on PATH.")]
    GitMissing,
    #[error("Git could not authenticate. Check your credential helper or SSH agent and repository access.")]
    Authentication,
    #[error("Git could not reach the repository. Check the clone URL and network connection.")]
    RemoteUnavailable,
    #[error("Git could not clone the repository. Check the URL, repository access, and available disk space.")]
    CloneFailed,
    #[error("The Git clone timed out. Check your connection and try again.")]
    Timeout,
    #[error("The repository does not have workspace.yaml at its root.")]
    MissingMetadata,
    #[error("The repository's workspace.yaml is not a valid Zest workspace file.")]
    InvalidMetadata,
    #[error("The repository contains local-only data or a linked path inside the workspace. Remove it from the repository and try again.")]
    UnsafeLayout,
    #[error("Clone cancelled.")]
    Cancelled,
}

/// Clone a remote Zest workspace into a repository-named child of the parent folder.
pub fn clone_workspace(
    source: &str,
    parent_folder: &Path,
    cancel: &CloneCancellation,
) -> Result<PathBuf, CloneWorkspaceError> {
    let source = source.trim();
    validate_source(source)?;
    let destination = clone_destination(source, parent_folder)?;
    clone_from_source(source, &destination, cancel, "https:ssh")
}

fn clone_from_source(
    source: &str,
    destination: &Path,
    cancel: &CloneCancellation,
    protocols: &str,
) -> Result<PathBuf, CloneWorkspaceError> {
    cancel.check()?;
    let (Some(parent), Some(name)) = (destination.parent(), destination.file_name()) else {
        return Err(CloneWorkspaceError::InvalidDestination);
    };
    if !destination.is_absolute() {
        return Err(CloneWorkspaceError::InvalidDestination);
    }
    let parent = parent
        .canonicalize()
        .map_err(|_| CloneWorkspaceError::InvalidDestination)?;
    let destination = parent.join(name);
    destination_is_empty(&destination)?;

    let staging = tempfile::Builder::new()
        .prefix(".zest-clone-")
        .tempdir_in(&parent)
        .map_err(|_| CloneWorkspaceError::DestinationWrite)?;
    run_clone(source, staging.path(), protocols, cancel)?;
    cancel.check()?;
    let checkout = staging.path().join("checkout");
    validate_workspace(&checkout, cancel)?;
    let existed = destination_is_empty(&destination)?;
    cancel.begin_publish()?;
    if existed {
        // remove_dir only succeeds while the folder is still empty.
        fs::remove_dir(&destination).map_err(|_| {
            destination_is_empty(&destination)
                .err()
                .unwrap_or(CloneWorkspaceError::DestinationWrite)
        })?;
    }
    if let Err(err) = publish_checkout(&checkout, &destination) {
        let occupied = err.kind() == io::ErrorKind::AlreadyExists
            || fs::symlink_metadata(&destination).is_ok();
        if existed {
            let _ = fs::create_dir(&destination);
        }
        return Err(if occupied {
            CloneWorkspaceError::DestinationOccupied
        } else {
            CloneWorkspaceError::DestinationWrite
        });
    }
    Ok(destination)
}

fn destination_is_empty(path: &Path) -> Result<bool, CloneWorkspaceError> {
    match fs::symlink_metadata(path) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Ok(metadata) if metadata.is_dir() && !is_link(&metadata) => {
            let mut entries =
                fs::read_dir(path).map_err(|_| CloneWorkspaceError::DestinationWrite)?;
            if entries.next().is_some() {
                return Err(CloneWorkspaceError::DestinationOccupied);
            }
            Ok(true)
        }
        Ok(_) => Err(CloneWorkspaceError::DestinationOccupied),
        Err(_) => Err(CloneWorkspaceError::DestinationWrite),
    }
}

#[cfg(any(target_os = "linux", target_vendor = "apple", target_os = "redox"))]
fn publish_checkout(checkout: &Path, destination: &Path) -> io::Result<()> {
    use rustix::fs::{renameat_with, RenameFlags, CWD};
    renameat_with(CWD, checkout, CWD, destination, RenameFlags::NOREPLACE).map_err(Into::into)
}

#[cfg(not(any(target_os = "linux", target_vendor = "apple", target_os = "redox")))]
fn publish_checkout(checkout: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(checkout, destination)
}

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Includes junctions and other reparse points.
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn validate_workspace(root: &Path, cancel: &CloneCancellation) -> Result<(), CloneWorkspaceError> {
    match fs::symlink_metadata(root.join("workspace.yaml")) {
        Ok(meta) if is_link(&meta) => return Err(CloneWorkspaceError::UnsafeLayout),
        Ok(meta) if meta.is_file() => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(CloneWorkspaceError::MissingMetadata);
        }
        _ => return Err(CloneWorkspaceError::InvalidMetadata),
    }
    FsWorkspaceStore::new(root)
        .load_workspace_meta()
        .map_err(|_| CloneWorkspaceError::InvalidMetadata)?;
    match fs::symlink_metadata(root.join(".local")) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        _ => return Err(CloneWorkspaceError::UnsafeLayout),
    }
    let mut pending = [".gitignore", "collections", "environments"]
        .map(|name| root.join(name))
        .to_vec();
    while let Some(path) = pending.pop() {
        cancel.check()?;
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => return Err(CloneWorkspaceError::UnsafeLayout),
        };
        if is_link(&metadata) {
            return Err(CloneWorkspaceError::UnsafeLayout);
        }
        if metadata.is_dir() {
            for entry in fs::read_dir(path).map_err(|_| CloneWorkspaceError::UnsafeLayout)? {
                cancel.check()?;
                pending.push(entry.map_err(|_| CloneWorkspaceError::UnsafeLayout)?.path());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
