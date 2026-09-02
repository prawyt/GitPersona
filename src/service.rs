use crate::{
    api::{
        DependencyState, DependencyStatus, DoctorReport, NamedProfile, ProfileDraft,
        RepositoryScanEvent, RepositoryStatus, SshTestReport, SshTestStatus,
    },
    check::{self, CheckOptions},
    config::{Config, ConfigStore, Profile, SigningFormat, validate_profile_name},
    directory,
    error::GitPersonaError,
    git::Git,
    github::{Account, GitHub},
    process::{Runner, os_args},
    repository,
    ssh::{self, SshIdentity},
};
use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Duration,
};

pub struct GitPersonaService<'a> {
    store: ConfigStore,
    runner: &'a dyn Runner,
}

impl<'a> GitPersonaService<'a> {
    pub fn discover(runner: &'a dyn Runner) -> Result<Self, GitPersonaError> {
        Ok(Self::new(ConfigStore::discover()?, runner))
    }

    pub fn new(store: ConfigStore, runner: &'a dyn Runner) -> Self {
        Self { store, runner }
    }

    pub fn config(&self) -> Result<Config, GitPersonaError> {
        self.store.load()
    }

    pub fn config_path(&self) -> &Path {
        self.store.path()
    }

    pub fn list_profiles(&self) -> Result<Vec<NamedProfile>, GitPersonaError> {
        Ok(self
            .store
            .load()?
            .profiles
            .into_iter()
            .map(|(name, profile)| NamedProfile { name, profile })
            .collect())
    }

    pub fn get_profile(&self, name: &str) -> Result<NamedProfile, GitPersonaError> {
        let profile = self
            .store
            .load()?
            .profiles
            .get(name)
            .cloned()
            .ok_or_else(|| GitPersonaError::usage(format!("profile '{name}' does not exist")))?;
        Ok(NamedProfile {
            name: name.into(),
            profile,
        })
    }

    pub fn create_profile(
        &self,
        name: &str,
        profile: Profile,
    ) -> Result<NamedProfile, GitPersonaError> {
        validate_profile_name(name)?;
        profile.validate()?;
        profile.validate_local_resources()?;
        self.store.update(|config| {
            if config.profiles.contains_key(name) {
                return Err(GitPersonaError::usage(format!(
                    "profile '{name}' already exists"
                )));
            }
            config.profiles.insert(name.into(), profile.clone());
            Ok(())
        })?;
        Ok(NamedProfile {
            name: name.into(),
            profile,
        })
    }

    pub fn update_profile(
        &self,
        name: &str,
        profile: Profile,
    ) -> Result<NamedProfile, GitPersonaError> {
        validate_profile_name(name)?;
        profile.validate()?;
        profile.validate_local_resources()?;
        self.store.update(|config| {
            if !config.profiles.contains_key(name) {
                return Err(GitPersonaError::usage(format!(
                    "profile '{name}' does not exist"
                )));
            }
            config.profiles.insert(name.into(), profile.clone());
            Ok(())
        })?;
        directory::sync_profile(name, &self.store)?;
        Ok(NamedProfile {
            name: name.into(),
            profile,
        })
    }

    pub fn remove_profile(&self, name: &str) -> Result<(), GitPersonaError> {
        let config = self.store.load()?;
        if !config.profiles.contains_key(name) {
            return Err(GitPersonaError::usage(format!(
                "profile '{name}' does not exist"
            )));
        }
        if config.directories.iter().any(|rule| rule.profile == name) {
            return Err(GitPersonaError::usage(format!(
                "profile '{name}' has directory rules; remove them first"
            )));
        }
        self.store.update(|config| {
            config.profiles.remove(name);
            Ok(())
        })
    }

