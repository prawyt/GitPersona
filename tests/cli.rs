use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use std::{fs, process::Command};

#[test]
fn help_lists_primary_commands() {
    cargo_bin_cmd!()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("profile"))
        .stdout(predicate::str::contains("clone"))
        .stdout(predicate::str::contains("bind"))
        .stdout(predicate::str::contains("hooks"));
}

#[test]
fn profile_add_and_list_json() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config.toml");
    cargo_bin_cmd!()
        .env("GITPERSONA_CONFIG", &config)
        .args([
            "profile",
            "add",
            "work",
            "--github-user",
            "alice-work",
            "--git-name",
            "Alice",
            "--git-email",
            "alice@example.com",
        ])
        .assert()
        .success();

    cargo_bin_cmd!()
        .env("GITPERSONA_CONFIG", &config)
        .args(["profile", "list", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("alice-work"));
}

#[test]
fn profile_signing_and_completions_are_exposed() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config.toml");
    cargo_bin_cmd!()
        .env("GITPERSONA_CONFIG", &config)
        .args([
            "profile",
            "add",
            "signed",
            "--github-user",
            "alice",
            "--git-name",
            "Alice",
            "--git-email",
            "alice@example.com",
            "--signing-key",
            "ABC123",
            "--require-signing",
        ])
        .assert()
        .success();
    cargo_bin_cmd!()
        .env("GITPERSONA_CONFIG", &config)
        .args(["profile", "show", "signed", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"require_signing\": true"));
    cargo_bin_cmd!()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("gitpersona"));
}

#[test]
fn bind_and_unbind_restore_original_identity() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).unwrap();
    assert!(
        Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&repo)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new("git")
            .args(["config", "--local", "user.name", "Original"])
            .current_dir(&repo)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new("git")
            .args(["config", "--local", "user.email", "original@example.com"])
            .current_dir(&repo)
            .status()
            .unwrap()
            .success()
    );
    let config = temp.path().join("config.toml");

    cargo_bin_cmd!()
        .current_dir(&repo)
        .env("GITPERSONA_CONFIG", &config)
        .args([
            "profile",
            "add",
            "work",
            "--github-user",
            "alice-work",
            "--git-name",
            "Alice Work",
            "--git-email",
            "work@example.com",
        ])
        .assert()
        .success();
    cargo_bin_cmd!()
        .current_dir(&repo)
        .env("GITPERSONA_CONFIG", &config)
        .args(["bind", "work"])
        .assert()
        .success();
    assert_eq!(git_value(&repo, "user.email"), "work@example.com");
    assert_eq!(git_value(&repo, "gitpersona.profile"), "work");

    cargo_bin_cmd!()
        .current_dir(&repo)
        .env("GITPERSONA_CONFIG", &config)
        .arg("unbind")
        .assert()
        .success();
    assert_eq!(git_value(&repo, "user.name"), "Original");
    assert_eq!(git_value(&repo, "user.email"), "original@example.com");
    assert!(
        !Command::new("git")
            .args(["config", "--local", "--get", "gitpersona.profile"])
            .current_dir(&repo)
            .status()
            .unwrap()
            .success()
    );
}

#[test]
fn bind_and_unbind_restore_signing_configuration() {
    let temp = tempfile::tempdir().unwrap();
    let repo = initialized_repo(temp.path());
    let config = temp.path().join("config.toml");
    for (key, value) in [
        ("user.signingKey", "OLDKEY"),
        ("gpg.format", "ssh"),
        ("commit.gpgSign", "false"),
    ] {
        assert!(
            Command::new("git")
                .args(["config", "--local", key, value])
                .current_dir(&repo)
                .status()
                .unwrap()
                .success()
        );
    }
    cargo_bin_cmd!()
        .current_dir(&repo)
        .env("GITPERSONA_CONFIG", &config)
        .args([
            "profile",
            "add",
            "signed",
            "--github-user",
            "alice",
            "--git-name",
            "Alice",
            "--git-email",
            "alice@example.com",
            "--signing-key",
            "NEWKEY",
            "--require-signing",
        ])
        .assert()
        .success();
    cargo_bin_cmd!()
        .current_dir(&repo)
        .env("GITPERSONA_CONFIG", &config)
        .args(["bind", "signed"])
        .assert()
        .success();
    assert_eq!(git_value(&repo, "user.signingKey"), "NEWKEY");
    assert_eq!(git_value(&repo, "gpg.format"), "openpgp");
    assert_eq!(git_value(&repo, "commit.gpgSign"), "true");

    cargo_bin_cmd!()
        .current_dir(&repo)
        .env("GITPERSONA_CONFIG", &config)
        .arg("unbind")
        .assert()
        .success();
    assert_eq!(git_value(&repo, "user.signingKey"), "OLDKEY");
    assert_eq!(git_value(&repo, "gpg.format"), "ssh");
    assert_eq!(git_value(&repo, "commit.gpgSign"), "false");
}

