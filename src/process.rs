use crate::error::GitPersonaError;
use std::{
    ffi::OsString,
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};
use wait_timeout::ChildExt;

#[derive(Debug, Clone)]
pub struct ProcessOutput {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl ProcessOutput {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
    pub fn combined(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

/// Environment variables that would silently redirect Git away from the
/// repository GitPersona was told to act on, or forge the values it reads back.
///
/// `GIT_DIR`, `GIT_WORK_TREE`, and friends take precedence over the working
/// directory, so a bind would write to a repository the user never selected.
/// `GIT_CONFIG_COUNT`/`KEY`/`VALUE` inject configuration into every invocation,
/// which can make a check report an identity that is not the one on disk.
/// `GIT_SSH_COMMAND` overrides the `core.sshCommand` a profile manages.
///
/// `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` are deliberately NOT removed:
/// they are the documented way to point Git at an alternate global config, and
/// honouring them is what lets a caller sandbox GitPersona's `--global`
/// includeIf writes. Removing them would break that isolation rather than
/// protect anything — the user set them for their own shell.
const GIT_ENVIRONMENT_OVERRIDES: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_CEILING_DIRECTORIES",
    "GIT_CONFIG",
    "GIT_CONFIG_COUNT",
    "GIT_SSH_COMMAND",
];

pub trait Runner: Send + Sync {
    fn run(
        &self,
        program: &str,
        args: &[OsString],
        timeout: Duration,
    ) -> Result<ProcessOutput, GitPersonaError>;

    fn run_in(
        &self,
        program: &str,
        args: &[OsString],
        cwd: &Path,
        timeout: Duration,
    ) -> Result<ProcessOutput, GitPersonaError> {
        let _ = cwd;
        self.run(program, args, timeout)
    }

    /// Run `git` with the ambient environment scrubbed of the variables that
    /// would redirect it away from the intended repository or configuration
    /// file. Every `git` invocation must go through this or [`Runner::run_git_in`].
    fn run_git(
        &self,
        args: &[OsString],
        timeout: Duration,
    ) -> Result<ProcessOutput, GitPersonaError> {
        self.run("git", args, timeout)
    }

    /// Directory-scoped counterpart to [`Runner::run_git`].
    fn run_git_in(
        &self,
        args: &[OsString],
        cwd: &Path,
        timeout: Duration,
    ) -> Result<ProcessOutput, GitPersonaError> {
        self.run_in("git", args, cwd, timeout)
    }
}

pub struct SystemRunner;

impl Runner for SystemRunner {
    fn run(
        &self,
        program: &str,
        args: &[OsString],
        timeout: Duration,
    ) -> Result<ProcessOutput, GitPersonaError> {
        run_command(Command::new(program), program, args, timeout)
    }

    fn run_in(
        &self,
        program: &str,
        args: &[OsString],
        cwd: &Path,
        timeout: Duration,
    ) -> Result<ProcessOutput, GitPersonaError> {
        let mut command = Command::new(program);
        command.current_dir(cwd);
        run_command(command, program, args, timeout)
    }

    fn run_git(
        &self,
        args: &[OsString],
        timeout: Duration,
    ) -> Result<ProcessOutput, GitPersonaError> {
        run_command(git_command(), "git", args, timeout)
    }

    fn run_git_in(
        &self,
        args: &[OsString],
        cwd: &Path,
        timeout: Duration,
    ) -> Result<ProcessOutput, GitPersonaError> {
        let mut command = git_command();
        command.current_dir(cwd);
        run_command(command, "git", args, timeout)
    }
}

fn git_command() -> Command {
    let mut command = Command::new("git");
    for variable in GIT_ENVIRONMENT_OVERRIDES {
        command.env_remove(variable);
    }
    // GIT_CONFIG_COUNT is removed above; the indexed pairs it governs must go
    // too, or a stale GIT_CONFIG_KEY_0 could be picked up by a later count.
    for index in 0..MAX_GIT_CONFIG_PAIRS {
        command.env_remove(format!("GIT_CONFIG_KEY_{index}"));
        command.env_remove(format!("GIT_CONFIG_VALUE_{index}"));
    }
    command
}

const MAX_GIT_CONFIG_PAIRS: usize = 64;

fn run_command(
    mut command: Command,
    program: &str,
    args: &[OsString],
    timeout: Duration,
) -> Result<ProcessOutput, GitPersonaError> {
    let mut child = command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            GitPersonaError::dependency(format!("could not run {program}: {error}"))
        })?;

    // Drain stdout and stderr in background threads to prevent pipe deadlock.
    // If a subprocess writes more than the OS pipe buffer capacity (~64KB),
    // it blocks on write while the parent blocks on wait — a classic deadlock.
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

    let stdout_thread = std::thread::spawn(move || {
        stdout_pipe.map_or_else(String::new, |mut pipe| {
            let mut buf = String::new();
            let _ = std::io::Read::read_to_string(&mut pipe, &mut buf);
            buf
        })
    });
    let stderr_thread = std::thread::spawn(move || {
        stderr_pipe.map_or_else(String::new, |mut pipe| {
            let mut buf = String::new();
            let _ = std::io::Read::read_to_string(&mut pipe, &mut buf);
            buf
        })
    });

    let status = child.wait_timeout(timeout).map_err(|error| {
        GitPersonaError::dependency(format!("could not wait for {program}: {error}"))
    })?;

    if status.is_none() {
        kill_process_tree(&mut child);
        // Allow the reader threads to finish after process is killed.
        let _ = stdout_thread.join();
        let _ = stderr_thread.join();
        return Ok(ProcessOutput {
            code: None,
            stdout: String::new(),
            stderr: format!("{program} timed out after {} seconds", timeout.as_secs()),
        });
    }

    let stdout = stdout_thread.join().unwrap_or_default();
    let stderr = stderr_thread.join().unwrap_or_default();

    Ok(ProcessOutput {
        code: status.and_then(|s| s.code()),
        stdout,
        stderr,
    })
}

fn kill_process_tree(child: &mut std::process::Child) {
    #[cfg(windows)]
    {
        // On Windows, child.kill() only terminates the top-level process.
        // /F = forcefully terminate, /T = terminate process and all child processes.
        let pid = child.id();
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

pub fn os_args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_runner_enforces_timeout() {
        #[cfg(windows)]
        let (program, args) = (
            "powershell",
            os_args(&["-NoProfile", "-Command", "Start-Sleep -Seconds 5"]),
        );
        #[cfg(not(windows))]
        let (program, args) = ("sh", os_args(&["-c", "sleep 5"]));

        let output = SystemRunner
            .run(program, &args, Duration::from_millis(50))
            .unwrap();
        assert_eq!(output.code, None);
        assert!(output.stderr.contains("timed out"));
    }

    #[test]
    fn system_runner_captures_successful_output() {
        let output = SystemRunner
            .run("rustc", &os_args(&["--version"]), Duration::from_secs(5))
            .unwrap();
        assert_eq!(output.code, Some(0));
        assert!(output.stdout.starts_with("rustc "));
    }
}
