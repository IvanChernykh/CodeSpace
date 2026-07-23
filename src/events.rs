use crate::util::{json_escape, now_unix_ms};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct Event {
    pub event_type: EventType,
    pub workspace_id: String,
    pub state_version: u64,
    pub timestamp_unix_ms: u128,
    pub data: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    IndexUpdated,
    IndexStale,
    DecisionAdded,
    WorkspaceRegistered,
    WorkspaceRemoved,
    WorkspaceSelected,
    SettingsChanged,
    ServerStarted,
    ServerStopping,
    SkillInstalled,
    SkillRemoved,
    McpServerStarted,
    McpServerStopped,
}

impl EventType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::IndexUpdated => "index.updated",
            Self::IndexStale => "index.stale",
            Self::DecisionAdded => "decision.added",
            Self::WorkspaceRegistered => "workspace.registered",
            Self::WorkspaceRemoved => "workspace.removed",
            Self::WorkspaceSelected => "workspace.selected",
            Self::SettingsChanged => "settings.changed",
            Self::ServerStarted => "server.started",
            Self::ServerStopping => "server.stopping",
            Self::SkillInstalled => "skill.installed",
            Self::SkillRemoved => "skill.removed",
            Self::McpServerStarted => "mcp.server.started",
            Self::McpServerStopped => "mcp.server.stopped",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "index.updated" => Some(Self::IndexUpdated),
            "index.stale" => Some(Self::IndexStale),
            "decision.added" => Some(Self::DecisionAdded),
            "workspace.registered" => Some(Self::WorkspaceRegistered),
            "workspace.removed" => Some(Self::WorkspaceRemoved),
            "workspace.selected" => Some(Self::WorkspaceSelected),
            "settings.changed" => Some(Self::SettingsChanged),
            "server.started" => Some(Self::ServerStarted),
            "server.stopping" => Some(Self::ServerStopping),
            "skill.installed" => Some(Self::SkillInstalled),
            "skill.removed" => Some(Self::SkillRemoved),
            "mcp.server.started" => Some(Self::McpServerStarted),
            "mcp.server.stopped" => Some(Self::McpServerStopped),
            _ => None,
        }
    }
}

impl Event {
    pub fn new(event_type: EventType, workspace_id: &str, state_version: u64) -> Self {
        Self {
            event_type,
            workspace_id: workspace_id.to_string(),
            state_version,
            timestamp_unix_ms: now_unix_ms(),
            data: BTreeMap::new(),
        }
    }

    pub fn with_data(mut self, key: &str, value: &str) -> Self {
        self.data.insert(key.to_string(), value.to_string());
        self
    }

    pub fn to_json(&self) -> String {
        let data_pairs: Vec<String> = self
            .data
            .iter()
            .map(|(k, v)| format!("\"{}\":\"{}\"", json_escape(k), json_escape(v)))
            .collect();
        format!(
            "{{\"type\":\"{}\",\"workspace_id\":\"{}\",\"state_version\":{},\"timestamp_unix_ms\":{},\"data\":{{{}}}}}",
            self.event_type.as_str(),
            json_escape(&self.workspace_id),
            self.state_version,
            self.timestamp_unix_ms,
            data_pairs.join(",")
        )
    }
}

pub struct EventBus {
    subscribers: Vec<Box<dyn Fn(&Event) + Send + Sync>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self { subscribers: Vec::new() }
    }

    pub fn subscribe<F>(&mut self, callback: F)
    where
        F: Fn(&Event) + Send + Sync + 'static,
    {
        self.subscribers.push(Box::new(callback));
    }

    pub fn publish(&self, event: &Event) {
        for subscriber in &self.subscribers {
            subscriber(event);
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