    pub fn import_preview(
        &self,
        repository: &Path,
        remote_name: &str,
    ) -> Result<ProfileDraft, GitPersonaError> {
        let git = Git::at(self.runner, repository);
        let root = git.ensure_repo()?;
        let remote = git.remote(remote_name)?.ok_or_else(|| {
            GitPersonaError::usage(format!("remote '{remote_name}' is not configured"))
        })?;
        let mut warnings = Vec::new();
        let github_user = match GitHub::new(self.runner).active_account(&remote.hostname) {
            Ok(user) => user,
            Err(error) => {
                warnings.push(format!(
                    "GitHub CLI identity could not be imported: {error}"
                ));
                None
            }
        };
        let signing_format = match git.get("gpg.format", false)?.as_deref() {
            Some("ssh") => SigningFormat::Ssh,
            _ => SigningFormat::Openpgp,
        };
        Ok(ProfileDraft {
            repository: root,
            github_user,
            git_name: git.get("user.name", false)?,
            git_email: git.get("user.email", false)?,
            hostname: remote.hostname,
            allowed_owners: vec![remote.owner],
            signing_key: git.get("user.signingKey", false)?,
            signing_format,
            require_signing: git
                .get("commit.gpgSign", false)?
                .is_some_and(|value| value.eq_ignore_ascii_case("true")),
            warnings,
        })
    }

    pub fn inspect_repository(
        &self,
        repository: &Path,
        remote_name: &str,
        network: bool,
    ) -> Result<RepositoryStatus, GitPersonaError> {
        let config = self.store.load()?;
        let report = check::inspect_at(
            self.runner,
            &config,
            repository,
            CheckOptions {
                remote_name,
                network,
            },
        )?;
        Ok(RepositoryStatus {
            report,
            network_checked: network,
        })
    }

    pub fn bind_repository(
        &self,
        repository: &Path,
        profile_name: &str,
        remote_name: &str,
        force: bool,
    ) -> Result<(), GitPersonaError> {
        let config = self.store.load()?;
        let profile = config.profiles.get(profile_name).ok_or_else(|| {
            GitPersonaError::usage(format!("profile '{profile_name}' does not exist"))
        })?;
        let git = Git::at(self.runner, repository);
        git.ensure_repo()?;
        let remote = git.remote(remote_name)?;
        let old_name = git.get("gitpersona.profile", true)?;
        let old_profile = old_name.as_ref().and_then(|name| config.profiles.get(name));
        git.bind(profile_name, profile, remote.as_ref(), force, old_profile)
    }

    pub fn unbind_repository(&self, repository: &Path) -> Result<(), GitPersonaError> {
        Git::at(self.runner, repository).unbind()
    }

    pub fn github_accounts(&self, hostname: &str) -> Result<Vec<Account>, GitPersonaError> {
        GitHub::new(self.runner).accounts(hostname)
    }

    pub fn switch_github_account(&self, profile_name: &str) -> Result<(), GitPersonaError> {
        let named = self.get_profile(profile_name)?;
        GitHub::new(self.runner).switch(&named.profile.hostname, &named.profile.github_user)
    }

    pub fn ssh_test(&self, profile_name: &str) -> Result<SshTestReport, GitPersonaError> {
        let named = self.get_profile(profile_name)?;
        let key = named.profile.ssh_key.clone();
        let (status, actual_user, message) = match ssh::verify(self.runner, &named.profile)? {
            SshIdentity::Verified(user) => (
                SshTestStatus::Verified,
                Some(user.clone()),
                format!("SSH authenticates as {user}"),
            ),
            SshIdentity::Rejected(user) => (
                SshTestStatus::Rejected,
                Some(user.clone()),
                format!(
                    "SSH authenticates as {user}, not {}",
                    named.profile.github_user
                ),
            ),
            SshIdentity::Unavailable(reason) => (
                SshTestStatus::Unavailable,
                None,
                if reason.is_empty() {
                    "SSH identity could not be verified".into()
                } else {
                    reason
                },
            ),
        };
        Ok(SshTestReport {
            profile: named.name,
            expected_user: named.profile.github_user,
            actual_user,
            hostname: named.profile.hostname,
            key,
            status,
            message,
        })
    }

