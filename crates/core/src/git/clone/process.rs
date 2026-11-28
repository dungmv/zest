use std::{
    io::{Read, Seek},
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use super::{CloneCancellation, CloneWorkspaceError};
use crate::git::command::git_command;

const CLONE_TIMEOUT: Duration = Duration::from_secs(300);
const POLL_INTERVAL: Duration = Duration::from_millis(50);

pub(super) fn run_clone(
    source: &str,
    staging: &Path,
    protocols: &str,
    cancel: &CloneCancellation,
) -> Result<(), CloneWorkspaceError> {
    let mut stderr =
        tempfile::tempfile_in(staging).map_err(|_| CloneWorkspaceError::DestinationWrite)?;
    let mut git = git_command();
    git.args([
        "clone",
        "--origin",
        "origin",
        "--no-recurse-submodules",
        "--",
        source,
        "checkout",
    ])
    .current_dir(staging)
    .env("GIT_ALLOW_PROTOCOL", protocols)
    .env("GIT_TERMINAL_PROMPT", "0")
    .env("LC_ALL", "C")
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(
        stderr
            .try_clone()
            .map_err(|_| CloneWorkspaceError::DestinationWrite)?,
    );

    let status = wait_for_clone(&mut git, cancel, CLONE_TIMEOUT)?;
    if status.success() {
        return Ok(());
    }
    let mut output = Vec::new();
    if stderr.rewind().is_ok() {
        let _ = stderr.take(64 * 1024).read_to_end(&mut output);
    }
    Err(classify_git_failure(&String::from_utf8_lossy(&output)))
}

fn classify_git_failure(stderr: &str) -> CloneWorkspaceError {
    let stderr = stderr.to_ascii_lowercase();
    if [
        "authentication failed",
        "permission denied (",
        "could not read username",
        "could not read password",
        "access denied",
        "error: 401",
        "error: 403",
    ]
    .iter()
    .any(|pattern| stderr.contains(pattern))
    {
        CloneWorkspaceError::Authentication
    } else if [
        "resolve host",
        "resolve hostname",
        "could not connect",
        "failed to connect",
        "connection timed out",
        "connection refused",
        "repository not found",
    ]
    .iter()
    .any(|pattern| stderr.contains(pattern))
    {
        CloneWorkspaceError::RemoteUnavailable
    } else {
        CloneWorkspaceError::CloneFailed
    }
}

fn wait_for_clone(
    command: &mut Command,
    cancel: &CloneCancellation,
    timeout: Duration,
) -> Result<std::process::ExitStatus, CloneWorkspaceError> {
    if cancel.is_cancelled() {
        return Err(CloneWorkspaceError::Cancelled);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    let job = windows_job::Job::new().map_err(|_| CloneWorkspaceError::CloneFailed)?;
    let mut child = command.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            CloneWorkspaceError::GitMissing
        } else {
            CloneWorkspaceError::CloneFailed
        }
    })?;
    #[cfg(windows)]
    if job.assign(&child).is_err() {
        stop_and_reap(&mut child);
        return Err(CloneWorkspaceError::CloneFailed);
    }

    let started = Instant::now();
    let failure = loop {
        if cancel.is_cancelled() {
            break CloneWorkspaceError::Cancelled;
        }
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if started.elapsed() < timeout => thread::sleep(POLL_INTERVAL),
            Ok(None) => break CloneWorkspaceError::Timeout,
            Err(_) => break CloneWorkspaceError::CloneFailed,
        }
    };
    #[cfg(windows)]
    drop(job);
    stop_and_reap(&mut child);
    Err(failure)
}

fn stop_and_reap(child: &mut Child) {
    #[cfg(unix)]
    if let Some(pid) = rustix::process::Pid::from_raw(child.id() as i32) {
        let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(windows)]
mod windows_job {
    use std::io;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::process::Child;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    pub(super) struct Job(OwnedHandle);

    impl Job {
        pub(super) fn new() -> io::Result<Self> {
            // A null name and attributes create a private, non-inheritable job.
            let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            // A successful CreateJobObjectW returns a handle owned by the caller.
            let job = Self(unsafe { OwnedHandle::from_raw_handle(handle) });
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            // The information class matches the input struct and byte size.
            let result = unsafe {
                SetInformationJobObject(
                    job.0.as_raw_handle(),
                    JobObjectExtendedLimitInformation,
                    (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    std::mem::size_of_val(&limits) as u32,
                )
            };
            if result == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(job)
        }

        pub(super) fn assign(&self, child: &Child) -> io::Result<()> {
            // Both the job and the child remain owned during this call.
            if unsafe { AssignProcessToJobObject(self.0.as_raw_handle(), child.as_raw_handle()) }
                == 0
            {
                let error = io::Error::last_os_error();
                // A short-lived Git process can exit before job assignment.
                // Its caller can still read the exit status and diagnostics.
                if child_is_running(child) {
                    return Err(error);
                }
            }
            Ok(())
        }
    }

    fn child_is_running(child: &Child) -> bool {
        use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
        use windows_sys::Win32::System::Threading::WaitForSingleObject;
        // Zero timeout checks whether the child handle is already signaled.
        unsafe { WaitForSingleObject(child.as_raw_handle(), 0) != WAIT_OBJECT_0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interrupts_git_blocked_on_input() {
        let cancel = CloneCancellation::default();
        let mut git = git_command();
        git.args(["hash-object", "--stdin"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        assert!(matches!(
            wait_for_clone(&mut git, &cancel, Duration::from_millis(100)),
            Err(CloneWorkspaceError::Timeout)
        ));

        thread::scope(|scope| {
            scope.spawn(|| {
                thread::sleep(Duration::from_millis(150));
                cancel.cancel();
            });
            assert!(matches!(
                wait_for_clone(&mut git, &cancel, Duration::from_secs(5)),
                Err(CloneWorkspaceError::Cancelled)
            ));
        });
    }
}
