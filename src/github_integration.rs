use crate::model::{Error, Result};
use crate::util::json_escape;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

const GITHUB_HOST: &str = "github.com";
const AUTH_MARKER: &str = "gh-cli";

#[derive(Debug, Clone, Default)]
pub struct GitHubConfig {
    /// Compatibility field. It is only an in-memory authentication marker and
    /// is never persisted. GitHub credentials are owned by the `gh` CLI.
    pub token: String,
    pub username: String,
    pub default_owner: String,
    pub default_repo: String,
}

impl GitHubConfig {
    pub fn is_configured(&self) -> bool {
        !self.token.is_empty()
    }

    pub fn to_json(&self) -> String {
        format!(
            "{{\"username\":\"{}\",\"default_owner\":\"{}\",\"default_repo\":\"{}\",\"token_set\":{},\"credential_provider\":\"gh-cli\"}}",
            json_escape(&self.username),
            json_escape(&self.default_owner),
            json_escape(&self.default_repo),
            self.is_configured()
        )
    }
}

pub fn config_path(root: &Path) -> std::path::PathBuf {
    root.join(".codespace").join("github.json")
}

pub fn load_config(root: &Path) -> GitHubConfig {
    let path = config_path(root);
    if !path.exists() {
        return GitHubConfig::default();
    }
    let Ok(content) = fs::read_to_string(&path) else {
        return GitHubConfig::default();
    };
    let mut config = parse_config(&content);
    if !config.token.is_empty() {
        // Migrate legacy versions that stored a PAT in plaintext. The old
        // credential is intentionally discarded and must be linked again via
        // `gh auth login` so it can live in the platform credential store.
        config.token.clear();
        let _ = save_config(root, &config);
    }
    config
}

pub fn save_config(root: &Path, config: &GitHubConfig) -> Result<()> {
    let path = config_path(root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = format!(
        "{{\"username\":\"{}\",\"default_owner\":\"{}\",\"default_repo\":\"{}\",\"credential_provider\":\"gh-cli\"}}",
        json_escape(&config.username),
        json_escape(&config.default_owner),
        json_escape(&config.default_repo)
    );
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, json)?;
    if path.exists() {
        let _ = fs::remove_file(&path);
    }
    fs::rename(temp, path)?;
    Ok(())
}

fn parse_config(content: &str) -> GitHubConfig {
    GitHubConfig {
        token: json_string(content, "token").unwrap_or_default(),
        username: json_string(content, "username").unwrap_or_default(),
        default_owner: json_string(content, "default_owner").unwrap_or_default(),
        default_repo: json_string(content, "default_repo").unwrap_or_default(),
    }
}

pub fn link(root: &Path, token: &str, username: &str) -> Result<GitHubConfig> {
    if token.trim().is_empty() {
        return Err(Error::InvalidArgument(
            "GitHub token is required for `gh auth login --with-token`".to_string(),
        ));
    }
    let input = format!("{}\n", token.trim());
    run_gh(
        &[
            "auth",
            "login",
            "--hostname",
            GITHUB_HOST,
            "--git-protocol",
            "https",
            "--with-token",
        ],
        Some(&input),
    )?;

    let resolved_username = if username.trim().is_empty() {
        current_username()?
    } else {
        username.trim().to_string()
    };
    let mut config = load_config(root);
    config.token = AUTH_MARKER.to_string();
    config.username = resolved_username.clone();
    if config.default_owner.is_empty() {
        config.default_owner = resolved_username;
    }
    save_config(root, &config)?;
    Ok(config)
}

