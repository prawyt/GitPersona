use crate::error::GitPersonaError;
use regex::Regex;
use serde::Serialize;
use url::Url;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RemoteProtocol {
    Ssh,
    Https,
    Http,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RemoteInfo {
    pub url: String,
    pub protocol: RemoteProtocol,
    pub hostname: String,
    pub owner: String,
    pub repository: String,
}

pub fn parse_remote(input: &str) -> Result<RemoteInfo, GitPersonaError> {
    let input = input.trim();
    if input.contains("://") {
        let url = Url::parse(input)
            .map_err(|e| GitPersonaError::usage(format!("invalid remote URL: {e}")))?;
        let protocol = match url.scheme() {
            "ssh" => RemoteProtocol::Ssh,
            "https" => RemoteProtocol::Https,
            "http" => RemoteProtocol::Http,
            other => {
                return Err(GitPersonaError::usage(format!(
                    "unsupported remote protocol: {other}"
                )));
            }
        };
        let hostname = url
            .host_str()
            .ok_or_else(|| GitPersonaError::usage("remote URL has no hostname"))?
            .to_string();
        return build(input, protocol, hostname, url.path());
    }
    let scp = Regex::new(r"^(?:[^@]+@)?(?P<host>[^:]+):(?P<path>.+)$").expect("valid regex");
    if let Some(captures) = scp.captures(input) {
        return build(
            input,
            RemoteProtocol::Ssh,
            captures["host"].to_string(),
            &captures["path"],
        );
    }
    Err(GitPersonaError::usage(
        "unsupported remote URL; expected SSH, HTTPS, or HTTP",
    ))
}

fn build(
    original: &str,
    protocol: RemoteProtocol,
    hostname: String,
    path: &str,
) -> Result<RemoteInfo, GitPersonaError> {
    let cleaned = path
        .trim_matches('/')
        .strip_suffix(".git")
        .unwrap_or(path.trim_matches('/'));
    let mut parts = cleaned.split('/').filter(|part| !part.is_empty());
    let owner = parts
        .next()
        .ok_or_else(|| GitPersonaError::usage("remote URL has no owner"))?;
    let repository = parts
        .next()
        .ok_or_else(|| GitPersonaError::usage("remote URL has no repository"))?;
    Ok(RemoteInfo {
        url: original.to_string(),
        protocol,
        hostname,
        owner: owner.to_string(),
        repository: repository.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_remotes() {
        for value in [
            "git@github.com:Org/repo.git",
            "ssh://git@github.example/Org/repo.git",
            "https://github.com/Org/repo.git",
        ] {
            let remote = parse_remote(value).unwrap();
            assert_eq!(remote.owner, "Org");
            assert_eq!(remote.repository, "repo");
        }
    }
}
