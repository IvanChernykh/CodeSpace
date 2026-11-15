use crate::application::{ActionContext, ActionParams, ActionRegistry, OutputFormat};
use crate::events::{Event, EventType};
use crate::model::{Error, GraphIndex, Result};
use crate::storage;
use crate::util::json_escape;
use crate::workspace::{WorkspaceRegistry, load_global_registry};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const READ_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub bootstrap_token: String,
    pub open_desktop: bool,
}

impl ServerConfig {
    pub fn new(port: u16) -> Result<Self> {
        Ok(Self {
            host: "127.0.0.1".to_string(),
            port,
            bootstrap_token: generate_token()?,
            open_desktop: false,
        })
    }

    pub fn with_desktop(mut self) -> Self {
        self.open_desktop = true;
        self
    }

    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

fn generate_token() -> Result<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|error| {
        Error::Protocol(format!(
            "secure OS entropy is unavailable; refusing to start: {error}"
        ))
    })?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[derive(Debug, Clone)]
pub struct EventLog {
    next_id: u64,
    events: std::collections::VecDeque<(u64, String)>,
}

impl EventLog {
    fn new() -> Self {
        Self {
            next_id: 1,
            events: std::collections::VecDeque::new(),
        }
    }

    pub fn publish(&mut self, event: &Event) {
        let id = self.next_id;
        self.next_id += 1;
        self.events.push_back((id, event.to_json()));
        while self.events.len() > 200 {
            self.events.pop_front();
        }
    }

    fn since(&self, last_id: u64) -> Vec<(u64, String)> {
        self.events
            .iter()
            .filter(|(id, _)| *id > last_id)
            .cloned()
            .collect()
    }
}

const MAX_CONNECTIONS: usize = 64;
const AUTH_MAX_ATTEMPTS: usize = 10;
const AUTH_WINDOW_MS: u128 = 60_000;

#[derive(Debug)]
pub struct ServerState {
    pub config: ServerConfig,
    pub started_unix_ms: u128,
    pub workspaces: WorkspaceRegistry,
    pub events: EventLog,
    pub skills: crate::skills::SkillRegistry,
    pub mcp: crate::mcp_manager::McpManager,
    auth_failures: std::collections::HashMap<String, std::collections::VecDeque<u128>>,
}

impl ServerState {
    pub fn new(config: ServerConfig) -> Self {
        let mut events = EventLog::new();
        events.publish(&Event::new(EventType::ServerStarted, "", 0));
        Self {
            config,
            started_unix_ms: crate::util::now_unix_ms(),
            workspaces: load_global_registry(),
            events,
            skills: crate::skills::load_skill_registry(),
            mcp: crate::mcp_manager::load_mcp_manager(),
            auth_failures: std::collections::HashMap::new(),
        }
    }

    fn auth_allowed(&mut self, ip: &str) -> bool {
        let now = crate::util::now_unix_ms();
        let entries = self.auth_failures.entry(ip.to_string()).or_default();
        while entries
            .front()
            .is_some_and(|stamp| now.saturating_sub(*stamp) > AUTH_WINDOW_MS)
        {
            entries.pop_front();
        }
        entries.len() < AUTH_MAX_ATTEMPTS
    }

    fn record_auth_failure(&mut self, ip: &str) {
        let now = crate::util::now_unix_ms();
        self.auth_failures
            .entry(ip.to_string())
            .or_default()
            .push_back(now);
    }

    fn clear_auth_failures(&mut self, ip: &str) {
        self.auth_failures.remove(ip);
    }
}

pub fn discover_instance(port: u16) -> Option<ServerConfig> {
    let address = format!("127.0.0.1:{port}");
    let mut stream =
        TcpStream::connect_timeout(&address.parse().ok()?, Duration::from_millis(500)).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .ok()?;
    let request =
        "GET /api/v1/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n".to_string();
    stream.write_all(request.as_bytes()).ok()?;
    let mut buffer = Vec::with_capacity(4096);
    stream.read_to_end(&mut buffer).ok()?;
    let response = String::from_utf8_lossy(&buffer);
    if response.contains("\"status\":\"ok\"") {
        Some(ServerConfig {
            host: "127.0.0.1".to_string(),
            port,
            bootstrap_token: String::new(),
            open_desktop: false,
        })
    } else {
        None
    }
}

pub fn serve(root: &Path, mut config: ServerConfig) -> Result<()> {
    let listener = bind_with_dynamic_port(&mut config)?;
    let address = config.address();
    eprintln!("CodeSpace server listening on http://{address}");
    eprintln!("Session auth: HttpOnly SameSite cookie");
    if config.open_desktop {
        launch_desktop_window(config.port);
    }
    let state = Arc::new(Mutex::new(ServerState::new(config)));
    let registry = Arc::new(ActionRegistry::new());
    let root = Arc::new(root.to_path_buf());
    let active_connections = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    for stream_result in listener.incoming() {
        match stream_result {
            Ok(mut stream) => {
                if active_connections.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                    >= MAX_CONNECTIONS
                {
                    active_connections.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                    let _ = write_json_response(
                        &mut stream,
                        503,
                        "{\"error\":\"server busy: too many connections\"}",
                    );
                    continue;
                }
                let state = Arc::clone(&state);
                let registry = Arc::clone(&registry);
                let root = Arc::clone(&root);
                let active_connections = Arc::clone(&active_connections);
                thread::spawn(move || {
                    let result = handle_connection(stream, &root, &state, &registry);
                    active_connections.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                    if let Err(error) = result {
                        eprintln!("server request failed: {error}");
                    }
                });
            }
            Err(error) => eprintln!("server accept failed: {error}"),
        }
    }
    Ok(())
}

