use crate::{
    config::{Profile, expand_path},
    error::GitPersonaError,
    process::{Runner, os_args},
    remote::{RemoteInfo, RemoteProtocol, parse_remote},
};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
    time::Duration,
};

const TIMEOUT: Duration = Duration::from_secs(15);
/// GitPersona's own bookkeeping keys, removed wholesale on unbind.
const BINDING_KEYS: &[&str] = &["gitpersona.profile", "gitpersona.version"];
/// Git's identity keys that a binding writes and a snapshot must capture.
const IDENTITY_KEYS: &[&str] = &[
    "user.name",
    "user.email",
    "core.sshCommand",
    "user.signingKey",
    "gpg.format",
    "commit.gpgSign",
];
const BACKUP_KEYS: &[(&str, &str, &str)] = &[
    (
        "user.name",
        "gitpersona.backupUserNamePresent",
        "gitpersona.backupUserName",
    ),
    (
        "user.email",
        "gitpersona.backupUserEmailPresent",
        "gitpersona.backupUserEmail",
    ),
    (
        "core.sshCommand",
        "gitpersona.backupSshCommandPresent",
        "gitpersona.backupSshCommand",
    ),
    (
        "user.signingKey",
        "gitpersona.backupSigningKeyPresent",
        "gitpersona.backupSigningKey",
    ),
    (
        "gpg.format",
        "gitpersona.backupSigningFormatPresent",
        "gitpersona.backupSigningFormat",
    ),
    (
        "commit.gpgSign",
        "gitpersona.backupCommitSigningPresent",
        "gitpersona.backupCommitSigning",
    ),
];

pub struct Git<'a> {
    runner: &'a dyn Runner,
    cwd: Option<PathBuf>,
}

impl<'a> Git<'a> {
    pub fn new(runner: &'a dyn Runner) -> Self {
        Self { runner, cwd: None }
    }

    pub fn at(runner: &'a dyn Runner, cwd: impl AsRef<Path>) -> Self {
        Self {
            runner,
            cwd: Some(cwd.as_ref().to_path_buf()),
        }
    }

    fn run(&self, args: &[OsString]) -> Result<crate::process::ProcessOutput, GitPersonaError> {
        match &self.cwd {
            Some(cwd) => self.runner.run_git_in(args, cwd, TIMEOUT),
            None => self.runner.run_git(args, TIMEOUT),
        }
    }

    pub fn ensure_repo(&self) -> Result<PathBuf, GitPersonaError> {
        let output = self.run(&os_args(&["rev-parse", "--show-toplevel"]))?;
        if !output.success() {
            return Err(GitPersonaError::usage(
                "current directory is not inside a Git worktree",
            ));
        }
        Ok(PathBuf::from(output.stdout.trim()))
    }

    pub fn common_dir(&self) -> Result<PathBuf, GitPersonaError> {
        let root = self.ensure_repo()?;
        let output = self.run(&os_args(&["rev-parse", "--git-common-dir"]))?;
        if !output.success() {
            return Err(GitPersonaError::dependency(
                "could not locate the Git common directory",
            ));
        }
        let path = PathBuf::from(output.stdout.trim());
        Ok(if path.is_absolute() {
            path
        } else {
            root.join(path)
        })
    }

    pub fn get(&self, key: &str, local: bool) -> Result<Option<String>, GitPersonaError> {
        let mut args = vec![OsString::from("config")];
        if local {
            args.push(OsString::from("--local"));
        }
        args.extend([OsString::from("--get"), OsString::from(key)]);
        let output = self.run(&args)?;
        match output.code {
            Some(0) => Ok(Some(output.stdout.trim_end().to_string())),
            Some(1) => Ok(None),
            _ => Err(GitPersonaError::dependency(format!(
                "git config --get {key} failed: {}",
                output.stderr.trim()
            ))),
        }
    }

    pub fn get_all(&self, key: &str, local: bool) -> Result<Vec<String>, GitPersonaError> {
        let mut args = vec![OsString::from("config")];
        if local {
            args.push(OsString::from("--local"));
        }
        args.extend([OsString::from("--get-all"), OsString::from(key)]);
        let output = self.run(&args)?;
        match output.code {
            Some(0) => Ok(output.stdout.lines().map(str::to_string).collect()),
            Some(1) => Ok(Vec::new()),
            _ => Err(GitPersonaError::dependency(format!(
                "git config --get-all {key} failed: {}",
                output.stderr.trim()
            ))),
        }
    }

    fn set(&self, key: &str, value: &str) -> Result<(), GitPersonaError> {
        let output = self.run(&[
            OsString::from("config"),
            OsString::from("--local"),
            OsString::from(key),
            OsString::from(value),
        ])?;
        if output.success() {
            Ok(())
        } else {
            Err(GitPersonaError::dependency(format!(
                "could not set {key}: {}",
                output.stderr.trim()
            )))
        }
    }

