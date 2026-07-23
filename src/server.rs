use crate::application::{ActionContext, ActionParams, ActionRegistry, OutputFormat};
use crate::events::Event;
use crate::model::{Error, GraphIndex, Result};
use crate::storage;
use crate::util::json_escape;
use crate::workspace::{load_global_registry, WorkspaceRegistry};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const READ_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_REQUEST_SIZE: usize = 1_048_576;

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
pub struct ServerState {
    pub config: ServerConfig,
    pub started_unix_ms: u128,
    pub workspaces: WorkspaceRegistry,
}

impl ServerState {
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config,
            started_unix_ms: crate::util::now_unix_ms(),
            workspaces: load_global_registry(),
        }
    }
}

pub fn discover_instance(port: u16) -> Option<ServerConfig> {
    let address = format!("127.0.0.1:{port}");
    let mut stream = TcpStream::connect_timeout(
        &address.parse().ok()?,
        Duration::from_millis(500),
    ).ok()?;
    stream.set_read_timeout(Some(Duration::from_millis(500))).ok()?;
    let request = format!(
        "GET /api/v1/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    );
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
    root: &PathBuf,
    state: &Arc<Mutex<ServerState>>,
    registry: &ActionRegistry,
) -> Result<()> {
    stream.set_read_timeout(Some(READ_TIMEOUT))?;
    let mut buffer = [0_u8; 16_384];
    let mut request_data = Vec::new();
    loop {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        request_data.extend_from_slice(&buffer[..read]);
        if request_data.len() > MAX_REQUEST_SIZE {
            return write_json_response(&mut stream, 413, "{\"error\":\"request too large\"}");
        }
        if request_data.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
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

    let is_authorized = check_authorization(&request, state);
    let is_public = path == "/api/v1/health" || path == "/api/v1/bootstrap" || path == "/";

    if !is_authorized && !is_public {
        return write_json_response(&mut stream, 401, "{\"error\":\"unauthorized\"}");
    }

    match (method, path) {
        ("GET", "/") | ("GET", "/dashboard") => serve_dashboard(&mut stream),
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
                return write_json_response(&mut stream, 400, "{\"error\":\"missing q parameter\"}");
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
                action_params.flags.insert("limit".to_string(), limit.clone());
            }
            if let Some(kind) = params.get("kind") {
                action_params.flags.insert("kind".to_string(), kind.clone());
            }
            match registry.execute("search", &ctx, &action_params) {
                Ok(result) => write_json_response(&mut stream, 200, &result.stdout),
                Err(error) => write_json_response(&mut stream, 500, &format!("{{\"error\":\"{}\"}}", json_escape(&error.to_string()))),
            }
        }
        ("GET", "/api/v1/context") => {
            let query = params.get("q").map_or("", String::as_str).trim();
            if query.is_empty() {
                return write_json_response(&mut stream, 400, "{\"error\":\"missing q parameter\"}");
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
                action_params.flags.insert("max-tokens".to_string(), max_tokens.clone());
            }
            if let Some(max_items) = params.get("max_items") {
                action_params.flags.insert("max-items".to_string(), max_items.clone());
            }
            match registry.execute("context", &ctx, &action_params) {
                Ok(result) => write_json_response(&mut stream, 200, &result.stdout),
                Err(error) => write_json_response(&mut stream, 500, &format!("{{\"error\":\"{}\"}}", json_escape(&error.to_string()))),
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
                Err(error) => write_json_response(&mut stream, 500, &format!("{{\"error\":\"{}\"}}", json_escape(&error.to_string()))),
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
                return write_json_response(&mut stream, 400, "{\"error\":\"missing path parameter\"}");
            }
            let mut state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
            match state_guard.workspaces.register(Path::new(path), name) {
                Ok(ws) => {
                    let (id, name, path) = (ws.id.clone(), ws.name.clone(), ws.path.clone());
                    let _ = crate::workspace::save_global_registry(&state_guard.workspaces);
                    let body = format!(
                        "{{\"id\":\"{}\",\"name\":\"{}\",\"path\":\"{}\"}}",
                        json_escape(&id), json_escape(&name), json_escape(&path)
                    );
                    write_json_response(&mut stream, 200, &body)
                }
                Err(error) => write_json_response(&mut stream, 400, &format!("{{\"error\":\"{}\"}}", json_escape(&error.to_string()))),
            }
        }
        ("POST", "/api/v1/workspaces/select") => {
            let id = params.get("id").map_or("", String::as_str);
            if id.is_empty() {
                return write_json_response(&mut stream, 400, "{\"error\":\"missing id parameter\"}");
            }
            let mut state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
            match state_guard.workspaces.select(id) {
                Ok(()) => {
                    let _ = crate::workspace::save_global_registry(&state_guard.workspaces);
                    write_json_response(&mut stream, 200, "{\"status\":\"selected\"}")
                }
                Err(error) => write_json_response(&mut stream, 400, &format!("{{\"error\":\"{}\"}}", json_escape(&error.to_string()))),
            }
        }
        ("GET", "/api/v1/dashboard") => {
            let html = crate::dashboard::render_dashboard();
            write_html_response(&mut stream, 200, &html)
        }
        ("GET", "/api/v1/events") => {
            handle_event_stream(&mut stream, state)
        }
        ("POST", "/api/v1/actions") => {
            let body = extract_body(&request);
            let action_name = extract_json_string(&body, "action").unwrap_or_default();
            let action_input = extract_json_string(&body, "input").unwrap_or_default();
            if action_name.is_empty() {
                return write_json_response(&mut stream, 400, "{\"error\":\"missing action name\"}");
            }
            let graph = load_graph(root)?;
            let ctx = ActionContext {
                root: root.as_path().to_path_buf(),
                graph,
                format: OutputFormat::Json,
            };
            let action_params = parse_action_params(&action_input);
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
                Err(error) => write_json_response(&mut stream, 500, &format!("{{\"error\":\"{}\"}}", json_escape(&error.to_string()))),
            }
        }
        _ => write_json_response(&mut stream, 404, "{\"error\":\"not found\"}"),
    }
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

