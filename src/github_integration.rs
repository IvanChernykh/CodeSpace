use crate::model::{Error, Result};
use crate::util::json_escape;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

const GITHUB_API_HOST: &str = "api.github.com";
const GITHUB_API_PORT: u16 = 443;

#[derive(Debug, Clone, Default)]
pub struct GitHubConfig {
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
            "{{\"username\":\"{}\",\"default_owner\":\"{}\",\"default_repo\":\"{}\",\"token_set\":{}}}",
            json_escape(&self.username),
            json_escape(&self.default_owner),
            json_escape(&self.default_repo),
            !self.token.is_empty()
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
    match fs::read_to_string(&path) {
        Ok(content) => parse_config(&content),
        Err(_) => GitHubConfig::default(),
    }
}

pub fn save_config(root: &Path, config: &GitHubConfig) -> Result<()> {
    let path = config_path(root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = format!(
        "{{\"token\":\"{}\",\"username\":\"{}\",\"default_owner\":\"{}\",\"default_repo\":\"{}\"}}",
        json_escape(&config.token),
        json_escape(&config.username),
        json_escape(&config.default_owner),
        json_escape(&config.default_repo)
    );
    fs::write(&path, json)?;
    Ok(())
}

fn parse_config(content: &str) -> GitHubConfig {
    let mut config = GitHubConfig::default();
    let bytes = content.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        if bytes[idx] == b'"' {
            if let Some((key, end)) = parse_string(content, idx) {
                idx = end;
                idx = skip_ws(bytes, idx);
                if idx < bytes.len() && bytes[idx] == b':' {
                    idx = skip_ws(bytes, idx + 1);
                    match key.as_str() {
                        "token" => { if let Some((v, n)) = parse_string(content, idx) { config.token = v; idx = n; } else { idx = skip_value(bytes, idx); } }
                        "username" => { if let Some((v, n)) = parse_string(content, idx) { config.username = v; idx = n; } else { idx = skip_value(bytes, idx); } }
                        "default_owner" => { if let Some((v, n)) = parse_string(content, idx) { config.default_owner = v; idx = n; } else { idx = skip_value(bytes, idx); } }
                        "default_repo" => { if let Some((v, n)) = parse_string(content, idx) { config.default_repo = v; idx = n; } else { idx = skip_value(bytes, idx); } }
                        _ => { idx = skip_value(bytes, idx); }
                    }
                }
            } else { idx += 1; }
        } else { idx += 1; }
    }
    config
}

pub fn link(root: &Path, token: &str, username: &str) -> Result<GitHubConfig> {
    let mut config = load_config(root);
    config.token = token.to_string();
    config.username = username.to_string();
    if config.default_owner.is_empty() {
        config.default_owner = username.to_string();
    }
    save_config(root, &config)?;
    Ok(config)
}

