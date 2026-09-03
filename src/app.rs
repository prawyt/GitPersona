use crate::{
    check::{self, CheckOptions},
    cli::{
        BindArgs, Cli, Command, DoctorArgs, HookMode, HooksCommand, InspectArgs, ProfileCommand,
        ProfileImportArgs, ProfileMutationArgs, SshCommand,
    },
    clone_repo,
    config::{ConfigStore, Profile, validate_profile_name},
    directory,
    error::GitPersonaError,
    git::Git,
    github::GitHub,
    hooks::HookManager,
    process::{Runner, os_args},
    service::GitPersonaService,
};
use clap::CommandFactory;
use dialoguer::{Confirm, Input};
use std::{
    io::{self, IsTerminal},
    time::Duration,
};

pub fn run(cli: Cli, runner: &dyn Runner) -> Result<u8, GitPersonaError> {
    let store = ConfigStore::discover()?;
    match cli.command {
        Command::Profile { command } => profile_command(command, &store, runner),
        Command::Use { profile } => use_profile(&profile, &store, runner),
        Command::Clone(args) => clone_repo::execute(args, &store, runner),
        Command::Bind(args) => bind(args, &store, runner),
        Command::Unbind(args) => {
            git_for(runner, args.repo.as_deref()).unbind()?;
            println!("Repository unbound; original Git settings restored.");
            Ok(0)
        }
        Command::Status(args) => inspect(args, &store, runner, false),
        Command::Check(args) => inspect(args, &store, runner, true),
        Command::Hooks { command } => hooks(command, runner),
        Command::Directory { command } => directory::execute(command, &store, runner),
        Command::Doctor(args) => doctor(args, &store, runner),
        Command::Ssh { command } => ssh(command, &store, runner),
        Command::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "gitpersona", &mut io::stdout());
            Ok(0)
        }
    }
}

fn profile_command(
    command: ProfileCommand,
    store: &ConfigStore,
    runner: &dyn Runner,
) -> Result<u8, GitPersonaError> {
    match command {
        ProfileCommand::Add(args) => {
            validate_profile_name(&args.name)?;
            let profile = profile_from_args(&args, None, true)?;
            profile.validate()?;
            profile.validate_local_resources()?;
            store.update(|config| {
                if config.profiles.contains_key(&args.name) {
                    return Err(GitPersonaError::usage(format!(
                        "profile '{}' already exists",
                        args.name
                    )));
                }
                config.profiles.insert(args.name.clone(), profile);
                Ok(())
            })?;
            println!("Profile '{}' added.", args.name);
            Ok(0)
        }
        ProfileCommand::Import(args) => import_profile(args, store, runner),
        ProfileCommand::Edit(args) => {
            validate_profile_name(&args.name)?;
            store.update(|config| {
                let old = config.profiles.get(&args.name).cloned().ok_or_else(|| {
                    GitPersonaError::usage(format!("profile '{}' does not exist", args.name))
                })?;
                let profile = profile_from_args(&args, Some(&old), false)?;
                profile.validate()?;
                profile.validate_local_resources()?;
                config.profiles.insert(args.name.clone(), profile);
                Ok(())
            })?;
            println!(
                "Profile '{}' updated. Rebind repositories to apply changed settings.",
                args.name
            );
            directory::sync_profile(&args.name, store)?;
            Ok(0)
        }
        ProfileCommand::List { json } => {
            let config = store.load()?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&config.profiles).map_err(json_error)?
                );
            } else if config.profiles.is_empty() {
                println!("No profiles configured.");
            } else {
                for (name, profile) in config.profiles {
                    println!("{name}\t{}@{}", profile.github_user, profile.hostname);
                }
            }
            Ok(0)
        }
        ProfileCommand::Show { name, json } => {
            let config = store.load()?;
            let profile = config.profiles.get(&name).ok_or_else(|| {
                GitPersonaError::usage(format!("profile '{name}' does not exist"))
            })?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(profile).map_err(json_error)?
                );
            } else {
                println!(
                    "Profile:        {name}\nGitHub user:    {}\nHostname:       {}\nGit author:     {}\nGit email:      {}\nSSH key:        {}\nAllowed owners: {}\nSigning key:    {}\nSigning format: {}\nRequire signing: {}",
                    profile.github_user,
                    profile.hostname,
                    profile.git_name,
                    profile.git_email,
                    profile
                        .ssh_key
                        .as_ref()
                        .map_or_else(|| "(none)".into(), |p| p.display().to_string()),
                    if profile.allowed_owners.is_empty() {
                        "(any)".into()
                    } else {
                        profile.allowed_owners.join(", ")
                    },
                    profile.signing_key.as_deref().unwrap_or("(none)"),
                    profile.signing_format.as_git_value(),
                    profile.require_signing,
                );
            }
            Ok(0)
        }
        ProfileCommand::Remove { name, yes } => {
            let config = store.load()?;
            if !config.profiles.contains_key(&name) {
                return Err(GitPersonaError::usage(format!(
                    "profile '{name}' does not exist"
                )));
            }
            if config.directories.iter().any(|rule| rule.profile == name) {
                return Err(GitPersonaError::usage(format!(
                    "profile '{name}' has directory rules; remove them first"
                )));
            }
            if !yes {
                if !io::stdin().is_terminal() {
                    return Err(GitPersonaError::usage(
                        "profile removal requires --yes in non-interactive mode",
                    ));
                }
                if !Confirm::new()
                    .with_prompt(format!(
                        "Remove profile '{name}'? Bound repositories will report a missing profile"
                    ))
                    .default(false)
                    .interact()
                    .map_err(prompt_error)?
                {
                    return Ok(0);
                }
            }
            store.update(|config| {
                config.profiles.remove(&name);
                Ok(())
            })?;
            println!("Profile '{name}' removed.");
            Ok(0)
        }
        ProfileCommand::Rename { old_name, new_name } => {
            let service = GitPersonaService::new(store.clone(), runner);
            service.rename_profile(&old_name, &new_name)?;
            println!("Profile '{old_name}' renamed to '{new_name}'.");
            Ok(0)
        }
    }
}

