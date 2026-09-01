# GitPersona

GitPersona is a safety-first local identity manager for developers who use personal, work, client, or organization GitHub accounts on the same computer. It binds each repository to an explicit profile and checks the Git author, GitHub CLI account, SSH key, remote host, and optional owner policy before work leaves your machine.

GitPersona delegates credentials to GitHub CLI, Git credential helpers, and OpenSSH. It never asks for, reads, or stores GitHub tokens.

## Install

Install a Rust toolchain, then build from source:

```console
cargo install --path .
gitpersona --help
```

GitPersona also expects `git`, `gh`, and `ssh` on `PATH`. Run `gitpersona doctor` to inspect the local setup.

## Quick start

Create profiles using flags or omit required flags in an interactive terminal to be prompted:

```console
gitpersona profile add personal \
  --github-user alice \
  --git-name "Alice Developer" \
  --git-email alice@example.com \
  --ssh-key ~/.ssh/id_ed25519_personal \
  --allowed-owner alice

gitpersona profile add work \
  --github-user alice-company \
  --git-name "Alice Developer" \
  --git-email alice@company.example \
  --ssh-key ~/.ssh/id_ed25519_company \
  --allowed-owner company-name
```

Bind the current repository. Binding does not switch GitHub CLI unless requested explicitly:

```console
gitpersona bind work
gitpersona bind work --switch
gitpersona status
gitpersona check
```

Clone and bind a repository in one identity-safe operation. GitPersona uses SSH
when the profile has an SSH key and HTTPS otherwise; override that choice with
`--protocol`:

```console
gitpersona clone work company-name/device-firmware
gitpersona clone personal alice/project ./project --protocol https
```

GitPersona validates the host and owner policy before cloning, switches GitHub
CLI explicitly, and restores the previous account if cloning or binding fails.

Import an existing repository identity without reading credentials, then apply a
profile automatically to every repository under a directory using native Git
`includeIf` rules:

```console
gitpersona profile import existing-work
gitpersona directory add work ~/work
gitpersona directory list
gitpersona directory sync work
gitpersona directory remove ~/work
```

Directory rules write marked profile fragments beside GitPersona's configuration
and add an exact global include. Removal deletes only that exact include and only
removes fragments carrying GitPersona's marker.

For HTTPS remotes, configure GitHub CLI as Git's credential helper:

```console
gh auth setup-git --hostname github.com
```

For SSH remotes, GitPersona writes a repository-local `core.sshCommand` using the profile key and `IdentitiesOnly=yes`.

## Safety hooks

Hooks are opt-in and GitPersona never replaces or chains an existing hook setup:

```console
gitpersona hooks install
gitpersona hooks status
gitpersona hooks uninstall
```

The pre-commit hook performs local author and policy checks. The pre-push hook performs full GitHub CLI and SSH verification and fails closed when a network-dependent identity cannot be verified.

## Configuration

Configuration is stored in the platform-native user configuration directory. Override the location with `GITPERSONA_CONFIG` for portable or test setups.

```toml
schema_version = 1

[profiles.work]
github_user = "alice-company"
git_name = "Alice Developer"
git_email = "alice@company.example"
hostname = "github.com"
ssh_key = "~/.ssh/id_ed25519_company"
allowed_owners = ["company-name"]
signing_key = "ABC123"
signing_format = "openpgp"
require_signing = true
```

Profiles can require OpenPGP or SSH commit signing. Binding snapshots and applies
`user.signingKey`, `gpg.format`, and `commit.gpgSign`; unbinding restores their
original repository-local values exactly.

Generate shell completions without modifying shell configuration:

```console
gitpersona completions bash > gitpersona.bash
gitpersona completions powershell > _gitpersona.ps1
```

Repository binding and rollback metadata live only in local Git configuration. `gitpersona unbind` restores the exact values that existed before the first bind.

## Exit codes

- `0`: success
- `1`: identity or policy check failed
- `2`: invalid input or configuration
- `3`: missing dependency or subprocess failure

## Development

```console
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution expectations.

## License

MIT