pub fn unlink(root: &Path) -> Result<()> {
    // Do not revoke a machine-wide gh session implicitly. CodeSpace only
    // removes its repository metadata; `gh auth logout` remains an explicit
    // user operation.
    let path = config_path(root);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn status(root: &Path) -> GitHubConfig {
    let mut config = load_config(root);
    if gh_authenticated() {
        config.token = AUTH_MARKER.to_string();
        if config.username.is_empty() {
            config.username = current_username().unwrap_or_default();
        }
    }
    config
}

pub fn list_issues(
    root: &Path,
    owner: Option<&str>,
    repo: Option<&str>,
    state: &str,
) -> Result<String> {
    let config = authenticated_config(root)?;
    let owner = owner.unwrap_or(&config.default_owner);
    let repo = repo.unwrap_or(&config.default_repo);
    validate_repository(owner, repo)?;
    github_api_request(
        "GET",
        &format!("repos/{owner}/{repo}/issues?state={state}&per_page=30"),
        "",
    )
}

pub fn list_prs(
    root: &Path,
    owner: Option<&str>,
    repo: Option<&str>,
    state: &str,
) -> Result<String> {
    let config = authenticated_config(root)?;
    let owner = owner.unwrap_or(&config.default_owner);
    let repo = repo.unwrap_or(&config.default_repo);
    validate_repository(owner, repo)?;
    github_api_request(
        "GET",
        &format!("repos/{owner}/{repo}/pulls?state={state}&per_page=30"),
        "",
    )
}

pub fn create_issue(
    root: &Path,
    title: &str,
    body: &str,
    owner: Option<&str>,
    repo: Option<&str>,
) -> Result<String> {
    let config = authenticated_config(root)?;
    let owner = owner.unwrap_or(&config.default_owner);
    let repo = repo.unwrap_or(&config.default_repo);
    validate_repository(owner, repo)?;
    let payload = format!(
        "{{\"title\":\"{}\",\"body\":\"{}\"}}",
        json_escape(title),
        json_escape(body)
    );
    github_api_request("POST", &format!("repos/{owner}/{repo}/issues"), &payload)
}

pub fn list_repos(root: &Path) -> Result<String> {
    let _ = authenticated_config(root)?;
    github_api_request("GET", "user/repos?per_page=50&sort=updated", "")
}

pub fn set_default_repo(root: &Path, owner: &str, repo: &str) -> Result<()> {
    validate_repository(owner, repo)?;
    let mut config = load_config(root);
    config.default_owner = owner.to_string();
    config.default_repo = repo.to_string();
    save_config(root, &config)
}

fn authenticated_config(root: &Path) -> Result<GitHubConfig> {
    let config = status(root);
    if config.is_configured() {
        Ok(config)
    } else {
        Err(Error::InvalidArgument(
            "GitHub is not authenticated. Install GitHub CLI and run `cse github link --token <token> --username <user>` or `gh auth login`.".to_string(),
        ))
    }
}

fn validate_repository(owner: &str, repo: &str) -> Result<()> {
    if owner.trim().is_empty() || repo.trim().is_empty() {
        return Err(Error::InvalidArgument(
            "GitHub owner and repository must be configured".to_string(),
        ));
    }
    Ok(())
}

fn current_username() -> Result<String> {
    let username = run_gh(&["api", "user", "--jq", ".login"], None)?;
    let username = username.trim();
    if username.is_empty() {
        Err(Error::Protocol(
            "GitHub CLI returned an empty authenticated username".to_string(),
        ))
    } else {
        Ok(username.to_string())
    }
}

fn gh_authenticated() -> bool {
    Command::new("gh")
        .args(["auth", "status", "--hostname", GITHUB_HOST])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn github_api_request(method: &str, path: &str, body: &str) -> Result<String> {
    let path = path.trim_start_matches('/');
    if body.is_empty() {
        run_gh(&["api", "--method", method, path], None)
    } else {
        run_gh(
            &["api", "--method", method, path, "--input", "-"],
            Some(body),
        )
    }
}

fn run_gh(args: &[&str], input: Option<&str>) -> Result<String> {
    let mut command = Command::new("gh");
    command.args(args);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    if input.is_some() {
        command.stdin(Stdio::piped());
    } else {
        command.stdin(Stdio::null());
    }

    let mut child = command.spawn().map_err(|error| {
        Error::InvalidArgument(format!(
            "GitHub CLI (`gh`) is required but could not be started: {error}"
        ))
    })?;
    if let Some(input) = input {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| Error::Protocol("failed to open stdin for GitHub CLI".to_string()))?;
        stdin.write_all(input.as_bytes())?;
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(Error::Protocol(if message.is_empty() {
            format!("GitHub CLI command failed with status {}", output.status)
        } else {
            format!("GitHub CLI command failed: {message}")
        }));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn json_string(input: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let position = input.find(&needle)?;
    let rest = &input[position + needle.len()..];
    let colon = rest.find(':')?;
    parse_json_string(rest[colon + 1..].trim_start())
}

fn parse_json_string(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    if bytes.first() != Some(&b'\"') {
        return None;
    }
    let mut output = String::new();
    let mut index = 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\"' => return Some(output),
            b'\\' => {
                index += 1;
                match bytes.get(index) {
                    Some(b'\"') => output.push('\"'),
                    Some(b'\\') => output.push('\\'),
                    Some(b'n') => output.push('\n'),
                    Some(b'r') => output.push('\r'),
                    Some(b't') => output.push('\t'),
                    Some(other) => output.push(char::from(*other)),
                    None => return None,
                }
            }
            byte if byte.is_ascii() => output.push(char::from(byte)),
            _ => {
                let character = input[index..].chars().next()?;
                output.push(character);
                index += character.len_utf8().saturating_sub(1);
            }
        }
        index += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_config_never_contains_token() {
        let config = GitHubConfig {
            token: "secret".to_string(),
            username: "ivan".to_string(),
            default_owner: "IvanChernykh".to_string(),
            default_repo: "CodeSpace".to_string(),
        };
        let rendered = format!(
            "{{\"username\":\"{}\",\"default_owner\":\"{}\",\"default_repo\":\"{}\"}}",
            json_escape(&config.username),
            json_escape(&config.default_owner),
            json_escape(&config.default_repo)
        );
        assert!(!rendered.contains("secret"));
    }

    #[test]
    fn parses_legacy_token_for_migration() {
        let config = parse_config(
            "{\"token\":\"legacy\",\"username\":\"ivan\",\"default_owner\":\"o\",\"default_repo\":\"r\"}",
        );
        assert_eq!(config.token, "legacy");
        assert_eq!(config.username, "ivan");
    }
}