pub fn unlink(root: &Path) -> Result<()> {
    let path = config_path(root);
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

pub fn status(root: &Path) -> GitHubConfig {
    load_config(root)
}

pub fn list_issues(root: &Path, owner: Option<&str>, repo: Option<&str>, state: &str) -> Result<String> {
    let config = load_config(root);
    if !config.is_configured() {
        return Err(Error::InvalidArgument("GitHub not linked. Run: cse github link --token <token> --username <user>".to_string()));
    }
    let owner = owner.unwrap_or(&config.default_owner);
    let repo = repo.unwrap_or(&config.default_repo);
    if owner.is_empty() || repo.is_empty() {
        return Err(Error::InvalidArgument("owner and repo must be set".to_string()));
    }
    let path = format!("/repos/{owner}/{repo}/issues?state={state}&per_page=30");
    let response = github_api_get(&config.token, &path)?;
    Ok(response)
}

pub fn list_prs(root: &Path, owner: Option<&str>, repo: Option<&str>, state: &str) -> Result<String> {
    let config = load_config(root);
    if !config.is_configured() {
        return Err(Error::InvalidArgument("GitHub not linked. Run: cse github link --token <token> --username <user>".to_string()));
    }
    let owner = owner.unwrap_or(&config.default_owner);
    let repo = repo.unwrap_or(&config.default_repo);
    let path = format!("/repos/{owner}/{repo}/pulls?state={state}&per_page=30");
    let response = github_api_get(&config.token, &path)?;
    Ok(response)
}

pub fn create_issue(root: &Path, title: &str, body: &str, owner: Option<&str>, repo: Option<&str>) -> Result<String> {
    let config = load_config(root);
    if !config.is_configured() {
        return Err(Error::InvalidArgument("GitHub not linked".to_string()));
    }
    let owner = owner.unwrap_or(&config.default_owner);
    let repo = repo.unwrap_or(&config.default_repo);
    let path = format!("/repos/{owner}/{repo}/issues");
    let body_json = format!(
        "{{\"title\":\"{}\",\"body\":\"{}\"}}",
        json_escape(title),
        json_escape(body)
    );
    let response = github_api_post(&config.token, &path, &body_json)?;
    Ok(response)
}

pub fn list_repos(root: &Path) -> Result<String> {
    let config = load_config(root);
    if !config.is_configured() {
        return Err(Error::InvalidArgument("GitHub not linked".to_string()));
    }
    let path = format!("/user/repos?per_page=50&sort=updated");
    let response = github_api_get(&config.token, &path)?;
    Ok(response)
}

pub fn set_default_repo(root: &Path, owner: &str, repo: &str) -> Result<()> {
    let mut config = load_config(root);
    config.default_owner = owner.to_string();
    config.default_repo = repo.to_string();
    save_config(root, &config)?;
    Ok(())
}

fn github_api_get(token: &str, path: &str) -> Result<String> {
    github_api_request(token, "GET", path, "")
}

fn github_api_post(token: &str, path: &str, body: &str) -> Result<String> {
    github_api_request(token, "POST", path, body)
}

fn github_api_request(token: &str, method: &str, path: &str, body: &str) -> Result<String> {
    use std::net::ToSocketAddrs;

    let host_port = format!("{GITHUB_API_HOST}:{GITHUB_API_PORT}");
    let addrs = host_port.to_socket_addrs()
        .map_err(|e| Error::InvalidArgument(format!("DNS resolution failed: {e}")))?;
    let mut stream = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, Duration::from_secs(10)) {
            Ok(s) => { stream = Some(s); break; }
            Err(_) => continue,
        }
    }
    let mut stream = stream.ok_or_else(|| Error::InvalidArgument("cannot connect to GitHub API".to_string()))?;

    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;

    let request = if body.is_empty() {
        format!(
            "{method} {path} HTTP/1.1\r\nHost: {GITHUB_API_HOST}\r\nUser-Agent: CodeSpace/2.0\r\nAccept: application/vnd.github+json\r\nAuthorization: Bearer {token}\r\nX-GitHub-Api-Version: 2022-11-28\r\nConnection: close\r\n\r\n"
        )
    } else {
        format!(
            "{method} {path} HTTP/1.1\r\nHost: {GITHUB_API_HOST}\r\nUser-Agent: CodeSpace/2.0\r\nAccept: application/vnd.github+json\r\nAuthorization: Bearer {token}\r\nX-GitHub-Api-Version: 2022-11-28\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    };

    stream.write_all(request.as_bytes())?;

    let mut response_data = Vec::new();
    stream.read_to_end(&mut response_data)?;

    let response = String::from_utf8_lossy(&response_data);
    let body_start = response
        .find("\r\n\r\n")
        .ok_or_else(|| Error::InvalidArgument("malformed GitHub API response".to_string()))?;

    let status_line = response.lines().next().unwrap_or("");
    if !status_line.contains(" 200") && !status_line.contains(" 201") {
        return Err(Error::InvalidArgument(format!(
            "GitHub API error: {status_line}"
        )));
    }

    Ok(response[body_start + 4..].to_string())
}

fn parse_string(content: &str, start: usize) -> Option<(String, usize)> {
    let bytes = content.as_bytes();
    if bytes.get(start) != Some(&b'"') { return None; }
    let mut output = String::new();
    let mut idx = start + 1;
    while idx < bytes.len() {
        match bytes[idx] {
            b'"' => return Some((output, idx + 1)),
            b'\\' => {
                idx += 1;
                match bytes.get(idx) {
                    Some(&b'"') => output.push('"'),
                    Some(&b'\\') => output.push('\\'),
                    Some(&b'n') => output.push('\n'),
                    Some(&b't') => output.push('\t'),
                    Some(&b'r') => output.push('\r'),
                    Some(&b'/') => output.push('/'),
                    Some(&b'u') => {
                        if let Some(hex) = content.get(idx + 1..idx + 5) {
                            if let Ok(val) = u16::from_str_radix(hex, 16) {
                                if let Some(ch) = char::from_u32(u32::from(val)) {
                                    output.push(ch);
                                }
                            }
                            idx += 4;
                        }
                    }
                    _ => {}
                }
            }
            byte if byte < 0x80 => output.push(char::from(byte)),
            _ => {
                if let Some(ch) = content[idx..].chars().next() {
                    output.push(ch);
                    idx += ch.len_utf8() - 1;
                }
            }
        }
        idx += 1;
    }
    None
}

fn skip_ws(bytes: &[u8], mut idx: usize) -> usize {
    while idx < bytes.len() && matches!(bytes[idx], b' ' | b'\t' | b'\n' | b'\r') { idx += 1; }
    idx
}

fn skip_value(bytes: &[u8], mut idx: usize) -> usize {
    let mut depth = 0;
    while idx < bytes.len() {
        match bytes[idx] {
            b'{' | b'[' => depth += 1,
            b'}' | b']' => { if depth == 0 { return idx; } depth -= 1; }
            b',' if depth == 0 => return idx,
            b'"' => { idx += 1; while idx < bytes.len() && bytes[idx] != b'"' { if bytes[idx] == b'\\' { idx += 1; } idx += 1; } }
            _ => {}
        }
        idx += 1;
    }
    idx
}
