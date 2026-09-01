use crate::error::GitPersonaError;
use clap::ValueEnum;
use directories::{BaseDirs, ProjectDirs};
use fs2::FileExt;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    pub schema_version: u32,
    #[serde(default)]
    pub profiles: BTreeMap<String, Profile>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: 1,
            profiles: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Profile {
    pub github_user: String,
    pub git_name: String,
    pub git_email: String,
    #[serde(default = "default_hostname")]
    pub hostname: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_key: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_owners: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signing_key: Option<String>,
    #[serde(default, skip_serializing_if = "is_default_signing_format")]
    pub signing_format: SigningFormat,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub require_signing: bool,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum SigningFormat {
    #[default]
    Openpgp,
    Ssh,
}

impl SigningFormat {
    pub fn as_git_value(self) -> &'static str {
        match self {
            Self::Openpgp => "openpgp",
            Self::Ssh => "ssh",
        }
    }
}

fn is_default_signing_format(value: &SigningFormat) -> bool {
    *value == SigningFormat::default()
}

fn default_hostname() -> String {
    "github.com".to_string()
}

impl Profile {
    pub fn validate(&self) -> Result<(), GitPersonaError> {
        for (label, value) in [
            ("GitHub user", &self.github_user),
            ("Git name", &self.git_name),
            ("Git email", &self.git_email),
        ] {
            if value.trim().is_empty() {
                return Err(GitPersonaError::usage(format!("{label} cannot be empty")));
            }
        }
        validate_hostname(&self.hostname)?;
        if self
            .ssh_key
            .as_ref()
            .is_some_and(|key| key.as_os_str().is_empty())
        {
            return Err(GitPersonaError::usage("SSH key path cannot be empty"));
        }
        if self
            .allowed_owners
            .iter()
            .any(|owner| owner.trim().is_empty())
        {
            return Err(GitPersonaError::usage("allowed owners cannot be empty"));
        }
        if self
            .signing_key
            .as_ref()
            .is_some_and(|key| key.trim().is_empty())
        {
            return Err(GitPersonaError::usage("signing key cannot be empty"));
        }
        if self.require_signing && self.signing_key.is_none() {
            return Err(GitPersonaError::usage(
                "required commit signing needs a signing key",
            ));
        }
        Ok(())
    }

    pub fn validate_local_resources(&self) -> Result<(), GitPersonaError> {
        if let Some(key) = &self.ssh_key {
            let expanded = expand_path(key)?;
            if !expanded.is_file() {
                return Err(GitPersonaError::usage(format!(
                    "SSH key does not exist or is not a file: {}",
                    expanded.display()
                )));
            }
        }
        if self.signing_format == SigningFormat::Ssh {
            if let Some(key) = &self.signing_key {
                if !key.starts_with("key::") {
                    let expanded = expand_path(Path::new(key))?;
                    if !expanded.is_file() {
                        return Err(GitPersonaError::usage(format!(
                            "SSH signing key does not exist or is not a file: {}",
                            expanded.display()
                        )));
                    }
                }
            }
        }
        Ok(())
    }
}

pub fn validate_profile_name(name: &str) -> Result<(), GitPersonaError> {
    let pattern = Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$").expect("valid regex");
    if pattern.is_match(name) {
        Ok(())
    } else {
        Err(GitPersonaError::usage(
            "profile name must match [A-Za-z0-9][A-Za-z0-9._-]{0,63}",
        ))
    }
}

pub fn validate_hostname(hostname: &str) -> Result<(), GitPersonaError> {
    let value = hostname.trim();
    if value.is_empty()
        || value.contains("://")
        || value.contains('/')
        || value.chars().any(char::is_whitespace)
    {
        Err(GitPersonaError::usage(
            "hostname must be a host name without a scheme or path",
        ))
    } else {
        Ok(())
    }
}

pub fn expand_path(path: &Path) -> Result<PathBuf, GitPersonaError> {
    let text = path.to_string_lossy();
    if text == "~" || text.starts_with("~/") || text.starts_with("~\\") {
        let base = BaseDirs::new()
            .ok_or_else(|| GitPersonaError::dependency("could not determine the home directory"))?;
        let suffix = text.trim_start_matches('~').trim_start_matches(['/', '\\']);
        Ok(base.home_dir().join(suffix))
    } else {
        Ok(path.to_path_buf())
    }
}