fn launch_desktop_window(port: u16) {
    let url = format!("http://127.0.0.1:{port}/dashboard");
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(180));
        if let Err(error) = open_app_window(&url) {
            eprintln!("Could not launch desktop window: {error}. Open {url} manually.");
        }
    });
}

#[cfg(target_os = "windows")]
fn open_app_window(url: &str) -> std::io::Result<()> {
    use std::path::PathBuf;
    use std::process::Command;

    let mut candidates = Vec::new();
    if let Ok(program_files) = std::env::var("ProgramFiles(x86)") {
        candidates.push(PathBuf::from(program_files).join("Microsoft/Edge/Application/msedge.exe"));
    }
    if let Ok(program_files) = std::env::var("ProgramFiles") {
        candidates.push(PathBuf::from(program_files).join("Microsoft/Edge/Application/msedge.exe"));
    }
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        candidates
            .push(PathBuf::from(&local_app_data).join("Microsoft/Edge/Application/msedge.exe"));
        candidates.push(PathBuf::from(local_app_data).join("Google/Chrome/Application/chrome.exe"));
    }
    for executable in candidates.into_iter().filter(|path| path.is_file()) {
        if Command::new(executable)
            .arg(format!("--app={url}"))
            .arg("--no-first-run")
            .spawn()
            .is_ok()
        {
            return Ok(());
        }
    }
    Command::new("explorer.exe").arg(url).spawn().map(|_| ())
}