    fn unset(&self, key: &str) -> Result<(), GitPersonaError> {
        let output = self.run(&[
            OsString::from("config"),
            OsString::from("--local"),
            OsString::from("--unset-all"),
            OsString::from(key),
        ])?;
        if matches!(output.code, Some(0 | 5)) {
            Ok(())
        } else {
            Err(GitPersonaError::dependency(format!(
                "could not unset {key}: {}",
                output.stderr.trim()
            )))
        }
    }

    pub fn remote(&self, name: &str) -> Result<Option<RemoteInfo>, GitPersonaError> {
        let output = self.run(&[
            OsString::from("remote"),
            OsString::from("get-url"),
            OsString::from(name),
        ])?;
        if !output.success() {
            return Ok(None);
        }
        parse_remote(output.stdout.trim()).map(Some)
    }

    pub fn expected_ssh_command(profile: &Profile) -> Result<String, GitPersonaError> {
        let key = profile.ssh_key.as_ref().ok_or_else(|| {
            GitPersonaError::usage("SSH remotes require the profile to define --ssh-key")
        })?;
        let path = expand_path(key)?;
        if !path.is_file() {
            return Err(GitPersonaError::usage(format!(
                "SSH key does not exist: {}",
                path.display()
            )));
        }
        // Escape the four characters that keep their meaning inside a
        // double-quoted POSIX sh word. `!` is deliberately not escaped: it is
        // only special to interactive history expansion, and `\!` inside double
        // quotes survives literally, which would corrupt the key path.
        let escaped = path
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('$', "\\$")
            .replace('`', "\\`");
        Ok(format!("ssh -i \"{escaped}\" -o IdentitiesOnly=yes"))
    }

    /// Whether Git is configured to delegate credentials for `hostname` to the
    /// GitHub CLI. `scheme` must be the scheme of the remote actually in use:
    /// Git scopes `credential.<url>.helper` by scheme, so checking the `https`
    /// key for an `http` remote reports configuration that does not apply.
    pub fn has_compatible_credential_helper(
        &self,
        scheme: &str,
        hostname: &str,
    ) -> Result<bool, GitPersonaError> {
        let scoped = format!("credential.{scheme}://{hostname}.helper");
        let values = self
            .get_all(&scoped, false)?
            .into_iter()
            .chain(self.get_all("credential.helper", false)?);
        Ok(values.into_iter().any(|value| {
            value
                .to_ascii_lowercase()
                .contains("gh auth git-credential")
        }))
    }

    pub fn binding_drifted(
        &self,
        profile: &Profile,
        remote: Option<&RemoteInfo>,
    ) -> Result<bool, GitPersonaError> {
        if self.get("user.name", true)?.as_deref() != Some(profile.git_name.as_str()) {
            return Ok(true);
        }
        if self.get("user.email", true)?.as_deref() != Some(profile.git_email.as_str()) {
            return Ok(true);
        }
        if matches!(remote.map(|r| r.protocol), Some(RemoteProtocol::Ssh)) {
            return Ok(self.get("core.sshCommand", true)?.as_deref()
                != Some(Self::expected_ssh_command(profile)?.as_str()));
        }
        if let Some(key) = &profile.signing_key {
            if self.get("user.signingKey", true)?.as_deref() != Some(key.as_str())
                || self.get("gpg.format", true)?.as_deref()
                    != Some(profile.signing_format.as_git_value())
                || self.get("commit.gpgSign", true)?.as_deref()
                    != profile.require_signing.then_some("true")
            {
                return Ok(true);
            }
        } else if self.get("user.signingKey", true)?.is_some()
            || self.get("gpg.format", true)?.is_some()
            || self.get("commit.gpgSign", true)?.is_some()
        {
            return Ok(true);
        }
        Ok(false)
    }

