use crate::model::{Error, Result};
use crate::util::{json_escape, now_unix_ms, stable_id};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpServerStatus {
    Stopped,
    Starting,
    Running,
    Error,
}

impl McpServerStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExternalMcpServer {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub status: McpServerStatus,
    pub auto_start: bool,
    pub registered_unix_ms: u128,
    pub last_started_unix_ms: u128,
    pub last_error: String,
}

#[derive(Debug)]
pub struct McpManager {
    pub servers: BTreeMap<String, ExternalMcpServer>,
    pub processes: BTreeMap<String, Child>,
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            servers: BTreeMap::new(),
            processes: BTreeMap::new(),
        }
    }

    pub fn register(
        &mut self,
        name: &str,
        command: &str,
        args: Vec<String>,
        env: BTreeMap<String, String>,
        auto_start: bool,
    ) -> Result<&ExternalMcpServer> {
        if name.trim().is_empty() || command.trim().is_empty() {
            return Err(Error::InvalidArgument(
                "MCP server name and command are required".to_string(),
            ));
        }
        let id = stable_id(&["mcp", name, command]).to_string();
        if self.servers.contains_key(&id) {
            return Err(Error::InvalidArgument(format!(
                "MCP server `{name}` is already registered"
            )));
        }
        let server = ExternalMcpServer {
            id: id.clone(),
            name: name.to_string(),
            command: command.to_string(),
            args,
            env,
            status: McpServerStatus::Stopped,
            auto_start,
            registered_unix_ms: now_unix_ms(),
            last_started_unix_ms: 0,
            last_error: String::new(),
        };
        self.servers.insert(id.clone(), server);
        if auto_start {
            let _ = self.start(&id);
        }
        self.servers
            .get(&id)
            .ok_or_else(|| Error::CorruptIndex("server insertion failed".to_string()))
    }

    pub fn unregister(&mut self, id: &str) -> Result<()> {
        self.stop(id)?;
        if self.servers.remove(id).is_none() {
            return Err(Error::InvalidArgument(format!(
                "MCP server not found: {id}"
            )));
        }
        Ok(())
    }

    pub fn start(&mut self, id: &str) -> Result<()> {
        let server = self
            .servers
            .get(id)
            .ok_or_else(|| Error::InvalidArgument(format!("MCP server not found: {id}")))?;
        if server.status == McpServerStatus::Running {
            return Ok(());
        }
        let command = server.command.clone();
        let args = server.args.clone();
        let env = server.env.clone();
        if let Some(server) = self.servers.get_mut(id) {
            server.status = McpServerStatus::Starting;
        }
        let mut process = Command::new(&command);
        process.args(&args);
        process.stdin(Stdio::piped());
        process.stdout(Stdio::piped());
        process.stderr(Stdio::piped());
        for (key, value) in &env {
            process.env(key, value);
        }
        match process.spawn() {
            Ok(child) => {
                self.processes.insert(id.to_string(), child);
                if let Some(server) = self.servers.get_mut(id) {
                    server.status = McpServerStatus::Running;
                    server.last_started_unix_ms = now_unix_ms();
                    server.last_error.clear();
                }
                Ok(())
            }
            Err(error) => {
                if let Some(server) = self.servers.get_mut(id) {
                    server.status = McpServerStatus::Error;
                    server.last_error = error.to_string();
                }
                Err(Error::Protocol(format!(
                    "failed to start MCP server `{id}`: {error}"
                )))
            }
        }
    }

    pub fn stop(&mut self, id: &str) -> Result<()> {
        if !self.servers.contains_key(id) {
            return Err(Error::InvalidArgument(format!(
                "MCP server not found: {id}"
            )));
        }
        if let Some(mut child) = self.processes.remove(id) {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(server) = self.servers.get_mut(id) {
            server.status = McpServerStatus::Stopped;
        }
        Ok(())
    }

    pub fn stop_all(&mut self) {
        let ids: Vec<String> = self.processes.keys().cloned().collect();
        for id in ids {
            let _ = self.stop(&id);
        }
    }

    pub fn list(&self) -> Vec<&ExternalMcpServer> {
        self.servers.values().collect()
    }

    pub fn to_json(&self) -> String {
        let servers_json: Vec<String> = self
            .servers
            .values()
            .map(|server| {
                let args: Vec<String> = server
                    .args
                    .iter()
                    .map(|argument| format!("\"{}\"", json_escape(argument)))
                    .collect();
                let env_keys: Vec<String> = server
                    .env
                    .keys()
                    .map(|key| format!("\"{}\"", json_escape(key)))
                    .collect();
                format!(
                    "{{\"id\":\"{}\",\"name\":\"{}\",\"command\":\"{}\",\"args\":[{}],\"env_keys\":[{}],\"status\":\"{}\",\"auto_start\":{},\"registered_unix_ms\":{},\"last_started_unix_ms\":{},\"last_error\":\"{}\"}}",
                    json_escape(&server.id),
                    json_escape(&server.name),
                    json_escape(&server.command),
                    args.join(","),
                    env_keys.join(","),
                    server.status.as_str(),
                    server.auto_start,
                    server.registered_unix_ms,
                    server.last_started_unix_ms,
                    json_escape(&server.last_error)
                )
            })
            .collect();
        format!("{{\"servers\":[{}]}}", servers_json.join(","))
    }

    pub fn check_health(&mut self) {
        let ids: Vec<String> = self.processes.keys().cloned().collect();
        for id in ids {
            let state = self.processes.get_mut(&id).map(|child| child.try_wait());
            match state {
                Some(Ok(Some(status))) => {
                    self.processes.remove(&id);
                    if let Some(server) = self.servers.get_mut(&id) {
                        server.status = McpServerStatus::Error;
                        server.last_error = format!("process exited: {status}");
                    }
                }
                Some(Err(error)) => {
                    if let Some(server) = self.servers.get_mut(&id) {
                        server.status = McpServerStatus::Error;
                        server.last_error = error.to_string();
                    }
                }
                _ => {}
            }
        }
    }
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for McpManager {
    fn drop(&mut self) {
        self.stop_all();
    }
}

fn state_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".codespace")
        .join("mcp-servers.tsv")
}