    pub fn doctor(&self) -> Result<DoctorReport, GitPersonaError> {
        let config = self.store.load()?;
        let mut dependencies = Vec::new();
        for (program, args, remediation) in [
            (
                "git",
                vec!["--version"],
                "Install Git and ensure it is on PATH.",
            ),
            (
                "gh",
                vec!["--version"],
                "Install GitHub CLI and authenticate the accounts used by your profiles.",
            ),
            (
                "ssh",
                vec!["-V"],
                "Install OpenSSH and ensure it is on PATH.",
            ),
        ] {
            match self
                .runner
                .run(program, &os_args(&args), Duration::from_secs(10))
            {
                Ok(output) if output.code.is_some() => dependencies.push(DependencyStatus {
                    name: program.into(),
                    state: DependencyState::Ok,
                    detail: output
                        .combined()
                        .trim()
                        .lines()
                        .next()
                        .unwrap_or("available")
                        .into(),
                    remediation: None,
                }),
                Ok(_) => dependencies.push(DependencyStatus {
                    name: program.into(),
                    state: DependencyState::Unavailable,
                    detail: "timed out".into(),
                    remediation: Some(remediation.into()),
                }),
                Err(error) => dependencies.push(DependencyStatus {
                    name: program.into(),
                    state: DependencyState::Unavailable,
                    detail: error.to_string(),
                    remediation: Some(remediation.into()),
                }),
            }
        }
        let profile_issues = config
            .profiles
            .iter()
            .filter_map(|(name, profile)| {
                profile
                    .validate_local_resources()
                    .err()
                    .map(|error| format!("{name}: {error}"))
            })
            .collect::<Vec<_>>();
        let healthy = dependencies
            .iter()
            .all(|item| item.state == DependencyState::Ok)
            && profile_issues.is_empty();
        Ok(DoctorReport {
            config_path: self.store.path().to_path_buf(),
            schema_version: config.schema_version,
            profile_count: config.profiles.len(),
            dependencies,
            profile_issues,
            healthy,
        })
    }

    pub fn repository_roots(&self) -> Result<Vec<PathBuf>, GitPersonaError> {
        Ok(self.store.load()?.repository_roots)
    }

    pub fn add_repository_root(&self, root: &Path) -> Result<PathBuf, GitPersonaError> {
        let canonical = std::fs::canonicalize(root).map_err(|error| {
            GitPersonaError::usage(format!(
                "could not resolve repository root {}: {error}",
                root.display()
            ))
        })?;
        if !canonical.is_dir() {
            return Err(GitPersonaError::usage(
                "repository root must be an existing directory",
            ));
        }
        self.store.update(|config| {
            if !config
                .repository_roots
                .iter()
                .any(|existing| paths_equal(existing, &canonical))
            {
                config.repository_roots.push(canonical.clone());
            }
            Ok(())
        })?;
        Ok(canonical)
    }

    pub fn remove_repository_root(&self, root: &Path) -> Result<(), GitPersonaError> {
        self.store.update(|config| {
            let before = config.repository_roots.len();
            config
                .repository_roots
                .retain(|existing| !paths_equal(existing, root));
            if config.repository_roots.len() == before {
                return Err(GitPersonaError::usage(
                    "approved repository root does not exist",
                ));
            }
            Ok(())
        })
    }

    pub fn scan_repositories(
        &self,
        cancel: &AtomicBool,
        emit: impl FnMut(RepositoryScanEvent),
    ) -> Result<Vec<crate::api::RepositorySummary>, GitPersonaError> {
        let config = self.store.load()?;
        Ok(repository::scan_roots(self.runner, &config, cancel, emit))
    }
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    if cfg!(windows) {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    } else {
        left == right
    }
}
