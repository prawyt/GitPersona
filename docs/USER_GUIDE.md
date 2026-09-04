# GitPersona user guide

This guide explains how to use the GitPersona desktop application safely. It
is intended for individual developers who use more than one Git or GitHub
identity on the same computer.

GitPersona stores identity settings and key **paths**, not passwords, private
key contents, GitHub tokens, or other credentials. Credentials remain managed
by OpenSSH, Git, Git credential helpers, and GitHub CLI.

## Before you begin

Install these programs and ensure they are available on `PATH`:

- Git
- OpenSSH (`ssh` and `ssh-keygen`)
- GitHub CLI (`gh`) when GitHub account checks or switching are needed

Open **Diagnostics** in GitPersona to confirm that each dependency is
available.

## First-time setup

1. Open GitPersona.
2. Select an existing Git repository when prompted.
3. Review the imported Git author, email, remote owner, hostname, and signing
   settings.
4. Enter a profile name such as `personal` or `work`.
5. Create the profile and bind the repository.
6. Optionally approve the repository's parent folder as a scan root.

Binding changes repository-local identity settings. It does not implicitly
switch the active GitHub CLI account.

## Configure SSH authentication

SSH authentication uses a key pair:

- The **private key** remains on your computer. A typical filename is
  `id_ed25519`.
- The matching **public key** ends in `.pub`, such as `id_ed25519.pub`. Add
  this public key to the corresponding GitHub account.

Never upload, paste, or share the private key.

### 1. Check for an existing key

On Windows PowerShell:

```powershell
Get-ChildItem $env:USERPROFILE\.ssh
```

On macOS or Linux:

```console
ls -al ~/.ssh
```

Look for a private/public pair such as `id_ed25519` and `id_ed25519.pub`.

### 2. Generate a key when needed

Replace the example email with an email associated with the intended GitHub
account:

```console
ssh-keygen -t ed25519 -C "you@example.com"
```

When multiple GitHub identities share one computer, give each key a distinct
filename, for example `id_ed25519_personal` and `id_ed25519_work`. Use a secure
passphrase unless your environment has a different security requirement.

