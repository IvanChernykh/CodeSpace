use crate::model::{Error, Result};
use crate::util::{json_escape, now_unix_ms, stable_id};
use std::collections::BTreeMap;
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
            drop(self.start(&id));
        }
        Ok(self.servers.get(&id).ok_or_else(|| Error::CorruptIndex("server insertion failed".to_string()))?)
    }

    pub fn unregister(&mut self, id: &str) -> Result<()> {
        self.stop(id)?;
        if self.servers.remove(id).is_none() {
            return Err(Error::InvalidArgument(format!("MCP server not found: {id}")));
        }
        Ok(())
    }

    pub fn start(&mut self, id: &str) -> Result<()> {
        let server = self.servers.get(id).ok_or_else(|| {
            Error::InvalidArgument(format!("MCP server not found: {id}"))
        })?;
        if server.status == McpServerStatus::Running {
            return Ok(());
        }
        let command = server.command.clone();
        let args = server.args.clone();
        let env = server.env.clone();

        let mut cmd = Command::new(&command);
        cmd.args(&args);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        for (key, value) in &env {
            cmd.env(key, value);
        }

        match cmd.spawn() {
            Ok(child) => {
                let pid = child.id();
                self.processes.insert(id.to_string(), child);
                if let Some(server) = self.servers.get_mut(id) {
                    server.status = McpServerStatus::Running;
                    server.last_started_unix_ms = now_unix_ms();
                    server.last_error = String::new();
                }
                eprintln!("MCP server `{}` started (pid={})", id, pid);
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
            .map(|s| {
                let args: Vec<String> = s.args.iter().map(|a| format!("\"{}\"", json_escape(a))).collect();
                let env_pairs: Vec<String> = s.env.iter().map(|(k, v)| format!("\"{}\":\"{}\"", json_escape(k), json_escape(v))).collect();
                format!(
                    "{{\"id\":\"{}\",\"name\":\"{}\",\"command\":\"{}\",\"args\":[{}],\"status\":\"{}\",\"auto_start\":{},\"registered_unix_ms\":{},\"last_started_unix_ms\":{},\"last_error\":\"{}\"}}",
                    json_escape(&s.id),
                    json_escape(&s.name),
                    json_escape(&s.command),
                    args.join(","),
                    s.status.as_str(),
                    s.auto_start,
                    s.registered_unix_ms,
                    s.last_started_unix_ms,
                    json_escape(&s.last_error)
                )
            })
            .collect();
        format!("{{\"servers\":[{}]}}", servers_json.join(","))
    }

    pub fn check_health(&mut self) {
        let ids: Vec<String> = self.processes.keys().cloned().collect();
        for id in ids {
            if let Some(child) = self.processes.get_mut(&id) {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        self.processes.remove(&id);
                        if let Some(server) = self.servers.get_mut(&id) {
                            server.status = McpServerStatus::Error;
                            server.last_error = format!("process exited: {status}");
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        if let Some(server) = self.servers.get_mut(&id) {
                            server.status = McpServerStatus::Error;
                            server.last_error = error.to_string();
                        }
                    }
                }
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