#[cfg(target_os = "macos")]
fn open_app_window(url: &str) -> std::io::Result<()> {
    std::process::Command::new("open")
        .args(["-a", "Safari", url])
        .spawn()
        .map(|_| ())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_app_window(url: &str) -> std::io::Result<()> {
    std::process::Command::new("xdg-open")
        .arg(url)
        .spawn()
        .map(|_| ())
}

fn bind_with_dynamic_port(config: &mut ServerConfig) -> Result<TcpListener> {
    let start_port = config.port;
    for offset in 0..100u16 {
        let port = start_port.saturating_add(offset);
        let address = format!("{}:{}", config.host, port);
        match TcpListener::bind(&address) {
            Ok(listener) => {
                if offset > 0 {
                    eprintln!("Port {start_port} was busy, using port {port} instead");
                }
                config.port = listener.local_addr()?.port();
                return Ok(listener);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => continue,
            Err(e) => return Err(Error::Io(e)),
        }
    }
    Err(Error::InvalidArgument(format!(
        "no free port found in range {start_port}..{}",
        start_port.saturating_add(100)
    )))
}

fn handle_connection(
    mut stream: TcpStream,
    root: &Path,
    state: &Arc<Mutex<ServerState>>,
    registry: &ActionRegistry,
) -> Result<()> {
    stream.set_read_timeout(Some(READ_TIMEOUT))?;
    let request_data = read_http_request(&mut stream)?;
    let request = String::from_utf8_lossy(&request_data);
    let first_line = request.lines().next().unwrap_or_default();
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or("/");

    if !is_localhost_request(stream.peer_addr().ok()) {
        return write_json_response(&mut stream, 403, "{\"error\":\"forbidden: non-localhost\"}");
    }

    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let params = parse_query(query);

    let server_port = {
        let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
        state_guard.config.port
    };
    if !is_valid_host(&request, server_port) {
        return write_json_response(&mut stream, 400, "{\"error\":\"invalid host header\"}");
    }
    if method == "POST" && !is_allowed_origin(&request, server_port) {
        return write_json_response(&mut stream, 403, "{\"error\":\"forbidden: cross-origin\"}");
    }

    let is_public = path == "/api/v1/health"
        || path == "/api/v1/bootstrap"
        || path == "/"
        || path == "/dashboard"
        || path.starts_with("/assets/");

    let peer_ip = stream
        .peer_addr()
        .map_or_else(|_| String::new(), |addr| addr.ip().to_string());
    let (is_authorized, rate_limited) = {
        let mut state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
        if !is_public && !state_guard.auth_allowed(&peer_ip) {
            (false, true)
        } else if !is_public {
            let authorized = check_authorization(&request, &state_guard);
            if authorized {
                state_guard.clear_auth_failures(&peer_ip);
            } else {
                state_guard.record_auth_failure(&peer_ip);
            }
            (authorized, false)
        } else {
            (true, false)
        }
    };
    if rate_limited {
        return write_json_response(&mut stream, 429, "{\"error\":\"too many requests\"}");
    }
    if !is_authorized && !is_public {
        return write_json_response(&mut stream, 401, "{\"error\":\"unauthorized\"}");
    }

    let active_root = resolve_active_root(root, state);
    let root = &active_root;

    match (method, path) {
        ("POST", "/api/v1/runtime") => {
            let body = extract_body(&request);
            let result = crate::agent_runtime::parse_json(&body).and_then(|value| {
                let operation = value
                    .get("operation")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| {
                        Error::InvalidArgument("runtime operation is required".to_string())
                    })?;
                let input = value
                    .get("input")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                crate::agent_runtime::execute(root, operation, &input)
            });
            match result {
                Ok(value) => write_json_response(&mut stream, 200, &value.to_string()),
                Err(error) => write_json_response(
                    &mut stream,
                    400,
                    &serde_json::json!({"error": error.to_string()}).to_string(),
                ),
            }
        }
        ("GET", "/") | ("GET", "/dashboard") => serve_dashboard(&mut stream, state),
        ("GET", path) if path.starts_with("/assets/") => match crate::dashboard::asset(path) {
            Some((content_type, body)) => write_asset_response(&mut stream, content_type, &body),
            None => write_json_response(&mut stream, 404, "{\"error\":\"asset not found\"}"),
        },
        ("GET", "/api/v1/health") => {
            let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
            let body = format!(
                "{{\"status\":\"ok\",\"version\":\"{}\",\"started_unix_ms\":{},\"workspaces\":{}}}",
                env!("CARGO_PKG_VERSION"),
                state_guard.started_unix_ms,
                state_guard.workspaces.list().len()
            );
            write_json_response(&mut stream, 200, &body)
        }
        ("GET", "/api/v1/bootstrap") => {
            let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
            let body = format!(
                "{{\"version\":\"{}\",\"requires_token\":true,\"workspaces\":{}}}",
                env!("CARGO_PKG_VERSION"),
                state_guard.workspaces.to_json()
            );
            write_json_response(&mut stream, 200, &body)
        }
        ("GET", "/api/v1/actions") => {
            let actions: Vec<String> = registry
                .list()
                .iter()
                .map(|meta| {
                    format!(
                        "{{\"name\":\"{}\",\"description\":\"{}\",\"category\":\"{}\",\"read_only\":{}}}",
                        meta.name,
                        json_escape(meta.description),
                        meta.category.as_str(),
                        meta.read_only
                    )
                })
                .collect();
            let body = format!("{{\"actions\":[{}]}}", actions.join(","));
            write_json_response(&mut stream, 200, &body)
        }
        ("GET", "/api/v1/graph") => {
            let graph = load_graph(root)?;
            let body = crate::export::to_json(&graph);
            write_json_response(&mut stream, 200, &body)
        }
        ("GET", "/api/v1/search") => {
            let query = params.get("q").map_or("", String::as_str).trim();
            if query.is_empty() {
                return write_json_response(
                    &mut stream,
                    400,
                    "{\"error\":\"missing q parameter\"}",
                );
            }
            let graph = load_graph(root)?;
            let ctx = ActionContext {
                root: root.as_path().to_path_buf(),
                graph,
                format: OutputFormat::Json,
            };
            let mut action_params = ActionParams::default();
            action_params.positional.push(query.to_string());
            if let Some(limit) = params.get("limit") {
                action_params
                    .flags
                    .insert("limit".to_string(), limit.clone());
            }
            if let Some(kind) = params.get("kind") {
                action_params.flags.insert("kind".to_string(), kind.clone());
            }
            match registry.execute("search", &ctx, &action_params) {
                Ok(result) => write_json_response(&mut stream, 200, &result.stdout),
                Err(error) => write_json_response(
                    &mut stream,
                    500,
                    &format!("{{\"error\":\"{}\"}}", json_escape(&error.to_string())),
                ),
            }
        }
        ("GET", "/api/v1/context") => {
            let query = params.get("q").map_or("", String::as_str).trim();
            if query.is_empty() {
                return write_json_response(
                    &mut stream,
                    400,
                    "{\"error\":\"missing q parameter\"}",
                );
            }
            let graph = load_graph(root)?;
            let ctx = ActionContext {
                root: root.as_path().to_path_buf(),
                graph,
                format: OutputFormat::Json,
            };
            let mut action_params = ActionParams::default();
            action_params.positional.push(query.to_string());
            if let Some(max_tokens) = params.get("max_tokens") {
                action_params
                    .flags
                    .insert("max-tokens".to_string(), max_tokens.clone());
            }
            if let Some(max_items) = params.get("max_items") {
                action_params
                    .flags
                    .insert("max-items".to_string(), max_items.clone());
            }
            match registry.execute("context", &ctx, &action_params) {
                Ok(result) => write_json_response(&mut stream, 200, &result.stdout),
                Err(error) => write_json_response(
                    &mut stream,
                    500,
                    &format!("{{\"error\":\"{}\"}}", json_escape(&error.to_string())),
                ),
            }
        }
        ("GET", "/api/v1/impact") => {
            let graph = load_graph(root)?;
            let ctx = ActionContext {
                root: root.as_path().to_path_buf(),
                graph,
                format: OutputFormat::Json,
            };
            let mut action_params = ActionParams::default();
            for key in ["from", "to", "depth"] {
                if let Some(v) = params.get(key) {
                    action_params.flags.insert(key.to_string(), v.clone());
                }
            }
            match registry.execute("impact", &ctx, &action_params) {
                Ok(result) => write_json_response(&mut stream, 200, &result.stdout),
                Err(error) => write_json_response(
                    &mut stream,
                    500,
                    &format!("{{\"error\":\"{}\"}}", json_escape(&error.to_string())),
                ),
            }
        }
        ("GET", "/api/v1/history") => {
            let graph = load_graph(root)?;
            let ctx = ActionContext {
                root: root.as_path().to_path_buf(),
                graph,
                format: OutputFormat::Json,
            };
            let mut action_params = ActionParams::default();
            action_params
                .positional
                .push(params.get("q").cloned().unwrap_or_default());
            if let Some(v) = params.get("limit") {
                action_params.flags.insert("limit".to_string(), v.clone());
            }
            match registry.execute("history", &ctx, &action_params) {
                Ok(result) => write_json_response(&mut stream, 200, &result.stdout),
                Err(error) => write_json_response(
                    &mut stream,
                    500,
                    &format!("{{\"error\":\"{}\"}}", json_escape(&error.to_string())),
                ),
            }
        }
        ("GET", "/api/v1/read") => {
            let graph = load_graph(root)?;
            let ctx = ActionContext {
                root: root.as_path().to_path_buf(),
                graph,
                format: OutputFormat::Json,
            };
            let mut action_params = ActionParams::default();
            action_params
                .positional
                .push(params.get("file").cloned().unwrap_or_default());
            if let Some(v) = params.get("max_lines") {
                action_params
                    .flags
                    .insert("max-lines".to_string(), v.clone());
            }
            if let Some(v) = params.get("start_line") {
                action_params
                    .flags
                    .insert("start-line".to_string(), v.clone());
            }
            match registry.execute("read", &ctx, &action_params) {
                Ok(result) => write_json_response(
                    &mut stream,
                    200,
                    &format!("{{\"content\":\"{}\"}}", json_escape(&result.stdout)),
                ),
                Err(error) => write_json_response(
                    &mut stream,
                    400,
                    &format!("{{\"error\":\"{}\"}}", json_escape(&error.to_string())),
                ),
            }
        }
        ("GET", "/api/v1/stats") => {
            let graph = load_graph(root)?;
            let ctx = ActionContext {
                root: root.as_path().to_path_buf(),
                graph,
                format: OutputFormat::Json,
            };
            match registry.execute("stats", &ctx, &ActionParams::default()) {
                Ok(result) => write_json_response(&mut stream, 200, &result.stdout),
                Err(error) => write_json_response(
                    &mut stream,
                    500,
                    &format!("{{\"error\":\"{}\"}}", json_escape(&error.to_string())),
                ),
            }
        }
        ("GET", "/api/v1/workspaces") => {
            let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
            write_json_response(&mut stream, 200, &state_guard.workspaces.to_json())
        }
        ("POST", "/api/v1/workspaces/register") => {
            let path = params.get("path").map_or("", String::as_str);
            let name = params.get("name").map(|s| s.as_str());
            if path.is_empty() {
                return write_json_response(
                    &mut stream,
                    400,
                    "{\"error\":\"missing path parameter\"}",
                );
            }
            let mut state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
            match state_guard.workspaces.register(Path::new(path), name) {
                Ok(ws) => {
                    let (id, name, path) = (ws.id.clone(), ws.name.clone(), ws.path.clone());
                    let _ = crate::workspace::save_global_registry(&state_guard.workspaces);
                    state_guard.events.publish(
                        &Event::new(EventType::WorkspaceRegistered, &id, 0)
                            .with_data("name", &name)
                            .with_data("path", &path),
                    );
                    let body = format!(
                        "{{\"id\":\"{}\",\"name\":\"{}\",\"path\":\"{}\"}}",
                        json_escape(&id),
                        json_escape(&name),
                        json_escape(&path)
                    );
                    write_json_response(&mut stream, 200, &body)
                }
                Err(error) => write_json_response(
                    &mut stream,
                    400,
                    &format!("{{\"error\":\"{}\"}}", json_escape(&error.to_string())),
                ),
            }
        }
        ("POST", "/api/v1/workspaces/select") => {
            let id = params.get("id").map_or("", String::as_str);
            if id.is_empty() {
                return write_json_response(
                    &mut stream,
                    400,
                    "{\"error\":\"missing id parameter\"}",
                );
            }
            let mut state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
            match state_guard.workspaces.select(id) {
                Ok(()) => {
                    let _ = crate::workspace::save_global_registry(&state_guard.workspaces);
                    state_guard
                        .events
                        .publish(&Event::new(EventType::WorkspaceSelected, id, 0));
                    write_json_response(&mut stream, 200, "{\"status\":\"selected\"}")
                }
                Err(error) => write_json_response(
                    &mut stream,
                    400,
                    &format!("{{\"error\":\"{}\"}}", json_escape(&error.to_string())),
                ),
            }
        }
        ("POST", "/api/v1/workspaces/remove") => {
            let id = params.get("id").map_or("", String::as_str);
            if id.is_empty() {
                return write_json_response(
                    &mut stream,
                    400,
                    "{\"error\":\"missing id parameter\"}",
                );
            }
            let mut state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
            match state_guard.workspaces.remove(id) {
                Ok(()) => {
                    let _ = crate::workspace::save_global_registry(&state_guard.workspaces);
                    state_guard
                        .events
                        .publish(&Event::new(EventType::WorkspaceRemoved, id, 0));
                    write_json_response(&mut stream, 200, "{\"status\":\"removed\"}")
                }
                Err(error) => write_json_response(
                    &mut stream,
                    400,
                    &format!("{{\"error\":\"{}\"}}", json_escape(&error.to_string())),
                ),
            }
        }
        ("POST", "/api/v1/remember") => {
            let graph = load_graph(root)?;
            let ctx = ActionContext {
                root: root.as_path().to_path_buf(),
                graph,
                format: OutputFormat::Json,
            };
            let mut action_params = ActionParams::default();
            for key in [
                "file",
                "symbol",
                "summary",
                "rationale",
                "session",
                "agent",
                "tags",
            ] {
                if let Some(v) = params.get(key) {
                    action_params.flags.insert(key.to_string(), v.clone());
                }
            }
            match registry.execute("remember", &ctx, &action_params) {
                Ok(result) => {
                    let mut state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
                    state_guard.events.publish(&Event::new(
                        EventType::DecisionAdded,
                        "",
                        result.state_version,
                    ));
                    write_json_response(
                        &mut stream,
                        200,
                        &format!("{{\"message\":\"{}\"}}", json_escape(&result.stdout)),
                    )
                }
                Err(error) => write_json_response(
                    &mut stream,
                    400,
                    &format!("{{\"error\":\"{}\"}}", json_escape(&error.to_string())),
                ),
            }
        }
        ("POST", "/api/v1/update") => {
            let graph = load_graph(root)?;
            let ctx = ActionContext {
                root: root.as_path().to_path_buf(),
                graph,
                format: OutputFormat::Json,
            };
            let mut action_params = ActionParams::default();
            if params.contains_key("force") {
                action_params
                    .flags
                    .insert("force".to_string(), "true".to_string());
            }
            match registry.execute("update", &ctx, &action_params) {
                Ok(result) => {
                    let mut state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
                    state_guard.events.publish(&Event::new(
                        EventType::IndexUpdated,
                        "",
                        result.state_version,
                    ));
                    write_json_response(&mut stream, 200, &result.stdout)
                }
                Err(error) => write_json_response(
                    &mut stream,
                    500,
                    &format!("{{\"error\":\"{}\"}}", json_escape(&error.to_string())),
                ),
            }
        }
        ("POST", "/api/v1/doctor") => {
            let graph = load_graph(root)?;
            let ctx = ActionContext {
                root: root.as_path().to_path_buf(),
                graph,
                format: OutputFormat::Json,
            };
            let mut action_params = ActionParams::default();
            if params.contains_key("repair") {
                action_params
                    .flags
                    .insert("repair".to_string(), "true".to_string());
            }
            match registry.execute("doctor", &ctx, &action_params) {
                Ok(result) => {
                    let lines: Vec<String> = result
                        .stdout
                        .lines()
                        .map(|l| format!("\"{}\"", json_escape(l)))
                        .collect();
                    write_json_response(
                        &mut stream,
                        200,
                        &format!("{{\"messages\":[{}]}}", lines.join(",")),
                    )
                }
                Err(error) => write_json_response(
                    &mut stream,
                    500,
                    &format!("{{\"error\":\"{}\"}}", json_escape(&error.to_string())),
                ),
            }
        }
        ("GET", "/api/v1/dashboard") => serve_dashboard(&mut stream, state),
        ("GET", "/api/v1/tasks") => {
            let board = crate::tasks::load_tasks(root);
            write_json_response(&mut stream, 200, &board.to_json())
        }
        ("POST", "/api/v1/tasks") => {
            let body = extract_body(&request);
            let title = extract_json_string(&body, "title").unwrap_or_default();
            if title.is_empty() {
                return write_json_response(&mut stream, 400, "{\"error\":\"missing title\"}");
            }
            let description = extract_json_string(&body, "description").unwrap_or_default();
            let priority = extract_json_string(&body, "priority")
                .and_then(|p| crate::tasks::TaskPriority::parse(&p))
                .unwrap_or(crate::tasks::TaskPriority::Medium);
            let tags: Vec<String> = extract_json_string(&body, "tags")
                .map(|t| t.split(',').map(String::from).collect())
                .unwrap_or_default();
            let mut board = crate::tasks::transaction(root)?;
            let task = board.add(&title, &description, priority, None, tags);
            let task_id = task.id.clone();
            let task_title = task.title.clone();
            crate::tasks::save_tasks(root, &board)?;
            write_json_response(
                &mut stream,
                200,
                &format!(
                    "{{\"id\":\"{}\",\"title\":\"{}\"}}",
                    crate::util::json_escape(&task_id),
                    crate::util::json_escape(&task_title)
                ),
            )
        }
        ("POST", "/api/v1/tasks/status") => {
            let body = extract_body(&request);
            let id = extract_json_string(&body, "id").unwrap_or_default();
            let status = extract_json_string(&body, "status").unwrap_or_default();
            let Some(status) = crate::tasks::TaskStatus::parse(&status) else {
                return write_json_response(
                    &mut stream,
                    400,
                    "{\"error\":\"invalid task status\"}",
                );
            };
            let mut board = crate::tasks::transaction(root)?;
            match board.set_status(&id, status) {
                Ok(()) => {
                    crate::tasks::save_tasks(root, &board)?;
                    write_json_response(&mut stream, 200, "{\"status\":\"updated\"}")
                }
                Err(error) => write_json_response(
                    &mut stream,
                    400,
                    &format!("{{\"error\":\"{}\"}}", json_escape(&error.to_string())),
                ),
            }
        }
        ("POST", "/api/v1/tasks/remove") => {
            let body = extract_body(&request);
            let id = extract_json_string(&body, "id").unwrap_or_default();
            let mut board = crate::tasks::transaction(root)?;
            match board.remove(&id) {
                Ok(()) => {
                    crate::tasks::save_tasks(root, &board)?;
                    write_json_response(&mut stream, 200, "{\"status\":\"removed\"}")
                }
                Err(error) => write_json_response(
                    &mut stream,
                    400,
                    &format!("{{\"error\":\"{}\"}}", json_escape(&error.to_string())),
                ),
            }
        }
        ("GET", "/api/v1/skills") => {
            let state_guard = state.lock().unwrap_or_else(|error| error.into_inner());
            write_json_response(&mut stream, 200, &state_guard.skills.to_json())
        }
        ("POST", "/api/v1/skills/toggle") => {
            let body = extract_body(&request);
            let id = extract_json_string(&body, "id").unwrap_or_default();
            let enabled = extract_json_bool(&body, "enabled").unwrap_or(false);
            let mut state_guard = state.lock().unwrap_or_else(|error| error.into_inner());
            let result = if enabled {
                state_guard.skills.enable(&id)
            } else {
                state_guard.skills.disable(&id)
            };
            match result {
                Ok(()) => {
                    crate::skills::save_skill_registry(&state_guard.skills)?;
                    let event_type = if enabled {
                        EventType::SkillInstalled
                    } else {
                        EventType::SkillRemoved
                    };
                    state_guard.events.publish(&Event::new(event_type, &id, 0));
                    write_json_response(
                        &mut stream,
                        200,
                        &format!(
                            "{{\"id\":\"{}\",\"enabled\":{}}}",
                            json_escape(&id),
                            enabled
                        ),
                    )
                }
                Err(error) => write_json_response(
                    &mut stream,
                    400,
                    &format!("{{\"error\":\"{}\"}}", json_escape(&error.to_string())),
                ),
            }
        }
        ("GET", "/api/v1/mcp") => {
            let mut state_guard = state.lock().unwrap_or_else(|error| error.into_inner());
            state_guard.mcp.check_health();
            write_json_response(&mut stream, 200, &state_guard.mcp.to_json())
        }
        ("POST", "/api/v1/mcp/register") => {
            let body = extract_body(&request);
            let name = extract_json_string(&body, "name").unwrap_or_default();
            let command = extract_json_string(&body, "command").unwrap_or_default();
            let args = split_command_args(&extract_json_string(&body, "args").unwrap_or_default());
            let auto_start = extract_json_bool(&body, "auto_start").unwrap_or(false);
            let mut state_guard = state.lock().unwrap_or_else(|error| error.into_inner());
            let server_id =
                match state_guard
                    .mcp
                    .register(&name, &command, args, BTreeMap::new(), auto_start)
                {
                    Ok(server) => server.id.clone(),
                    Err(error) => {
                        return write_json_response(
                            &mut stream,
                            400,
                            &format!("{{\"error\":\"{}\"}}", json_escape(&error.to_string())),
                        );
                    }
                };
            crate::mcp_manager::save_mcp_manager(&state_guard.mcp)?;
            if auto_start {
                state_guard
                    .events
                    .publish(&Event::new(EventType::McpServerStarted, &server_id, 0));
            }
            write_json_response(
                &mut stream,
                200,
                &format!("{{\"id\":\"{}\"}}", json_escape(&server_id)),
            )
        }
        ("POST", "/api/v1/mcp/start")
        | ("POST", "/api/v1/mcp/stop")
        | ("POST", "/api/v1/mcp/remove") => {
            let id = params.get("id").map_or("", String::as_str);
            let mut state_guard = state.lock().unwrap_or_else(|error| error.into_inner());
            let result = match path {
                "/api/v1/mcp/start" => state_guard.mcp.start(id),
                "/api/v1/mcp/stop" => state_guard.mcp.stop(id),
                _ => state_guard.mcp.unregister(id),
            };
            match result {
                Ok(()) => {
                    crate::mcp_manager::save_mcp_manager(&state_guard.mcp)?;
                    let event_type = if path == "/api/v1/mcp/start" {
                        EventType::McpServerStarted
                    } else {
                        EventType::McpServerStopped
                    };
                    state_guard.events.publish(&Event::new(event_type, id, 0));
                    write_json_response(&mut stream, 200, "{\"status\":\"ok\"}")
                }
                Err(error) => write_json_response(
                    &mut stream,
                    400,
                    &format!("{{\"error\":\"{}\"}}", json_escape(&error.to_string())),
                ),
            }
        }
        ("GET", "/api/v1/settings") => {
            let global = crate::settings::load_global_settings();
            let workspace = crate::settings::load_workspace_settings(root);
            let mut effective = global.clone();
            effective.merge(&workspace);
            let body = format!(
                "{{\"global\":{},\"workspace\":{},\"effective\":{}}}",
                global.to_json(),
                workspace.to_json(),
                effective.to_json()
            );
            write_json_response(&mut stream, 200, &body)
        }
        ("POST", "/api/v1/settings") => {
            let body = extract_body(&request);
            let key = extract_json_string(&body, "key").unwrap_or_default();
            let value = extract_json_string(&body, "value").unwrap_or_default();
            let scope =
                extract_json_string(&body, "scope").unwrap_or_else(|| "workspace".to_string());
            if key.trim().is_empty() {
                return write_json_response(
                    &mut stream,
                    400,
                    "{\"error\":\"missing setting key\"}",
                );
            }
            if scope == "global" {
                let mut settings = crate::settings::load_global_settings();
                settings.set(&key, &value);
                crate::settings::save_global_settings(&settings)?;
            } else {
                let mut settings = crate::settings::load_workspace_settings(root);
                settings.set(&key, &value);
                crate::settings::save_workspace_settings(root, &settings)?;
            }
            let mut state_guard = state.lock().unwrap_or_else(|error| error.into_inner());
            state_guard
                .events
                .publish(&Event::new(EventType::SettingsChanged, "", 0).with_data("key", &key));
            write_json_response(&mut stream, 200, "{\"status\":\"saved\"}")
        }
        ("GET", "/api/v1/github/status") => {
            let config = crate::github_integration::status(root);
            write_json_response(&mut stream, 200, &config.to_json())
        }
        ("GET", "/api/v1/events") => handle_event_stream(&mut stream, state),
        ("POST", "/api/v1/actions") => {
            let body = extract_body(&request);
            let action_name = extract_json_string(&body, "action").unwrap_or_default();
            if action_name.is_empty() {
                return write_json_response(
                    &mut stream,
                    400,
                    "{\"error\":\"missing action name\"}",
                );
            }
            let action_input = extract_json_object(&body, "input").unwrap_or_default();
            let mut action_params = parse_action_params(&action_input);
            if let Some(query) = action_params.get("query").map(str::to_string) {
                action_params.positional.push(query);
            }
            let graph = load_graph(root)?;
            let ctx = ActionContext {
                root: root.as_path().to_path_buf(),
                graph,
                format: OutputFormat::Json,
            };
            match registry.execute(&action_name, &ctx, &action_params) {
                Ok(result) => {
                    let response_body = format!(
                        "{{\"exit_code\":{},\"stdout\":\"{}\",\"stderr\":\"{}\",\"state_version\":{}}}",
                        result.exit_code,
                        json_escape(&result.stdout),
                        json_escape(&result.stderr),
                        result.state_version
                    );
                    write_json_response(&mut stream, 200, &response_body)
                }
                Err(error) => write_json_response(
                    &mut stream,
                    500,
                    &format!("{{\"error\":\"{}\"}}", json_escape(&error.to_string())),
                ),
            }
        }
        _ => write_json_response(&mut stream, 404, "{\"error\":\"not found\"}"),
    }
}

fn read_http_request(stream: &mut TcpStream) -> Result<Vec<u8>> {
    const MAX_REQUEST_BYTES: usize = 1_048_576;
    let mut request = Vec::new();
    let mut buffer = [0_u8; 16_384];
    let mut expected_total = None;
    loop {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..count]);
        if request.len() > MAX_REQUEST_BYTES {
            return Err(Error::Protocol("request exceeds 1 MiB limit".to_string()));
        }
        if expected_total.is_none() {
            if let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                let header_bytes = &request[..header_end];
                let headers = String::from_utf8_lossy(header_bytes);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        if name.eq_ignore_ascii_case("content-length") {
                            value.trim().parse::<usize>().ok()
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                expected_total = Some(header_end + 4 + content_length);
            }
        }
        if expected_total.is_some_and(|total| request.len() >= total) {
            break;
        }
    }
    if let Some(total) = expected_total {
        request.truncate(total);
    }
    Ok(request)
}