fn check_authorization(request: &str, state: &Arc<Mutex<ServerState>>) -> bool {
    let expected_token = {
        let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
        state_guard.config.bootstrap_token.clone()
    };
    if expected_token.is_empty() {
        return false;
    }
    if let Some(auth_line) = request.lines().find(|line| {
        line.to_ascii_lowercase().starts_with("authorization:")
    }) {
        let token = auth_line.split(':').nth(1).unwrap_or("").trim();
        if let Some(provided) = token.strip_prefix("Bearer ") {
            return constant_time_eq(provided.as_bytes(), expected_token.as_bytes());
        }
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

fn serve_dashboard(stream: &mut TcpStream) -> Result<()> {
    let html = crate::dashboard::render_dashboard();
    write_html_response(stream, 200, &html)
}

fn handle_event_stream(stream: &mut TcpStream, _state: &Arc<Mutex<ServerState>>) -> Result<()> {
    let headers = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: keep-alive\r\nAccess-Control-Allow-Origin: http://localhost\r\n\r\n"
    );
    stream.write_all(headers.as_bytes())?;
    stream.flush()?;
    let hello = format!("data: {}\n\n", Event::new(
        crate::events::EventType::ServerStarted,
        "",
        0,
    ).to_json());
    stream.write_all(hello.as_bytes())?;
    stream.flush()?;
    Ok(())
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
            b'+' => { output.push(b' '); index += 1; }
            b'%' if index + 2 < bytes.len() => {
                if let (Some(high), Some(low)) = (hex_value(bytes[index + 1]), hex_value(bytes[index + 2])) {
                    output.push((high << 4) | low);
                    index += 3;
                } else {
                    output.push(bytes[index]);
                    index += 1;
                }
            }
            byte => { output.push(byte); index += 1; }
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
        if bytes[end] == b'\\' { end += 2; continue; }
        if bytes[end] == b'"' { break; }
        end += 1;
    }
    Some(trimmed[start..end].to_string())
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
    if idx < bytes.len() && bytes[idx] == b'-' { idx += 1; }
    while idx < bytes.len() && bytes[idx].is_ascii_digit() { idx += 1; }
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
        200 => "OK", 400 => "Bad Request", 401 => "Unauthorized", 403 => "Forbidden",
        404 => "Not Found", 413 => "Payload Too Large", 500 => "Internal Server Error",
        _ => "Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\nAccess-Control-Allow-Origin: http://localhost\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).map_err(Error::Io)
}

fn write_html_response(stream: &mut TcpStream, status: u16, body: &str) -> Result<()> {
    let reason = match status {
        200 => "OK", _ => "Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\nAccess-Control-Allow-Origin: http://localhost\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).map_err(Error::Io)
}
