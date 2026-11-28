use anyhow::{anyhow, Result};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// Flags for hiding the console window when running child processes on Windows
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Construct a Git subprocess without opening a console window on Windows.
pub(super) fn git_command() -> Command {
    let command = Command::new("git");
    #[cfg(target_os = "windows")]
    let command = {
        let mut command = command;
        command.creation_flags(CREATE_NO_WINDOW);
        command
    };
    command
}

/// Helper to configure and execute a Git command safely
pub fn run_git_cmd(repo_dir: &Path, args: &[&str]) -> Result<String> {
    run_git_cmd_timeout(repo_dir, args, Duration::from_secs(30))
}

/// Helper to execute a Git command safely with a custom timeout
pub fn run_git_cmd_timeout(repo_dir: &Path, args: &[&str], timeout: Duration) -> Result<String> {
    run_git_cmd_internal(repo_dir, args, timeout, false)
}

/// Helper to execute a Git diff command safely with a custom timeout, treating exit code 0 and 1 as valid
pub fn run_git_diff_cmd_timeout(
    repo_dir: &Path,
    args: &[&str],
    timeout: Duration,
) -> Result<String> {
    run_git_cmd_internal(repo_dir, args, timeout, true)
}

/// Helper to execute a Git diff command safely
pub fn run_git_diff_cmd(repo_dir: &Path, args: &[&str]) -> Result<String> {
    run_git_diff_cmd_timeout(repo_dir, args, Duration::from_secs(30))
}

fn run_git_cmd_internal(
    repo_dir: &Path,
    args: &[&str],
    timeout: Duration,
    is_diff: bool,
) -> Result<String> {
    let mut cmd = git_command();
    cmd.args(args);
    cmd.current_dir(repo_dir);
    cmd.env("GIT_TERMINAL_PROMPT", "0");

    let (tx, rx) = std::sync::mpsc::channel();
    let args_str = args.join(" ");

    std::thread::Builder::new()
        .name("git-cmd-runner".to_string())
        .spawn(move || {
            let output = cmd.output();
            let _ = tx.send(output);
        })
        .map_err(|e| anyhow!("Failed to spawn git worker thread: {}", e))?;

    match rx.recv_timeout(timeout) {
        Ok(Ok(output)) => {
            // For diff commands, exit code 0 (no diff) and exit code 1 (diff found) are both valid outcomes.
            let is_success = if is_diff {
                output.status.success()
                    || (output.status.code() == Some(1) && !output.stdout.is_empty())
            } else {
                output.status.success()
            };

            if is_success {
                Ok(String::from_utf8_lossy(&output.stdout).to_string())
            } else {
                let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let out = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let msg = if !err.is_empty() {
                    err
                } else if !out.is_empty() {
                    out
                } else {
                    format!("git exited with status {}", output.status)
                };
                Err(anyhow!(msg))
            }
        }
        Ok(Err(e)) => Err(anyhow!("Failed to execute 'git {}': {}", args_str, e)),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(anyhow!(
            "Git command timed out after {:?}: git {}",
            timeout,
            args_str
        )),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(anyhow!(
            "Git command worker thread disconnected unexpectedly"
        )),
    }
}

/// Check if Git CLI is installed and accessible in PATH
pub fn is_git_available() -> bool {
    let mut cmd = git_command();
    cmd.arg("--version");

    cmd.output().map(|o| o.status.success()).unwrap_or(false)
}
