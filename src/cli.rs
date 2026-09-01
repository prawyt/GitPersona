use crate::config::SigningFormat;
use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "gitpersona",
    version,
    about = "Manage and verify local GitHub identities"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
    },
    Use {
        profile: String,
    },
    Clone(CloneArgs),
    Bind(BindArgs),
    Unbind,
    Status(InspectArgs),
    Check(InspectArgs),
    Hooks {
        #[command(subcommand)]
        command: HooksCommand,
    },
    Doctor,
    Completions {
        shell: Shell,
    },
}

#[derive(Debug, Args)]
pub struct CloneArgs {
    pub profile: String,
    pub repository: String,
    pub directory: Option<PathBuf>,
    #[arg(long, value_enum)]
    pub protocol: Option<CloneProtocol>,
    #[arg(long, default_value = "origin")]
    pub remote: String,
    #[arg(long)]
    pub no_switch: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CloneProtocol {
    Ssh,
    Https,
}

#[derive(Debug, Subcommand)]
pub enum ProfileCommand {
    Add(ProfileMutationArgs),
    Edit(ProfileMutationArgs),
    List {
        #[arg(long)]
        json: bool,
    },
    Show {
        name: String,
        #[arg(long)]
        json: bool,
    },
    Remove {
        name: String,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Debug, Args)]
pub struct ProfileMutationArgs {
    pub name: String,
    #[arg(long)]
    pub github_user: Option<String>,
    #[arg(long)]
    pub git_name: Option<String>,
    #[arg(long)]
    pub git_email: Option<String>,
    #[arg(long)]
    pub hostname: Option<String>,
    #[arg(long)]
    pub ssh_key: Option<PathBuf>,
    #[arg(long = "allowed-owner")]
    pub allowed_owners: Vec<String>,
    #[arg(long)]
    pub clear_ssh_key: bool,
    #[arg(long)]
    pub clear_allowed_owners: bool,
    #[arg(long)]
    pub signing_key: Option<String>,
    #[arg(long, value_enum)]
    pub signing_format: Option<SigningFormat>,
    #[arg(long, conflicts_with = "no_require_signing")]
    pub require_signing: bool,
    #[arg(long, conflicts_with = "require_signing")]
    pub no_require_signing: bool,
    #[arg(long)]
    pub clear_signing_key: bool,
}

#[derive(Debug, Args)]
pub struct BindArgs {
    pub profile: String,
    #[arg(long, default_value = "origin")]
    pub remote: String,
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub switch: bool,
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct InspectArgs {
    #[arg(long, default_value = "origin")]
    pub remote: String,
    #[arg(long)]
    pub json: bool,
    #[arg(long, value_enum, hide = true)]
    pub hook: Option<HookMode>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum HookMode {
    PreCommit,
    PrePush,
}

#[derive(Debug, Subcommand)]
pub enum HooksCommand {
    Install,
    Status,
    Uninstall,
}