fn import_profile(
    args: ProfileImportArgs,
    store: &ConfigStore,
    runner: &dyn Runner,
) -> Result<u8, GitPersonaError> {
    validate_profile_name(&args.name)?;
    if store.load()?.profiles.contains_key(&args.name) {
        return Err(GitPersonaError::usage(format!(
            "profile '{}' already exists",
            args.name
        )));
    }
    let git = git_for(runner, args.repo.as_deref());
    git.ensure_repo()?;
    let remote = git.remote(&args.remote)?.ok_or_else(|| {
        GitPersonaError::usage(format!("remote '{}' is not configured", args.remote))
    })?;
    let required_git = |key: &str, label: &str| {
        git.get(key, false)?.ok_or_else(|| {
            GitPersonaError::usage(format!("cannot import profile: {label} is not configured"))
        })
    };
    let github_user = GitHub::new(runner)
        .active_account(&remote.hostname)?
        .ok_or_else(|| {
            GitPersonaError::usage(format!(
                "cannot import profile: GitHub CLI has no active account on {}",
                remote.hostname
            ))
        })?;
    let signing_format = match git.get("gpg.format", false)?.as_deref() {
        Some("ssh") => crate::config::SigningFormat::Ssh,
        _ => crate::config::SigningFormat::Openpgp,
    };
    let profile = Profile {
        github_user,
        git_name: required_git("user.name", "Git author name")?,
        git_email: required_git("user.email", "Git author email")?,
        hostname: remote.hostname,
        ssh_key: None,
        allowed_owners: if args.no_owner {
            Vec::new()
        } else {
            vec![remote.owner]
        },
        signing_key: git.get("user.signingKey", false)?,
        signing_format,
        require_signing: git
            .get("commit.gpgSign", false)?
            .is_some_and(|value| value.eq_ignore_ascii_case("true")),
    };
    profile.validate()?;
    profile.validate_local_resources()?;
    store.update(|config| {
        if config.profiles.contains_key(&args.name) {
            return Err(GitPersonaError::usage(format!(
                "profile '{}' already exists",
                args.name
            )));
        }
        config.profiles.insert(args.name.clone(), profile);
        Ok(())
    })?;
    println!(
        "Profile '{}' imported from the current repository. SSH keys are never inferred; add one explicitly if needed.",
        args.name
    );
    Ok(0)
}