fn resolve_active_root(default_root: &Path, state: &Arc<Mutex<ServerState>>) -> PathBuf {
    let state_guard = state.lock().unwrap_or_else(|error| error.into_inner());
    state_guard
        .workspaces
        .active()
        .map(|workspace| PathBuf::from(&workspace.path))
        .filter(|path| path.is_dir())
        .unwrap_or_else(|| default_root.to_path_buf())
}

fn extract_json_bool(input: &str, key: &str) -> Option<bool> {
    let needle = format!("\"{key}\"");
    let position = input.find(&needle)?;
    let after = &input[position + needle.len()..];
    let colon = after.find(':')?;
    let value = after[colon + 1..].trim_start();
    if value.starts_with("true") {
        Some(true)
    } else if value.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn split_command_args(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;
    for character in input.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if let Some(active_quote) = quote {
            if character == active_quote {
                quote = None;
            } else {
                current.push(character);
            }
            continue;
        }
        if matches!(character, '\'' | '"') {
            quote = Some(character);
        } else if character.is_whitespace() {
            if !current.is_empty() {
                args.push(std::mem::take(&mut current));
            }
        } else {
            current.push(character);
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

fn is_localhost_request(peer: Option<std::net::SocketAddr>) -> bool {
    match peer {
        Some(addr) => {
            let ip = addr.ip();
            ip.is_loopback()
        }
        None => false,
    }
}

fn check_authorization(request: &str, state: &ServerState) -> bool {
    let expected_token = &state.config.bootstrap_token;
    if expected_token.is_empty() {
        return false;
    }
    if let Some(auth_line) = request
        .lines()
        .find(|line| line.to_ascii_lowercase().starts_with("authorization:"))
    {
        let token = auth_line.split(':').nth(1).unwrap_or("").trim();
        if let Some(provided) = token.strip_prefix("Bearer ") {
            if constant_time_eq(provided.as_bytes(), expected_token.as_bytes()) {
                return true;
            }
        }
    }
    if let Some(cookie_line) = request
        .lines()
        .find(|line| line.to_ascii_lowercase().starts_with("cookie:"))
    {
        let cookie_header = cookie_line.split_once(':').map_or("", |(_, value)| value);
        for part in cookie_header.split(';') {
            let (name, value) = part.trim().split_once('=').unwrap_or((part.trim(), ""));
            if name == "cse_session"
                && constant_time_eq(value.as_bytes(), expected_token.as_bytes())
            {
                return true;
            }
        }
    }
    false
}

fn is_valid_host(request: &str, port: u16) -> bool {
    let Some(host) = request.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.eq_ignore_ascii_case("host") {
            Some(value.trim())
        } else {
            None
        }
    }) else {
        return false;
    };
    let (hostname, host_port) = match host.rsplit_once(':') {
        Some((name, value)) if !value.is_empty() && value.chars().all(|c| c.is_ascii_digit()) => {
            (name, Some(value))
        }
        _ => (host, None),
    };
    let hostname_ok = hostname.eq_ignore_ascii_case("localhost")
        || hostname == "127.0.0.1"
        || hostname == "[::1]";
    let port_ok = host_port.is_none_or(|value| value.parse::<u16>().is_ok_and(|p| p == port));
    hostname_ok && port_ok
}

fn is_allowed_origin(request: &str, port: u16) -> bool {
    let Some(origin) = request.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.eq_ignore_ascii_case("origin") {
            Some(value.trim())
        } else {
            None
        }
    }) else {
        // Non-browser clients (curl, CLI, MCP) do not send Origin.
        return true;
    };
    origin == format!("http://localhost:{port}") || origin == format!("http://127.0.0.1:{port}")
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn load_graph(root: &Path) -> Result<GraphIndex> {
    storage::load(root)
}

