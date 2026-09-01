use crate::{
    config::{Config, Profile},
    error::GitPersonaError,
    git::Git,
    github::GitHub,
    process::Runner,
    remote::{RemoteInfo, RemoteProtocol},
    ssh::{self, SshIdentity},
};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Ok,
    Warning,
    Failure,
    Unverified,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CheckItem {
    pub id: String,
    pub status: CheckStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OverallStatus {
    Ok,
    Warning,
    Failure,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckReport {
    pub repository: String,
    pub profile: Option<String>,
    pub remote: Option<RemoteInfo>,
    pub overall: OverallStatus,
    pub checks: Vec<CheckItem>,
}

impl CheckReport {
    pub fn enforceable(&self) -> bool {
        !self
            .checks
            .iter()
            .any(|check| matches!(check.status, CheckStatus::Failure | CheckStatus::Unverified))
    }

    pub fn render_human(&self) -> String {
        let mut output = format!(
            "Repository: {}\nProfile:    {}\n",
            self.repository,
            self.profile.as_deref().unwrap_or("(none)")
        );
        if let Some(remote) = &self.remote {
            output.push_str(&format!(
                "Remote:     {} ({}/{})\n",
                remote.url, remote.owner, remote.repository
            ));
        }
        output.push('\n');
        for check in &self.checks {
            output.push_str(&format!(
                "[{:<10}] {}\n",
                format!("{:?}", check.status).to_uppercase(),
                check.message
            ));
        }
        output.push_str(&format!("\nStatus: {:?}\n", self.overall).to_uppercase());
        output
    }
}

pub struct CheckOptions<'a> {
    pub remote_name: &'a str,
    pub network: bool,
}

pub fn inspect(
    runner: &dyn Runner,
    config: &Config,
    options: CheckOptions<'_>,
) -> Result<CheckReport, GitPersonaError> {
    let git = Git::new(runner);
    let root = git.ensure_repo()?;
    let bound = git.get("gitpersona.profile", true)?;
    let remote = git.remote(options.remote_name)?;
    let mut checks = Vec::new();

    let Some(profile_name) = bound.clone() else {
        checks.push(item(
            "binding",
            CheckStatus::Failure,
            None,
            None,
            "repository is not bound to a GitPersona profile",
        ));
        return Ok(finish(root.display().to_string(), None, remote, checks));
    };
    checks.push(item(
        "binding",
        CheckStatus::Ok,
        Some(profile_name.clone()),
        Some(profile_name.clone()),
        "repository binding is present",
    ));

    let Some(profile) = config.profiles.get(&profile_name) else {
        checks.push(item(
            "profile",
            CheckStatus::Failure,
            Some(profile_name.clone()),
            None,
            "bound profile does not exist in the user configuration",
        ));
        return Ok(finish(
            root.display().to_string(),
            Some(profile_name),
            remote,
            checks,
        ));
    };
    check_equal(
        &mut checks,
        "git_name",
        &profile.git_name,
        git.get("user.name", true)?.as_deref(),
        "Git author name",
    );
    check_equal(
        &mut checks,
        "git_email",
        &profile.git_email,
        git.get("user.email", true)?.as_deref(),
        "Git author email",
    );

    match &remote {
        None => checks.push(item(
            "remote",
            CheckStatus::Warning,
            Some(options.remote_name.into()),
            None,
            "configured remote is not available",
        )),
        Some(remote) => inspect_remote(runner, &git, profile, remote, options.network, &mut checks),
    }
    Ok(finish(
        root.display().to_string(),
        Some(profile_name),
        remote,
        checks,
    ))
}

fn inspect_remote(
    runner: &dyn Runner,
    git: &Git<'_>,
    profile: &Profile,
    remote: &RemoteInfo,
    network: bool,
    checks: &mut Vec<CheckItem>,
) {
    check_equal_case_insensitive(
        checks,
        "hostname",
        &profile.hostname,
        Some(&remote.hostname),
        "remote hostname",
    );
    if profile.allowed_owners.is_empty() {
        checks.push(item(
            "owner",
            CheckStatus::Ok,
            None,
            Some(remote.owner.clone()),
            "remote owner is reported; this profile has no owner restriction",
        ));
    } else if profile
        .allowed_owners
        .iter()
        .any(|owner| owner.eq_ignore_ascii_case(&remote.owner))
    {
        checks.push(item(
            "owner",
            CheckStatus::Ok,
            Some(profile.allowed_owners.join(", ")),
            Some(remote.owner.clone()),
            "remote owner is allowed",
        ));
    } else {
        checks.push(item(
            "owner",
            CheckStatus::Failure,
            Some(profile.allowed_owners.join(", ")),
            Some(remote.owner.clone()),
            "remote owner is not allowed by this profile",
        ));
    }

    if matches!(remote.protocol, RemoteProtocol::Ssh) {
        match Git::expected_ssh_command(profile) {
            Ok(expected) => check_equal(
                checks,
                "ssh_command",
                &expected,
                git.get("core.sshCommand", true).ok().flatten().as_deref(),
                "repository SSH command",
            ),
            Err(error) => checks.push(item(
                "ssh_command",
                CheckStatus::Failure,
                None,
                None,
                error.to_string(),
            )),
        }
    }

    if !network {
        return;
    }
    let github = GitHub::new(runner);
    match github.active_account(&profile.hostname) {
        Ok(Some(active)) if active.eq_ignore_ascii_case(&profile.github_user) => checks.push(item(
            "github_cli",
            CheckStatus::Ok,
            Some(profile.github_user.clone()),
            Some(active),
            "GitHub CLI account matches the profile",
        )),
        Ok(Some(active)) => checks.push(item(
            "github_cli",
            CheckStatus::Failure,
            Some(profile.github_user.clone()),
            Some(active),
            "GitHub CLI account does not match the profile",
        )),
        Ok(None) => checks.push(item(
            "github_cli",
            CheckStatus::Unverified,
            Some(profile.github_user.clone()),
            None,
            "GitHub CLI has no active account for this host",
        )),
        Err(error) => checks.push(item(
            "github_cli",
            CheckStatus::Unverified,
            Some(profile.github_user.clone()),
            None,
            format!("could not verify GitHub CLI identity: {error}"),
        )),
    }

    match remote.protocol {
        RemoteProtocol::Ssh => match ssh::verify(runner, profile) {
            Ok(SshIdentity::Verified(user)) => checks.push(item(
                "ssh_identity",
                CheckStatus::Ok,
                Some(profile.github_user.clone()),
                Some(user),
                "SSH authenticates as the expected GitHub user",
            )),
            Ok(SshIdentity::Rejected(user)) => checks.push(item(
                "ssh_identity",
                CheckStatus::Failure,
                Some(profile.github_user.clone()),
                Some(user),
                "SSH authenticates as a different GitHub user",
            )),
            Ok(SshIdentity::Unavailable(reason)) => checks.push(item(
                "ssh_identity",
                CheckStatus::Unverified,
                Some(profile.github_user.clone()),
                None,
                format!("could not verify SSH identity: {reason}"),
            )),
            Err(error) => checks.push(item(
                "ssh_identity",
                CheckStatus::Unverified,
                Some(profile.github_user.clone()),
                None,
                format!("could not verify SSH identity: {error}"),
            )),
        },
        RemoteProtocol::Https | RemoteProtocol::Http => {
            match git.has_compatible_credential_helper(&profile.hostname) {
                Ok(true) => checks.push(item(
                    "credential_helper",
                    CheckStatus::Ok,
                    Some("gh auth git-credential".into()),
                    Some("configured".into()),
                    "GitHub CLI credential helper is configured",
                )),
                Ok(false) => checks.push(item(
                    "credential_helper",
                    CheckStatus::Unverified,
                    Some("gh auth git-credential".into()),
                    None,
                    "HTTPS push identity is unverified; run gh auth setup-git for this host",
                )),
                Err(error) => checks.push(item(
                    "credential_helper",
                    CheckStatus::Unverified,
                    None,
                    None,
                    format!("could not inspect Git credential helper: {error}"),
                )),
            }
        }
    }
}

fn check_equal(
    checks: &mut Vec<CheckItem>,
    id: &str,
    expected: &str,
    actual: Option<&str>,
    label: &str,
) {
    if actual == Some(expected) {
        checks.push(item(
            id,
            CheckStatus::Ok,
            Some(expected.into()),
            actual.map(str::to_string),
            format!("{label} matches the profile"),
        ));
    } else {
        checks.push(item(
            id,
            CheckStatus::Failure,
            Some(expected.into()),
            actual.map(str::to_string),
            format!("{label} does not match the profile"),
        ));
    }
}

fn check_equal_case_insensitive(
    checks: &mut Vec<CheckItem>,
    id: &str,
    expected: &str,
    actual: Option<&str>,
    label: &str,
) {
    if actual.is_some_and(|value| value.eq_ignore_ascii_case(expected)) {
        checks.push(item(
            id,
            CheckStatus::Ok,
            Some(expected.into()),
            actual.map(str::to_string),
            format!("{label} matches the profile"),
        ));
    } else {
        checks.push(item(
            id,
            CheckStatus::Failure,
            Some(expected.into()),
            actual.map(str::to_string),
            format!("{label} does not match the profile"),
        ));
    }
}

fn item(
    id: &str,
    status: CheckStatus,
    expected: Option<String>,
    actual: Option<String>,
    message: impl Into<String>,
) -> CheckItem {
    CheckItem {
        id: id.into(),
        status,
        expected,
        actual,
        message: message.into(),
    }
}

fn finish(
    repository: String,
    profile: Option<String>,
    remote: Option<RemoteInfo>,
    checks: Vec<CheckItem>,
) -> CheckReport {
    let overall = if checks
        .iter()
        .any(|check| check.status == CheckStatus::Failure)
    {
        OverallStatus::Failure
    } else if checks
        .iter()
        .any(|check| matches!(check.status, CheckStatus::Warning | CheckStatus::Unverified))
    {
        OverallStatus::Warning
    } else {
        OverallStatus::Ok
    };
    CheckReport {
        repository,
        profile,
        remote,
        overall,
        checks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unverified_is_not_enforceable() {
        let report = finish(
            "repo".into(),
            None,
            None,
            vec![item(
                "network",
                CheckStatus::Unverified,
                None,
                None,
                "offline",
            )],
        );
        assert!(!report.enforceable());
        assert_eq!(report.overall, OverallStatus::Warning);
    }
}
