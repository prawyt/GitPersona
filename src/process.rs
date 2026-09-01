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
}

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
    let status = child.wait_timeout(timeout).map_err(|error| {
        GitPersonaError::dependency(format!("could not wait for {program}: {error}"))
    })?;
    if status.is_none() {
        let _ = child.kill();
        let _ = child.wait();
        return Ok(ProcessOutput {
            code: None,
            stdout: String::new(),
            stderr: format!("{program} timed out after {} seconds", timeout.as_secs()),
        });
    }
    let output = child.wait_with_output().map_err(|error| {
        GitPersonaError::dependency(format!("could not collect {program} output: {error}"))
    })?;
    Ok(ProcessOutput {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
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