#[derive(Debug, Clone)]
pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    pub fn discover() -> Result<Self, GitPersonaError> {
        if let Some(path) = env::var_os("GITPERSONA_CONFIG") {
            return Ok(Self {
                path: PathBuf::from(path),
            });
        }
        let dirs = ProjectDirs::from("dev", "GitPersona", "GitPersona").ok_or_else(|| {
            GitPersonaError::dependency("could not determine the operating-system config directory")
        })?;
        Ok(Self {
            path: dirs.config_dir().join("config.toml"),
        })
    }

    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<Config, GitPersonaError> {
        if !self.path.exists() {
            return Ok(Config::default());
        }
        let text = fs::read_to_string(&self.path).map_err(|e| {
            GitPersonaError::dependency(format!("could not read {}: {e}", self.path.display()))
        })?;
        let config: Config = toml::from_str(&text).map_err(|e| {
            GitPersonaError::usage(format!("invalid config {}: {e}", self.path.display()))
        })?;
        if config.schema_version != 1 {
            return Err(GitPersonaError::usage(format!(
                "unsupported config schema version {}",
                config.schema_version
            )));
        }
        for (name, profile) in &config.profiles {
            validate_profile_name(name)?;
            profile.validate()?;
        }
        Ok(config)
    }

    pub fn update<T>(
        &self,
        operation: impl FnOnce(&mut Config) -> Result<T, GitPersonaError>,
    ) -> Result<T, GitPersonaError> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| GitPersonaError::dependency("config path has no parent directory"))?;
        fs::create_dir_all(parent).map_err(|e| {
            GitPersonaError::dependency(format!("could not create {}: {e}", parent.display()))
        })?;
        let lock_path = parent.join("config.lock");
        let lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|e| GitPersonaError::dependency(format!("could not open config lock: {e}")))?;
        lock.lock_exclusive()
            .map_err(|e| GitPersonaError::dependency(format!("could not lock config: {e}")))?;
        let mut config = self.load()?;
        let result = operation(&mut config)?;
        let rendered = toml::to_string_pretty(&config)
            .map_err(|e| GitPersonaError::dependency(format!("could not serialize config: {e}")))?;
        let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| {
            GitPersonaError::dependency(format!("could not create temporary config: {e}"))
        })?;
        temp.write_all(rendered.as_bytes())
            .and_then(|_| temp.as_file().sync_all())
            .map_err(|e| {
                GitPersonaError::dependency(format!("could not write temporary config: {e}"))
            })?;
        temp.persist(&self.path).map_err(|e| {
            GitPersonaError::dependency(format!("could not persist config: {}", e.error))
        })?;
        FileExt::unlock(&lock).ok();
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_name_validation() {
        assert!(validate_profile_name("work-1.main").is_ok());
        assert!(validate_profile_name("bad name").is_err());
        assert!(validate_profile_name(&"a".repeat(65)).is_err());
    }

    #[test]
    fn config_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::at(dir.path().join("config.toml"));
        store
            .update(|config| {
                config.profiles.insert(
                    "work".into(),
                    Profile {
                        github_user: "alice".into(),
                        git_name: "Alice".into(),
                        git_email: "a@example.com".into(),
                        hostname: "github.com".into(),
                        ssh_key: None,
                        allowed_owners: vec!["Org".into()],
                        signing_key: None,
                        signing_format: SigningFormat::Openpgp,
                        require_signing: false,
                    },
                );
                Ok(())
            })
            .unwrap();
        store
            .update(|config| {
                config.profiles.get_mut("work").unwrap().git_name = "Alice Updated".into();
                Ok(())
            })
            .unwrap();
        assert_eq!(store.load().unwrap().profiles["work"].github_user, "alice");
        assert_eq!(
            store.load().unwrap().profiles["work"].git_name,
            "Alice Updated"
        );
    }
}