    pub fn bind(
        &self,
        profile_name: &str,
        profile: &Profile,
        remote: Option<&RemoteInfo>,
        force: bool,
        old_profile: Option<&Profile>,
    ) -> Result<(), GitPersonaError> {
        self.ensure_repo()?;
        let existing = self.get("gitpersona.profile", true)?;
        if existing.is_some() && !force {
            if let Some(old) = old_profile {
                if self.binding_drifted(old, remote)? {
                    return Err(GitPersonaError::usage(
                        "managed Git settings have changed; inspect them and rerun with --force to rebind",
                    ));
                }
            } else {
                return Err(GitPersonaError::usage(
                    "the repository references a missing profile; rerun with --force to repair the binding",
                ));
            }
        }

        let all_keys = transaction_keys();
        let snapshot = self.snapshot(&all_keys)?;
        let result = (|| {
            for (managed, present_key, value_key) in BACKUP_KEYS {
                if self.get(present_key, true)?.is_none() {
                    let value = self.get(managed, true)?;
                    self.set(present_key, if value.is_some() { "true" } else { "false" })?;
                    if let Some(value) = value {
                        self.set(value_key, &value)?;
                    } else {
                        self.unset(value_key)?;
                    }
                }
            }
            self.set("gitpersona.profile", profile_name)?;
            self.set("gitpersona.version", "2")?;
            self.set("user.name", &profile.git_name)?;
            self.set("user.email", &profile.git_email)?;
            if matches!(remote.map(|r| r.protocol), Some(RemoteProtocol::Ssh)) {
                self.set("core.sshCommand", &Self::expected_ssh_command(profile)?)?;
            } else {
                self.unset("core.sshCommand")?;
            }
            if let Some(key) = &profile.signing_key {
                self.set("user.signingKey", key)?;
                self.set("gpg.format", profile.signing_format.as_git_value())?;
                if profile.require_signing {
                    self.set("commit.gpgSign", "true")?;
                } else {
                    self.unset("commit.gpgSign")?;
                }
            } else {
                self.unset("user.signingKey")?;
                self.unset("gpg.format")?;
                self.unset("commit.gpgSign")?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            let rollback = self.restore(&snapshot);
            return match rollback {
                Ok(()) => Err(error),
                Err(rollback) => Err(GitPersonaError::dependency(format!(
                    "{error}; rollback also failed: {rollback}"
                ))),
            };
        }
        Ok(())
    }

    pub fn unbind(&self) -> Result<(), GitPersonaError> {
        self.ensure_repo()?;
        if self.get("gitpersona.profile", true)?.is_none() {
            return Err(GitPersonaError::usage(
                "repository is not bound to a GitPersona profile",
            ));
        }
        let all_keys = transaction_keys();
        let snapshot = self.snapshot(&all_keys)?;
        let result = (|| {
            for (managed, present_key, value_key) in BACKUP_KEYS {
                let present = self.get(present_key, true)?.as_deref() == Some("true");
                if present {
                    match self.get(value_key, true)? {
                        Some(value) => self.set(managed, &value)?,
                        // Backup flag says the value existed, but the backup
                        // itself is missing (manual tampering). Fall back to
                        // unsetting rather than trapping the repository in a
                        // permanently bound state.
                        None => self.unset(managed)?,
                    }
                } else {
                    self.unset(managed)?;
                }
            }
            for key in BINDING_KEYS {
                self.unset(key)?;
            }
            for (_, present, value) in BACKUP_KEYS {
                self.unset(present)?;
                self.unset(value)?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            let _ = self.restore(&snapshot);
            return Err(error);
        }
        Ok(())
    }

    fn snapshot(
        &self,
        keys: &[String],
    ) -> Result<BTreeMap<String, Option<String>>, GitPersonaError> {
        keys.iter()
            .map(|key| Ok((key.clone(), self.get(key, true)?)))
            .collect()
    }

    fn restore(&self, snapshot: &BTreeMap<String, Option<String>>) -> Result<(), GitPersonaError> {
        for (key, value) in snapshot {
            if let Some(value) = value {
                self.set(key, value)?;
            } else {
                self.unset(key)?;
            }
        }
        Ok(())
    }
}

fn transaction_keys() -> Vec<String> {
    let mut keys = BINDING_KEYS
        .iter()
        .chain(IDENTITY_KEYS)
        .map(|key| (*key).to_string())
        .collect::<Vec<_>>();
    for (_, present, value) in BACKUP_KEYS {
        keys.push((*present).to_string());
        keys.push((*value).to_string());
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_command_quotes_paths() {
        let dir = tempfile::tempdir().unwrap();
        let key = dir.path().join("my key");
        std::fs::write(&key, "key").unwrap();
        let profile = Profile {
            github_user: "a".into(),
            git_name: "A".into(),
            git_email: "a@x".into(),
            hostname: "github.com".into(),
            ssh_key: Some(key.clone()),
            allowed_owners: vec![],
            signing_key: None,
            signing_format: crate::config::SigningFormat::Openpgp,
            require_signing: false,
        };
        let command = Git::expected_ssh_command(&profile).unwrap();
        assert!(command.contains("\""));
        assert!(command.contains("IdentitiesOnly=yes"));
    }

    #[test]
    fn ssh_command_leaves_exclamation_marks_intact() {
        // `\!` inside a double-quoted sh word survives literally, so escaping
        // it would hand ssh a key path that does not exist.
        let dir = tempfile::tempdir().unwrap();
        let key = dir.path().join("my!key");
        std::fs::write(&key, "key").unwrap();
        let profile = Profile {
            github_user: "a".into(),
            git_name: "A".into(),
            git_email: "a@x".into(),
            hostname: "github.com".into(),
            ssh_key: Some(key.clone()),
            allowed_owners: vec![],
            signing_key: None,
            signing_format: crate::config::SigningFormat::Openpgp,
            require_signing: false,
        };
        let command = Git::expected_ssh_command(&profile).unwrap();
        assert!(command.contains("my!key"), "{command}");
        assert!(!command.contains(r"\!"), "{command}");
    }
}
