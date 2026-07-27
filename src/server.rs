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
}

impl ServerConfig {
    pub fn new(port: u16) -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port,
            bootstrap_token: generate_token(),
        }
    }

    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

fn generate_token() -> String {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0u128, |d| d.as_nanos());
    let pid = std::process::id();
    let stack_addr = &nanos as *const u128 as usize;
    let mut state = 0xcbf2_9ce4_8422_2325_u64;
    for byte in nanos.to_le_bytes().iter() {
        state ^= u64::from(*byte);
        state = state.wrapping_mul(0x0000_0100_0000_01b3);
    }
    for byte in (pid as u64).to_le_bytes().iter() {
        state ^= u64::from(*byte);
        state = state.wrapping_mul(0x0000_0100_0000_01b3);
    }
    for byte in (stack_addr as u64).to_le_bytes().iter() {
        state ^= u64::from(*byte);
        state = state.wrapping_mul(0x0000_0100_0000_01b3);
    }
    for round in 0..16u64 {
        state ^= round.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        state = state.wrapping_mul(0x100000001b3);
    }
    let part1 = state;
    let part2 = state.wrapping_mul(0x100000001b3) ^ 0x6c62_2e35_7662_7a6f;
    format!("{:016x}{:016x}", part1, part2)
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

#[derive(Debug)]
pub struct ServerState {
    pub config: ServerConfig,
    pub started_unix_ms: u128,
    pub workspaces: WorkspaceRegistry,
    pub events: EventLog,
    pub skills: crate::skills::SkillRegistry,
    pub mcp: crate::mcp_manager::McpManager,
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
        }
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
        })
    } else {
        None
    }
}

pub fn serve(root: &Path, mut config: ServerConfig) -> Result<()> {
    let listener = bind_with_dynamic_port(&mut config)?;
    let address = config.address();
    eprintln!("CodeSpace server listening on http://{address}");
    eprintln!("Session token: {}", config.bootstrap_token);
    let state = Arc::new(Mutex::new(ServerState::new(config)));
    let registry = Arc::new(ActionRegistry::new());
    let root = Arc::new(root.to_path_buf());

    for stream_result in listener.incoming() {
        match stream_result {
            Ok(stream) => {
                let state = Arc::clone(&state);
                let registry = Arc::clone(&registry);
                let root = Arc::clone(&root);
                thread::spawn(move || {
                    if let Err(error) = handle_connection(stream, &root, &state, &registry) {
                        eprintln!("server request failed: {error}");
                    }
                });
            }
            Err(error) => eprintln!("server accept failed: {error}"),
        }
    }
    Ok(())
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
                config.port = port;
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

    let is_authorized = check_authorization(&request, &params, state);
    let is_public = path == "/api/v1/health"
        || path == "/api/v1/bootstrap"
        || path == "/"
        || path == "/dashboard"
        || path.starts_with("/assets/");

    if !is_authorized && !is_public {
        return write_json_response(&mut stream, 401, "{\"error\":\"unauthorized\"}");
    }

    let active_root = resolve_active_root(root, state);
    let root = &active_root;

    match (method, path) {
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
        ("GET", "/api/v1/dashboard") => {
            let token = {
                let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
                state_guard.config.bootstrap_token.clone()
            };
            let html = crate::dashboard::render_dashboard(&token);
            write_html_response(&mut stream, 200, &html)
        }
        ("POST", "/api/v1/ai/chat") => {
            let body = extract_body(&request);
            let query = extract_json_string(&body, "query").unwrap_or_default();
            if query.trim().is_empty() {
                return write_json_response(&mut stream, 400, "{\"error\":\"missing query\"}");
            }
            let model =
                extract_json_string(&body, "model").filter(|value| !value.trim().is_empty());
            let graph = load_graph(root)?;
            let context = {
                let ctx = ActionContext {
                    root: root.as_path().to_path_buf(),
                    graph,
                    format: OutputFormat::Markdown,
                };
                let mut action_params = ActionParams::default();
                action_params.positional.push(query.clone());
                action_params
                    .flags
                    .insert("max-tokens".to_string(), "2200".to_string());
                action_params
                    .flags
                    .insert("max-items".to_string(), "12".to_string());
                registry
                    .execute("context", &ctx, &action_params)
                    .map(|result| result.stdout)
                    .unwrap_or_default()
            };
            let mut session = crate::ai::ChatSession::new(model.as_deref());
            match crate::ai::chat(&mut session, &query, Some(&context)) {
                Ok(response) => write_json_response(
                    &mut stream,
                    200,
                    &format!(
                        "{{\"response\":\"{}\"}}",
                        crate::util::json_escape(&response)
                    ),
                ),
                Err(error) => write_json_response(
                    &mut stream,
                    500,
                    &format!(
                        "{{\"error\":\"{}\"}}",
                        crate::util::json_escape(&error.to_string())
                    ),
                ),
            }
        }
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
            let mut board = crate::tasks::load_tasks(root);
            let task = board.add(&title, &description, priority, None, tags);
            let task_id = task.id.clone();
            let task_title = task.title.clone();
            let _ = crate::tasks::save_tasks(root, &board);
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
            let mut board = crate::tasks::load_tasks(root);
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
            let mut board = crate::tasks::load_tasks(root);
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

fn check_authorization(
    request: &str,
    params: &BTreeMap<String, String>,
    state: &Arc<Mutex<ServerState>>,
) -> bool {
    let expected_token = {
        let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
        state_guard.config.bootstrap_token.clone()
    };
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
    if let Some(provided) = params.get("token") {
        return constant_time_eq(provided.as_bytes(), expected_token.as_bytes());
    }
    false
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
    let token = {
        let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
        state_guard.config.bootstrap_token.clone()
    };
    let html = crate::dashboard::render_dashboard(&token);
    write_html_response(stream, 200, &html)
}

fn handle_event_stream(stream: &mut TcpStream, state: &Arc<Mutex<ServerState>>) -> Result<()> {
    let headers = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream; charset=utf-8\r\nCache-Control: no-store\r\nConnection: keep-alive\r\nX-Accel-Buffering: no\r\nAccess-Control-Allow-Origin: http://localhost\r\n\r\n";
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
    let needle = format!("\"{key}\"");
    let pos = input.find(&needle)?;
    let after = &input[pos + needle.len()..];
    let colon = after.find(':')?;
    let rest = &after[colon + 1..];
    let trimmed = rest.trim_start();
    if !trimmed.starts_with('"') {
        return None;
    }
    let start = 1;
    let bytes = trimmed.as_bytes();
    let mut end = start;
    while end < bytes.len() {
        if bytes[end] == b'\\' {
            end += 2;
            continue;
        }
        if bytes[end] == b'"' {
            break;
        }
        end += 1;
    }
    Some(trimmed[start..end].to_string())
}

fn extract_json_object(input: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let pos = input.find(&needle)?;
    let after = &input[pos + needle.len()..];
    let colon = after.find(':')?;
    let rest = &after[colon + 1..];
    let leading_ws = rest.len() - rest.trim_start().len();
    let bytes = rest.as_bytes();
    let mut idx = leading_ws;
    if bytes.get(idx) != Some(&b'{') {
        return None;
    }
    let start = idx;
    let mut depth = 0_i32;
    while idx < bytes.len() {
        match bytes[idx] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(rest[start..=idx].to_string());
                }
            }
            b'"' => {
                idx += 1;
                while idx < bytes.len() && bytes[idx] != b'"' {
                    if bytes[idx] == b'\\' {
                        idx += 1;
                    }
                    idx += 1;
                }
            }
            _ => {}
        }
        idx += 1;
    }
    None
}

