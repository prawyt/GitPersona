use crate::{
    cli::{CloneArgs, CloneProtocol},
    config::ConfigStore,
    error::GitPersonaError,
    git::Git,
    github::GitHub,
    process::Runner,
    remote::{RemoteProtocol, parse_repository},
};
use std::{env, ffi::OsString, path::PathBuf, time::Duration};

const CLONE_TIMEOUT: Duration = Duration::from_secs(300);

pub fn execute(
    args: CloneArgs,
    store: &ConfigStore,
    runner: &dyn Runner,
) -> Result<u8, GitPersonaError> {
    let config = store.load()?;
    let profile = config.profiles.get(&args.profile).ok_or_else(|| {
        GitPersonaError::usage(format!("profile '{}' does not exist", args.profile))
    })?;
    let source = parse_repository(&args.repository, &profile.hostname)?;
    if !source.hostname.eq_ignore_ascii_case(&profile.hostname) {
        return Err(GitPersonaError::usage(format!(
            "repository host '{}' does not match profile host '{}'",
            source.hostname, profile.hostname
        )));
    }
    if !profile.allowed_owners.is_empty()
        && !profile
            .allowed_owners
            .iter()
            .any(|owner| owner.eq_ignore_ascii_case(&source.owner))
    {
        return Err(GitPersonaError::check(format!(
            "repository owner '{}' is not allowed by profile '{}'",
            source.owner, args.profile
        )));
    }

    let protocol = match args.protocol {
        Some(CloneProtocol::Ssh) => RemoteProtocol::Ssh,
        Some(CloneProtocol::Https) => RemoteProtocol::Https,
        None if profile.ssh_key.is_some() => RemoteProtocol::Ssh,
        None => RemoteProtocol::Https,
    };
    if protocol == RemoteProtocol::Ssh {
        Git::expected_ssh_command(profile)?;
    }
    validate_remote_name(&args.remote)?;

    let destination = match args.directory {
        Some(path) => path,
        None => PathBuf::from(&source.repository),
    };
    if destination.exists() {
        return Err(GitPersonaError::usage(format!(
            "clone destination already exists: {}",
            destination.display()
        )));
    }
    let github = GitHub::new(runner);
    let previous = github.active_account(&profile.hostname)?;
    if args.no_switch {
        if protocol == RemoteProtocol::Https
            && !previous
                .as_deref()
                .is_some_and(|user| user.eq_ignore_ascii_case(&profile.github_user))
        {
            return Err(GitPersonaError::check(format!(
                "HTTPS clone requires GitHub CLI to be active as {}; omit --no-switch to switch explicitly",
                profile.github_user
            )));
        }
    } else {
        github.switch(&profile.hostname, &profile.github_user)?;
    }

    let clone_url = source.as_url(protocol, &profile.hostname);
    let clone_args = vec![
        OsString::from("clone"),
        OsString::from("--origin"),
        OsString::from(&args.remote),
        OsString::from(&clone_url),
        destination.clone().into_os_string(),
    ];
    let output = runner.run_git(&clone_args, CLONE_TIMEOUT)?;
    if !output.success() {
        restore_account(
            &github,
            &profile.hostname,
            previous.as_deref(),
            args.no_switch,
        )?;
        return Err(GitPersonaError::dependency(format!(
            "git clone failed: {}",
            output.combined().trim()
        )));
    }

    let absolute = if destination.is_absolute() {
        destination
    } else {
        env::current_dir()
            .map_err(|error| {
                GitPersonaError::dependency(format!("could not resolve clone destination: {error}"))
            })?
            .join(destination)
    };
    let git = Git::at(runner, &absolute);
    let remote = git.remote(&args.remote)?;
    if let Err(error) = git.bind(&args.profile, profile, remote.as_ref(), false, None) {
        let _ = std::fs::remove_dir_all(&absolute);
        restore_account(
            &github,
            &profile.hostname,
            previous.as_deref(),
            args.no_switch,
        )?;
        return Err(GitPersonaError::dependency(format!(
            "binding to profile '{}' failed; the cloned directory has been removed: {error}",
            args.profile
        )));
    }

    println!(
        "Cloned {}/{} to {} and bound it to profile '{}'.",
        source.owner,
        source.repository,
        absolute.display(),
        args.profile
    );
    Ok(0)
}

fn restore_account(
    github: &GitHub<'_>,
    hostname: &str,
    previous: Option<&str>,
    no_switch: bool,
) -> Result<(), GitPersonaError> {
    if !no_switch {
        if let Some(previous) = previous {
            github.switch(hostname, previous)?;
        }
    }
    Ok(())
}

fn validate_remote_name(name: &str) -> Result<(), GitPersonaError> {
    if !name.is_empty()
        && name
            .chars()
            .all(|value| value.is_ascii_alphanumeric() || matches!(value, '.' | '_' | '-'))
    {
        Ok(())
    } else {
        Err(GitPersonaError::usage(
            "remote name may contain only letters, numbers, '.', '_' and '-'",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_remote_names() {
        assert!(validate_remote_name("origin").is_ok());
        assert!(validate_remote_name("bad name").is_err());
    }
}