fn serve_dashboard(stream: &mut TcpStream, state: &Arc<Mutex<ServerState>>) -> Result<()> {
    let (token, html) = {
        let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
        (
            state_guard.config.bootstrap_token.clone(),
            crate::dashboard::render_dashboard(),
        )
    };
    write_dashboard_response(stream, &token, &html)
}

fn write_dashboard_response(stream: &mut TcpStream, token: &str, html: &str) -> Result<()> {
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nSet-Cookie: cse_session={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age=86400\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'\r\nCache-Control: no-store\r\n\r\n{html}",
        html.len()
    );
    stream.write_all(response.as_bytes()).map_err(Error::Io)
}

fn handle_event_stream(stream: &mut TcpStream, state: &Arc<Mutex<ServerState>>) -> Result<()> {
    let headers = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream; charset=utf-8\r\nCache-Control: no-store\r\nConnection: keep-alive\r\nX-Accel-Buffering: no\r\n\r\n";
    stream.write_all(headers.as_bytes())?;
    stream.flush()?;
    let mut last_id = 0_u64;
    let mut idle_ticks = 0_u32;
    loop {
        let batch = {
            let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
            state_guard.events.since(last_id)
        };
        if batch.is_empty() {
            idle_ticks += 1;
            if idle_ticks >= 50 {
                idle_ticks = 0;
                if stream.write_all(b": ping\n\n").is_err() || stream.flush().is_err() {
                    return Ok(());
                }
            }
            thread::sleep(Duration::from_millis(300));
            continue;
        }
        idle_ticks = 0;
        for (id, json) in batch {
            last_id = id;
            let chunk = format!("id: {id}\ndata: {json}\n\n");
            if stream.write_all(chunk.as_bytes()).is_err() {
                return Ok(());
            }
        }
        if stream.flush().is_err() {
            return Ok(());
        }
    }
}

