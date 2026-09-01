use crate::{
    error::GitPersonaError,
    process::{Runner, os_args},
};
use serde_json::Value;
use std::{ffi::OsString, time::Duration};

const TIMEOUT: Duration = Duration::from_secs(20);

pub struct GitHub<'a> {
    runner: &'a dyn Runner,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Account {
    pub login: String,
    pub active: bool,
    pub valid: bool,
}

impl<'a> GitHub<'a> {
    pub fn new(runner: &'a dyn Runner) -> Self {
        Self { runner }
    }

    pub fn accounts(&self, hostname: &str) -> Result<Vec<Account>, GitPersonaError> {
        let args = ["auth", "status", "--hostname", hostname, "--json", "hosts"];
        let output = self.runner.run("gh", &os_args(&args), TIMEOUT)?;
        if output.code.is_none() {
            return Err(GitPersonaError::dependency("GitHub CLI status timed out"));
        }
        let value: Value = serde_json::from_str(&output.stdout).map_err(|e| {
            GitPersonaError::dependency(format!("GitHub CLI returned invalid JSON: {e}"))
        })?;
        let mut accounts = Vec::new();
        collect_accounts(&value, &mut accounts);
        accounts.sort();
        accounts.dedup();
        Ok(accounts)
    }

    pub fn active_account(&self, hostname: &str) -> Result<Option<String>, GitPersonaError> {
        let active = self
            .accounts(hostname)?
            .into_iter()
            .find(|account| account.active);
        match active {
            Some(account) if account.valid => Ok(Some(account.login)),
            Some(account) => Err(GitPersonaError::dependency(format!(
                "GitHub CLI could not validate the active account '{}' on {hostname}",
                account.login
            ))),
            None => Ok(None),
        }
    }

    pub fn is_authenticated(&self, hostname: &str, user: &str) -> Result<bool, GitPersonaError> {
        Ok(self
            .accounts(hostname)?
            .iter()
            .any(|account| account.valid && account.login.eq_ignore_ascii_case(user)))
    }

    pub fn switch(&self, hostname: &str, user: &str) -> Result<(), GitPersonaError> {
        if !self.is_authenticated(hostname, user)? {
            return Err(GitPersonaError::usage(format!(
                "GitHub CLI is not authenticated as {user} on {hostname}"
            )));
        }
        let args = vec![
            OsString::from("auth"),
            OsString::from("switch"),
            OsString::from("--hostname"),
            OsString::from(hostname),
            OsString::from("--user"),
            OsString::from(user),
        ];
        let output = self.runner.run("gh", &args, TIMEOUT)?;
        if !output.success() {
            return Err(GitPersonaError::dependency(format!(
                "GitHub CLI account switch failed: {}",
                output.stderr.trim()
            )));
        }
        let active = self.active_account(hostname)?;
        if active
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case(user))
        {
            Ok(())
        } else {
            Err(GitPersonaError::dependency(format!(
                "GitHub CLI did not activate {user} after switching"
            )))
        }
    }
}

fn collect_accounts(value: &Value, output: &mut Vec<Account>) {
    match value {
        Value::Object(map) => {
            if let Some(login) = map.get("login").and_then(Value::as_str) {
                let active = map.get("active").and_then(Value::as_bool).unwrap_or(false);
                let valid = map
                    .get("state")
                    .and_then(Value::as_str)
                    .is_none_or(|state| state.eq_ignore_ascii_case("success"));
                output.push(Account {
                    login: login.to_string(),
                    active,
                    valid,
                });
            }
            for child in map.values() {
                collect_accounts(child, output);
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_accounts(child, output);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::ProcessOutput;

    struct FakeRunner {
        output: ProcessOutput,
    }

    impl Runner for FakeRunner {
        fn run(
            &self,
            _: &str,
            _: &[OsString],
            _: Duration,
        ) -> Result<ProcessOutput, GitPersonaError> {
            Ok(self.output.clone())
        }
    }

    #[test]
    fn extracts_accounts_from_gh_shape() {
        let value: Value = serde_json::json!({"hosts":{"github.com":[{"login":"alice","active":true,"state":"success"},{"login":"bob","active":false,"state":"error"}]}});
        let mut accounts = vec![];
        collect_accounts(&value, &mut accounts);
        assert_eq!(
            accounts,
            vec![
                Account {
                    login: "alice".into(),
                    active: true,
                    valid: true,
                },
                Account {
                    login: "bob".into(),
                    active: false,
                    valid: false,
                }
            ]
        );
    }

    #[test]
    fn rejects_malformed_and_offline_status() {
        let malformed = FakeRunner {
            output: ProcessOutput {
                code: Some(0),
                stdout: "not-json".into(),
                stderr: String::new(),
            },
        };
        assert!(
            GitHub::new(&malformed)
                .active_account("github.com")
                .is_err()
        );

        let offline = FakeRunner {
            output: ProcessOutput {
                code: Some(0),
                stdout: serde_json::json!({"hosts":{"github.com":[{"login":"alice","active":true,"state":"error"}]}}).to_string(),
                stderr: String::new(),
            },
        };
        assert!(GitHub::new(&offline).active_account("github.com").is_err());
    }
}