pub fn load_mcp_manager() -> McpManager {
    let mut manager = McpManager::new();
    let Ok(content) = fs::read_to_string(state_path()) else {
        return manager;
    };
    for line in content.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 5 {
            continue;
        }
        let name = unescape(fields[0]);
        let command = unescape(fields[1]);
        let args = if fields[2].is_empty() {
            Vec::new()
        } else {
            fields[2].split('\u{1f}').map(unescape).collect()
        };
        let auto_start = fields[3] == "1";
        let registered = fields[4].parse::<u128>().unwrap_or_else(|_| now_unix_ms());
        let id = stable_id(&["mcp", &name, &command]).to_string();
        manager.servers.insert(
            id.clone(),
            ExternalMcpServer {
                id,
                name,
                command,
                args,
                env: BTreeMap::new(),
                status: McpServerStatus::Stopped,
                auto_start,
                registered_unix_ms: registered,
                last_started_unix_ms: 0,
                last_error: String::new(),
            },
        );
    }
    let auto_start_ids: Vec<String> = manager
        .servers
        .values()
        .filter(|server| server.auto_start)
        .map(|server| server.id.clone())
        .collect();
    for id in auto_start_ids {
        let _ = manager.start(&id);
    }
    manager
}

pub fn save_mcp_manager(manager: &McpManager) -> Result<()> {
    let path = state_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut output = String::new();
    for server in manager.servers.values() {
        let args = server
            .args
            .iter()
            .map(|argument| escape(argument))
            .collect::<Vec<_>>()
            .join("\u{1f}");
        output.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            escape(&server.name),
            escape(&server.command),
            args,
            if server.auto_start { "1" } else { "0" },
            server.registered_unix_ms
        ));
    }
    let temp = path.with_extension("tmp");
    fs::write(&temp, output)?;
    if path.exists() {
        let _ = fs::remove_file(&path);
    }
    fs::rename(temp, path)?;
    Ok(())
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
}

fn unescape(value: &str) -> String {
    let mut output = String::new();
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            match character {
                't' => output.push('\t'),
                'n' => output.push('\n'),
                '\\' => output.push('\\'),
                other => output.push(other),
            }
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else {
            output.push(character);
        }
    }
    if escaped {
        output.push('\\');
    }
    output
}