fn parse_query(query: &str) -> BTreeMap<String, String> {
    query
        .split('&')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let (key, value) = part.split_once('=').unwrap_or((part, ""));
            (url_decode(key), url_decode(value))
        })
        .collect()
}

fn url_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                output.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                if let (Some(high), Some(low)) =
                    (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
                {
                    output.push((high << 4) | low);
                    index += 3;
                } else {
                    output.push(bytes[index]);
                    index += 1;
                }
            }
            byte => {
                output.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&output).to_string()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn extract_body(request: &str) -> String {
    if let Some(pos) = request.find("\r\n\r\n") {
        request[pos + 4..].to_string()
    } else {
        String::new()
    }
}

fn extract_json_string(input: &str, key: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(input)
        .ok()?
        .get(key)?
        .as_str()
        .map(String::from)
}

fn extract_json_object(input: &str, key: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(input)
        .ok()?
        .get(key)?
        .as_object()
        .map(|object| serde_json::to_string(object).ok())?
}

fn parse_action_params(input: &str) -> ActionParams {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(input) else {
        return ActionParams::default();
    };
    let Some(object) = value.as_object() else {
        return ActionParams::default();
    };
    let mut params = ActionParams::default();
    for (key, value) in object {
        match value {
            serde_json::Value::String(text) => {
                params.flags.insert(key.clone(), text.clone());
            }
            serde_json::Value::Number(number) => {
                params.flags.insert(key.clone(), number.to_string());
            }
            serde_json::Value::Bool(flag) => {
                params.flags.insert(key.clone(), flag.to_string());
            }
            _ => {}
        }
    }
    params
}