fn profile_from_args(
    args: &ProfileMutationArgs,
    old: Option<&Profile>,
    require: bool,
) -> Result<Profile, GitPersonaError> {
    let github_user = required_value(
        "GitHub user",
        args.github_user
            .clone()
            .or_else(|| old.map(|p| p.github_user.clone())),
        require,
    )?;
    let git_name = required_value(
        "Git author name",
        args.git_name
            .clone()
            .or_else(|| old.map(|p| p.git_name.clone())),
        require,
    )?;
    let git_email = required_value(
        "Git author email",
        args.git_email
            .clone()
            .or_else(|| old.map(|p| p.git_email.clone())),
        require,
    )?;
    let hostname = args
        .hostname
        .clone()
        .or_else(|| old.map(|p| p.hostname.clone()))
        .unwrap_or_else(|| "github.com".into());
    let ssh_key = if args.clear_ssh_key {
        None
    } else {
        args.ssh_key
            .clone()
            .or_else(|| old.and_then(|p| p.ssh_key.clone()))
    };
    let allowed_owners = if args.clear_allowed_owners {
        vec![]
    } else if args.allowed_owners.is_empty() {
        old.map_or_else(Vec::new, |p| p.allowed_owners.clone())
    } else {
        args.allowed_owners.clone()
    };
    let signing_key = if args.clear_signing_key {
        None
    } else {
        args.signing_key
            .clone()
            .or_else(|| old.and_then(|profile| profile.signing_key.clone()))
    };
    let signing_format = args
        .signing_format
        .or_else(|| old.map(|profile| profile.signing_format))
        .unwrap_or_default();
    let require_signing = if args.require_signing {
        true
    } else if args.no_require_signing || args.clear_signing_key {
        false
    } else {
        old.is_some_and(|profile| profile.require_signing)
    };
    Ok(Profile {
        github_user,
        git_name,
        git_email,
        hostname,
        ssh_key,
        allowed_owners,
        signing_key,
        signing_format,
        require_signing,
    })
}

fn required_value(
    label: &str,
    value: Option<String>,
    require: bool,
) -> Result<String, GitPersonaError> {
    if let Some(value) = value {
        return Ok(value);
    }
    if !require {
        return Err(GitPersonaError::usage(format!(
            "{label} is missing from the existing profile"
        )));
    }
    if !io::stdin().is_terminal() {
        return Err(GitPersonaError::usage(format!(
            "missing required {label}; provide the corresponding flag"
        )));
    }
    Input::<String>::new()
        .with_prompt(label)
        .interact_text()
        .map_err(prompt_error)
}

fn use_profile(
    name: &str,
    store: &ConfigStore,
    runner: &dyn Runner,
) -> Result<u8, GitPersonaError> {
    let config = store.load()?;
    let profile = config
        .profiles
        .get(name)
        .ok_or_else(|| GitPersonaError::usage(format!("profile '{name}' does not exist")))?;
    GitHub::new(runner).switch(&profile.hostname, &profile.github_user)?;
    println!(
        "GitHub CLI is now using {} on {}.",
        profile.github_user, profile.hostname
    );
    Ok(0)
}

fn bind(args: BindArgs, store: &ConfigStore, runner: &dyn Runner) -> Result<u8, GitPersonaError> {
    let config = store.load()?;
    let profile = config.profiles.get(&args.profile).ok_or_else(|| {
        GitPersonaError::usage(format!("profile '{}' does not exist", args.profile))
    })?;
    let git = git_for(runner, args.repo.as_deref());
    git.ensure_repo()?;
    let remote = git.remote(&args.remote)?;
    let old_name = git.get("gitpersona.profile", true)?;
    let old_profile = old_name.as_ref().and_then(|name| config.profiles.get(name));
    let github = GitHub::new(runner);
    let previous = if args.switch {
        github.active_account(&profile.hostname)?
    } else {
        None
    };
    if args.switch {
        github.switch(&profile.hostname, &profile.github_user)?;
    }
    if let Err(error) = git.bind(
        &args.profile,
        profile,
        remote.as_ref(),
        args.force,
        old_profile,
    ) {
        if args.switch {
            if let Some(previous) = previous {
                if let Err(rollback) = github.switch(&profile.hostname, &previous) {
                    return Err(GitPersonaError::dependency(format!(
                        "{error}; GitHub CLI rollback also failed: {rollback}"
                    )));
                }
            }
        }
        return Err(error);
    }
    println!("Repository bound to profile '{}'.", args.profile);
    if !args.switch {
        println!(
            "GitHub CLI was not switched. Run 'gitpersona use {}' or rebind with --switch if needed.",
            args.profile
        );
    }
    Ok(0)
}

