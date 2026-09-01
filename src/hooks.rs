use crate::{error::GitPersonaError, git::Git, process::Runner};
use std::{fs, path::PathBuf};

const MARKER: &str = "# Managed by GitPersona v0.1";
const PRE_COMMIT: &str =
    "#!/bin/sh\n# Managed by GitPersona v0.1\nexec gitpersona check --hook pre-commit\n";
const PRE_PUSH: &str = "#!/bin/sh\n# Managed by GitPersona v0.1\nexec gitpersona check --hook pre-push --remote \"${1:-origin}\"\n";

pub struct HookManager<'a> {
    git: Git<'a>,
}

impl<'a> HookManager<'a> {
    pub fn new(runner: &'a dyn Runner) -> Self {
        Self {
            git: Git::new(runner),
        }
    }

    fn paths(&self) -> Result<(PathBuf, PathBuf), GitPersonaError> {
        if self.git.get("core.hooksPath", false)?.is_some() {
            return Err(GitPersonaError::usage(
                "core.hooksPath is configured; GitPersona will not modify or chain that hook setup",
            ));
        }
        let hooks = self.git.common_dir()?.join("hooks");
        Ok((hooks.join("pre-commit"), hooks.join("pre-push")))
    }

    pub fn install(&self) -> Result<(), GitPersonaError> {
        let (commit, push) = self.paths()?;
        for path in [&commit, &push] {
            if path.exists() {
                return Err(GitPersonaError::usage(format!(
                    "hook already exists; refusing to replace {}",
                    path.display()
                )));
            }
        }
        let parent = commit.parent().expect("hook has parent");
        fs::create_dir_all(parent).map_err(|e| {
            GitPersonaError::dependency(format!("could not create hooks directory: {e}"))
        })?;
        write_hook(&commit, PRE_COMMIT)?;
        if let Err(error) = write_hook(&push, PRE_PUSH) {
            let _ = fs::remove_file(&commit);
            return Err(error);
        }
        Ok(())
    }

    pub fn status(&self) -> Result<String, GitPersonaError> {
        let (commit, push) = self.paths()?;
        Ok(format!(
            "pre-commit: {}\npre-push:   {}",
            hook_state(&commit, PRE_COMMIT),
            hook_state(&push, PRE_PUSH)
        ))
    }

    pub fn uninstall(&self) -> Result<(), GitPersonaError> {
        let (commit, push) = self.paths()?;
        for (path, expected) in [(&commit, PRE_COMMIT), (&push, PRE_PUSH)] {
            if path.exists() {
                let contents = fs::read_to_string(path).map_err(|e| {
                    GitPersonaError::dependency(format!("could not read {}: {e}", path.display()))
                })?;
                if contents != expected || !contents.contains(MARKER) {
                    return Err(GitPersonaError::usage(format!(
                        "{} is not an exact GitPersona-managed hook; refusing to remove it",
                        path.display()
                    )));
                }
            }
        }
        for path in [&commit, &push] {
            if path.exists() {
                fs::remove_file(path).map_err(|e| {
                    GitPersonaError::dependency(format!("could not remove {}: {e}", path.display()))
                })?;
            }
        }
        Ok(())
    }
}

fn write_hook(path: &std::path::Path, contents: &str) -> Result<(), GitPersonaError> {
    fs::write(path, contents).map_err(|e| {
        GitPersonaError::dependency(format!("could not write {}: {e}", path.display()))
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).map_err(|e| {
            GitPersonaError::dependency(format!(
                "could not make {} executable: {e}",
                path.display()
            ))
        })?;
    }
    Ok(())
}

fn hook_state(path: &std::path::Path, expected: &str) -> &'static str {
    match fs::read_to_string(path) {
        Ok(contents) if contents == expected => "installed",
        Ok(_) => "occupied by another hook",
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => "not installed",
        Err(_) => "unreadable",
    }
}