fn write_json_response(stream: &mut TcpStream, status: u16, body: &str) -> Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\nCache-Control: no-store\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).map_err(Error::Io)
}

fn write_asset_response(stream: &mut TcpStream, content_type: &str, body: &str) -> Result<()> {
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\nCache-Control: no-cache\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).map_err(Error::Io)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_token_is_hex_and_unique() {
        let first = generate_token().unwrap_or_else(|error| panic!("generate token: {error}"));
        let second = generate_token().unwrap_or_else(|error| panic!("generate token: {error}"));
        assert_eq!(first.len(), 64);
        assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(first, second);
    }

    #[test]
    fn host_validation_accepts_localhost_only() {
        assert!(is_valid_host(
            "GET / HTTP/1.1\r\nHost: localhost:8080\r\n\r\n",
            8080
        ));
        assert!(is_valid_host(
            "GET / HTTP/1.1\r\nHost: 127.0.0.1:8080\r\n\r\n",
            8080
        ));
        assert!(is_valid_host(
            "GET / HTTP/1.1\r\nHost: [::1]:8080\r\n\r\n",
            8080
        ));
        assert!(is_valid_host(
            "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n",
            8080
        ));
        assert!(!is_valid_host(
            "GET / HTTP/1.1\r\nHost: evil.example.com\r\n\r\n",
            8080
        ));
        assert!(!is_valid_host(
            "GET / HTTP/1.1\r\nHost: localhost:9999\r\n\r\n",
            8080
        ));
        assert!(!is_valid_host("GET / HTTP/1.1\r\n\r\n", 8080));
    }

    #[test]
    fn origin_validation_accepts_localhost_only() {
        assert!(is_allowed_origin(
            "POST / HTTP/1.1\r\nOrigin: http://localhost:8080\r\n\r\n",
            8080
        ));
        assert!(is_allowed_origin(
            "POST / HTTP/1.1\r\nOrigin: http://127.0.0.1:8080\r\n\r\n",
            8080
        ));
        assert!(!is_allowed_origin(
            "POST / HTTP/1.1\r\nOrigin: http://evil.example.com\r\n\r\n",
            8080
        ));
        assert!(!is_allowed_origin(
            "POST / HTTP/1.1\r\nOrigin: http://localhost:9999\r\n\r\n",
            8080
        ));
        assert!(is_allowed_origin("POST / HTTP/1.1\r\n\r\n", 8080));
    }

    #[test]
    fn authorization_accepts_bearer_and_cookie_but_rejects_query_tokens() {
        let config = ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
            bootstrap_token: "test-token-123".to_string(),
            open_desktop: false,
        };
        let state = ServerState::new(config);
        let bearer = "GET /api/v1/graph HTTP/1.1\r\nAuthorization: Bearer test-token-123\r\n\r\n";
        assert!(check_authorization(bearer, &state));
        assert!(check_authorization(
            "GET /api/v1/graph HTTP/1.1\r\nCookie: cse_session=test-token-123\r\n\r\n",
            &state
        ));
        let query = "GET /api/v1/graph?token=test-token-123 HTTP/1.1\r\n\r\n";
        assert!(!check_authorization(query, &state));
        let wrong = "GET /api/v1/graph HTTP/1.1\r\nAuthorization: Bearer wrong\r\n\r\n";
        assert!(!check_authorization(wrong, &state));
    }

    #[test]
    fn rate_limiter_blocks_after_max_attempts() {
        let config = ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
            bootstrap_token: "test-token".to_string(),
            open_desktop: false,
        };
        let mut state = ServerState::new(config);
        for _ in 0..AUTH_MAX_ATTEMPTS {
            assert!(state.auth_allowed("127.0.0.1"));
            state.record_auth_failure("127.0.0.1");
        }
        assert!(!state.auth_allowed("127.0.0.1"));
        state.clear_auth_failures("127.0.0.1");
        assert!(state.auth_allowed("127.0.0.1"));
    }
}