fn inspect(
    args: InspectArgs,
    store: &ConfigStore,
    runner: &dyn Runner,
    enforce: bool,
) -> Result<u8, GitPersonaError> {
    let config = store.load()?;
    let network = !matches!(args.hook, Some(HookMode::PreCommit));
    let report = check::inspect_at(
        runner,
        &config,
        args.repo
            .as_deref()
            .unwrap_or_else(|| std::path::Path::new(".")),
        CheckOptions {
            remote_name: &args.remote,
            network,
        },
    )?;
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(json_error)?
        );
    } else {
        print!("{}", report.render_human());
    }
    if enforce && !report.enforceable() {
        Ok(1)
    } else {
        Ok(0)
    }
}

fn hooks(command: HooksCommand, runner: &dyn Runner) -> Result<u8, GitPersonaError> {
    let hooks = HookManager::new(runner);
    match command {
        HooksCommand::Install => {
            hooks.install()?;
            println!("GitPersona pre-commit and pre-push hooks installed.");
        }
        HooksCommand::Status => println!("{}", hooks.status()?),
        HooksCommand::Uninstall => {
            hooks.uninstall()?;
            println!("GitPersona hooks removed.");
        }
    }
    Ok(0)
}

fn doctor(
    args: DoctorArgs,
    store: &ConfigStore,
    runner: &dyn Runner,
) -> Result<u8, GitPersonaError> {
    let service = GitPersonaService::new(store.clone(), runner);
    let report = service.doctor()?;
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(json_error)?
        );
        return Ok(if report.healthy { 0 } else { 3 });
    }
    let config = store.load()?;
    println!(
        "Config: OK ({}, {} profiles)",
        store.path().display(),
        config.profiles.len()
    );
    let mut unavailable = false;
    for (program, args) in [
        ("git", vec!["--version"]),
        ("gh", vec!["--version"]),
        ("ssh", vec!["-V"]),
    ] {
        match runner.run(program, &os_args(&args), Duration::from_secs(10)) {
            Ok(output) if output.code.is_some() => println!("{program}: OK"),
            Ok(_) => {
                unavailable = true;
                println!("{program}: unavailable (timed out)");
            }
            Err(error) => {
                unavailable = true;
                println!("{program}: unavailable ({error})");
            }
        }
    }
    for (name, profile) in &config.profiles {
        if let Err(error) = profile.validate_local_resources() {
            unavailable = true;
            println!("profile {name}: unavailable ({error})");
        }
    }
    println!(
        "HTTPS profiles require GitHub CLI's credential helper. Run 'gh auth setup-git --hostname <host>' if status reports it missing."
    );
    Ok(if unavailable { 3 } else { 0 })
}

fn ssh(
    command: SshCommand,
    store: &ConfigStore,
    runner: &dyn Runner,
) -> Result<u8, GitPersonaError> {
    match command {
        SshCommand::Test { profile, json } => {
            let report = GitPersonaService::new(store.clone(), runner).ssh_test(&profile)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(json_error)?
                );
            } else {
                println!("{}", report.message);
            }
            Ok(
                if matches!(report.status, crate::api::SshTestStatus::Verified) {
                    0
                } else {
                    3
                },
            )
        }
    }
}

fn git_for<'a>(runner: &'a dyn Runner, repository: Option<&std::path::Path>) -> Git<'a> {
    repository.map_or_else(|| Git::new(runner), |path| Git::at(runner, path))
}

fn prompt_error(error: dialoguer::Error) -> GitPersonaError {
    GitPersonaError::dependency(format!("interactive prompt failed: {error}"))
}
fn json_error(error: serde_json::Error) -> GitPersonaError {
    GitPersonaError::dependency(format!("could not serialize JSON: {error}"))
}