#[test]
fn check_json_reports_drift_and_exits_one() {
    let temp = tempfile::tempdir().unwrap();
    let repo = initialized_repo(temp.path());
    let config = temp.path().join("config.toml");
    add_profile(&repo, &config);
    cargo_bin_cmd!()
        .current_dir(&repo)
        .env("GITPERSONA_CONFIG", &config)
        .args(["bind", "work"])
        .assert()
        .success();
    assert!(
        Command::new("git")
            .args(["config", "--local", "user.email", "drift@example.com"])
            .current_dir(&repo)
            .status()
            .unwrap()
            .success()
    );

    cargo_bin_cmd!()
        .current_dir(&repo)
        .env("GITPERSONA_CONFIG", &config)
        .args(["check", "--json"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("\"overall\": \"failure\""))
        .stdout(predicate::str::contains("\"id\": \"git_email\""));
}

#[test]
fn drift_requires_force_to_rebind() {
    let temp = tempfile::tempdir().unwrap();
    let repo = initialized_repo(temp.path());
    let config = temp.path().join("config.toml");
    add_profile(&repo, &config);
    cargo_bin_cmd!()
        .current_dir(&repo)
        .env("GITPERSONA_CONFIG", &config)
        .args(["bind", "work"])
        .assert()
        .success();
    Command::new("git")
        .args(["config", "--local", "user.name", "Manual change"])
        .current_dir(&repo)
        .status()
        .unwrap();

    cargo_bin_cmd!()
        .current_dir(&repo)
        .env("GITPERSONA_CONFIG", &config)
        .args(["bind", "work"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--force"));
    cargo_bin_cmd!()
        .current_dir(&repo)
        .env("GITPERSONA_CONFIG", &config)
        .args(["bind", "work", "--force"])
        .assert()
        .success();
    assert_eq!(git_value(&repo, "user.name"), "Alice Work");
}

#[test]
fn hooks_install_and_uninstall_without_touching_other_hooks() {
    let temp = tempfile::tempdir().unwrap();
    let repo = initialized_repo(temp.path());
    let config = temp.path().join("config.toml");

    cargo_bin_cmd!()
        .current_dir(&repo)
        .env("GITPERSONA_CONFIG", &config)
        .args(["hooks", "install"])
        .assert()
        .success();
    let hooks = repo.join(".git").join("hooks");
    assert!(
        fs::read_to_string(hooks.join("pre-commit"))
            .unwrap()
            .contains("Managed by GitPersona")
    );
    cargo_bin_cmd!()
        .current_dir(&repo)
        .env("GITPERSONA_CONFIG", &config)
        .args(["hooks", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("installed"));
    cargo_bin_cmd!()
        .current_dir(&repo)
        .env("GITPERSONA_CONFIG", &config)
        .args(["hooks", "uninstall"])
        .assert()
        .success();
    assert!(!hooks.join("pre-commit").exists());

    fs::write(hooks.join("pre-commit"), "#!/bin/sh\necho custom\n").unwrap();
    cargo_bin_cmd!()
        .current_dir(&repo)
        .env("GITPERSONA_CONFIG", &config)
        .args(["hooks", "install"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("refusing to replace"));
    assert!(
        fs::read_to_string(hooks.join("pre-commit"))
            .unwrap()
            .contains("custom")
    );
}

#[test]
fn hooks_refuse_custom_hooks_path() {
    let temp = tempfile::tempdir().unwrap();
    let repo = initialized_repo(temp.path());
    let config = temp.path().join("config.toml");
    Command::new("git")
        .args(["config", "--local", "core.hooksPath", ".custom-hooks"])
        .current_dir(&repo)
        .status()
        .unwrap();
    cargo_bin_cmd!()
        .current_dir(&repo)
        .env("GITPERSONA_CONFIG", &config)
        .args(["hooks", "install"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("core.hooksPath"));
}

#[test]
fn invalid_profile_and_missing_key_exit_two() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config.toml");
    cargo_bin_cmd!()
        .env("GITPERSONA_CONFIG", &config)
        .args([
            "profile",
            "add",
            "bad name",
            "--github-user",
            "alice",
            "--git-name",
            "Alice",
            "--git-email",
            "a@example.com",
        ])
        .assert()
        .code(2);
    cargo_bin_cmd!()
        .env("GITPERSONA_CONFIG", &config)
        .args([
            "profile",
            "add",
            "work",
            "--github-user",
            "alice",
            "--git-name",
            "Alice",
            "--git-email",
            "a@example.com",
            "--ssh-key",
            "does-not-exist",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("SSH key does not exist"));
}

#[test]
fn deleted_key_does_not_make_config_unreadable() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config.toml");
    let key = temp.path().join("key");
    fs::write(&key, "test key").unwrap();
    cargo_bin_cmd!()
        .env("GITPERSONA_CONFIG", &config)
        .args([
            "profile",
            "add",
            "work",
            "--github-user",
            "alice",
            "--git-name",
            "Alice",
            "--git-email",
            "a@example.com",
            "--ssh-key",
            key.to_str().unwrap(),
        ])
        .assert()
        .success();
    fs::remove_file(&key).unwrap();

    cargo_bin_cmd!()
        .env("GITPERSONA_CONFIG", &config)
        .args(["profile", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("work"));
    cargo_bin_cmd!()
        .env("GITPERSONA_CONFIG", &config)
        .arg("doctor")
        .assert()
        .code(3)
        .stdout(predicate::str::contains("profile work: unavailable"));
}

fn initialized_repo(parent: &std::path::Path) -> std::path::PathBuf {
    let repo = parent.join("repo");
    fs::create_dir(&repo).unwrap();
    assert!(
        Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&repo)
            .status()
            .unwrap()
            .success()
    );
    repo
}

fn add_profile(repo: &std::path::Path, config: &std::path::Path) {
    cargo_bin_cmd!()
        .current_dir(repo)
        .env("GITPERSONA_CONFIG", config)
        .args([
            "profile",
            "add",
            "work",
            "--github-user",
            "alice-work",
            "--git-name",
            "Alice Work",
            "--git-email",
            "work@example.com",
        ])
        .assert()
        .success();
}

fn git_value(repo: &std::path::Path, key: &str) -> String {
    let output = Command::new("git")
        .args(["config", "--local", "--get", key])
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}