See GitHub's official guide to
[generating a new SSH key and adding it to the SSH agent](https://docs.github.com/en/authentication/connecting-to-github-with-ssh/generating-a-new-ssh-key-and-adding-it-to-the-ssh-agent).

### 3. Add the public key to GitHub

In GitHub, open **Settings → SSH and GPG keys → New SSH key**, choose
**Authentication Key**, and paste the contents of the `.pub` file. GitHub's
[adding an SSH key](https://docs.github.com/en/authentication/connecting-to-github-with-ssh/adding-a-new-ssh-key-to-your-github-account)
guide includes both browser and GitHub CLI instructions.

### 4. Select the private key in GitPersona

1. Open **Profiles**.
2. Select the profile and click **Edit**.
3. Beside **SSH key path**, click **Browse**.
4. Select the private key file—the file without `.pub`.
5. Click **Save profile**.

GitPersona records only the selected path. It does not copy or read the key
contents and does not edit `~/.ssh/config`.

### 5. Test authentication

1. Open **SSH & Signing**.
2. Select the profile.
3. Click **Test authentication**.
4. Confirm that the reported GitHub account matches the expected account.

The test is manual and makes one SSH connection. GitHub's SSH service normally
reports successful authentication while returning a non-zero shell exit code;
GitPersona recognizes GitHub's authenticated account message.

If the test reports a different account, verify that the profile points to the
correct private key and that its public key was added to the intended GitHub
account.

## Configure commit signing

SSH authentication and commit signing are related but separate settings. A key
that authenticates Git operations does not automatically enable signed
commits.

### SSH signing

SSH signing requires Git 2.34 or later.

1. Add the public key to GitHub as a **Signing Key**. If the same key is used
   for authentication and signing, GitHub requires it to be added for both
   purposes.
2. In GitPersona, open **Profiles**, select the profile, and click **Edit**.
3. Set **Signing format** to **SSH**.
4. Set **Signing key** to the public key path, such as
   `~/.ssh/id_ed25519_work.pub`.
5. Enable **Require signed commits for bound repositories** when every commit
   in those repositories should be signed.
6. Save the profile and bind or rebind the repository.

GitPersona applies `user.signingKey`, `gpg.format`, and `commit.gpgSign` to the
bound repository. Unbinding restores the repository-local values that existed
before binding.

See GitHub's guides to
[telling Git about an SSH signing key](https://docs.github.com/en/authentication/managing-commit-signature-verification/telling-git-about-your-signing-key)
and [commit signature verification](https://docs.github.com/en/authentication/managing-commit-signature-verification/about-commit-signature-verification).

### OpenPGP signing

1. Install GPG and create or import a private GPG key.
2. Add the public GPG key to the matching GitHub account.
3. In GitPersona, set **Signing format** to **OpenPGP**.
4. Enter the long GPG key ID in **Signing key**.
5. Enable required signing if desired, save the profile, and bind or rebind the
   repository.

GitPersona does not store the private GPG key or its passphrase.

## Discover and bind repositories

1. Open **Repositories**.
2. Click **Add root** and select a folder containing repositories.
3. Click **Scan roots**. GitPersona scans approved roots only, does not follow
   symlinks, and does not run network checks during scanning.
4. Select a repository and a profile.
5. Preview the expected changes, then bind.

GitHub CLI account switching remains a separate, explicit action. The app shows
the current and target accounts before switching.

## Use the Status page

The **Status** page compares the selected repository with its bound profile.
Local checks cover the Git author, email, remote, owner policy, SSH command,
and signing configuration.

1. Add and scan a repository under **Repositories**.
2. Select the repository.
3. Open **Status**.
4. Review the expected and actual values.
5. Click **Refresh network checks** only when you want GitPersona to contact
   GitHub or test SSH.

If no repository is selected, Status explains how to add and scan one rather
than showing an empty page.

## Unbind a repository

Select the repository under **Repositories**, choose **Unbind**, review the
confirmation, and confirm the action. GitPersona restores the exact
repository-local values captured before the first bind.

## Troubleshooting

### SSH key cannot be saved

- Select the private key file, not the `.pub` file, for **SSH key path**.
- Confirm that the file still exists and is readable by your user account.
- For SSH signing, use the public `.pub` file in **Signing key**.

### SSH authentication is unavailable

- Confirm OpenSSH appears as available under **Diagnostics**.
- Configure **SSH key path** in the selected profile.
- Add the matching public key to the expected GitHub account.
- Check that the profile hostname is correct. Custom SSH host aliases must
  already exist in your SSH configuration; GitPersona does not create them.

### SSH authenticates as the wrong account

- Confirm the selected private key belongs to the intended GitHub account.
- Check which account contains the matching public key.
- Avoid sharing one authentication key across identities when account
  separation is required.

### Commits are not signed

- Confirm the signing format matches the configured key.
- For SSH signing, use the public key path and Git 2.34 or later.
- Confirm **Require signed commits** is enabled and the repository has been
  bound or rebound after changing the profile.
- Inspect the repository under **Status** for signing drift.

### GitHub does not show “Verified”

- Add the public key to GitHub as a signing key, not only as an authentication
  key.
- Ensure the commit email belongs to and is verified by the expected GitHub
  account.
- Inspect a new commit; changing settings cannot retroactively sign an unsigned
  commit.

### Status is empty

- Install a current build of GitPersona v0.6 containing the Status data-contract
  fix.
- Add and scan a repository, then select it before opening Status.
- If inspection fails, read the persistent error message and run Diagnostics.

## Safety boundaries

GitPersona does not:

- store GitHub tokens, passwords, private key contents, or passphrases;
- automatically switch GitHub CLI accounts during ordinary binding;
- edit `~/.ssh/config`;
- scan outside folders explicitly approved by the user;
- follow symlinks while scanning; or
- run network checks without an explicit user action.