fn parse_action_params(input: &str) -> ActionParams {
    let mut params = ActionParams::default();
    if input.is_empty() {
        return params;
    }
    let mut idx = 0;
    let bytes = input.as_bytes();
    while idx < bytes.len() {
        if bytes[idx] == b'"' {
            if let Some((key, end)) = parse_json_string(input, idx) {
                idx = end;
                idx = skip_ws(bytes, idx);
                if idx < bytes.len() && bytes[idx] == b':' {
                    idx = skip_ws(bytes, idx + 1);
                    if bytes.get(idx) == Some(&b'"') {
                        if let Some((value, next)) = parse_json_string(input, idx) {
                            params.flags.insert(key, value);
                            idx = next;
                        } else {
                            idx += 1;
                        }
                    } else {
                        let (num, next) = parse_json_number(input, idx);
                        params.flags.insert(key, num.to_string());
                        idx = next;
                    }
                }
            } else {
                idx += 1;
            }
        } else {
            idx += 1;
        }
    }
    params
}

fn parse_json_string(input: &str, start: usize) -> Option<(String, usize)> {
    let bytes = input.as_bytes();
    if bytes.get(start) != Some(&b'"') {
        return None;
    }
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
                    _ => {}
                }
            }
            _ => {
                let remaining = &input[idx..];
                if let Some(ch) = remaining.chars().next() {
                    output.push(ch);
                    idx += ch.len_utf8() - 1;
                }
            }
        }
        idx += 1;
    }
    None
}

fn parse_json_number(input: &str, start: usize) -> (i64, usize) {
    let bytes = input.as_bytes();
    let mut idx = start;
    let s = idx;
    if idx < bytes.len() && bytes[idx] == b'-' {
        idx += 1;
    }
    while idx < bytes.len() && bytes[idx].is_ascii_digit() {
        idx += 1;
    }
    let value = input[s..idx].parse().unwrap_or(0);
    (value, idx)
}

fn skip_ws(bytes: &[u8], mut idx: usize) -> usize {
    while idx < bytes.len() && matches!(bytes[idx], b' ' | b'\t' | b'\n' | b'\r') {
        idx += 1;
    }
    idx
}

fn write_json_response(stream: &mut TcpStream, status: u16, body: &str) -> Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        _ => "Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\nCache-Control: no-store\r\nAccess-Control-Allow-Origin: http://localhost\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).map_err(Error::Io)
}

fn write_html_response(stream: &mut TcpStream, status: u16, body: &str) -> Result<()> {
    let reason = match status {
        200 => "OK",
        _ => "Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'\r\nCache-Control: no-store\r\nAccess-Control-Allow-Origin: http://localhost\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).map_err(Error::Io)
}

fn write_asset_response(stream: &mut TcpStream, content_type: &str, body: &str) -> Result<()> {
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\nCache-Control: no-cache\r\nAccess-Control-Allow-Origin: http://localhost\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).map_err(Error::Io)
}
