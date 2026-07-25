use crate::model::{Error, Result};
use crate::util::{json_escape, now_unix_ms};
use std::fs;
use std::io::{self, BufRead, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

const OLLAMA_HOST: &str = "127.0.0.1";
const OLLAMA_PORT: u16 = 11434;
const DEFAULT_MODEL: &str = "qwen2.5-coder:1.5b-instruct-q4_K_M";
const SYSTEM_PROMPT_EN: &str = "You are CodeSpace AI, a smart coding assistant integrated into the CodeSpace development system. You help with code analysis, debugging, architecture, and project management. Be concise and practical. Use the project context provided.";
const SYSTEM_PROMPT_RU: &str = "Ты — CodeSpace AI, умный помощник разработчика, интегрированный в систему CodeSpace. Ты помогаешь с анализом кода, отладкой, архитектурой и управлением проектом. Будь кратким и практичным. Используй предоставленный контекст проекта.";

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct ChatSession {
    pub id: String,
    pub messages: Vec<ChatMessage>,
    pub model: String,
    pub created_unix_ms: u128,
    pub language: String,
}

impl ChatSession {
    pub fn new(model: Option<&str>) -> Self {
        Self {
            id: format!("chat-{}", now_unix_ms()),
            messages: Vec::new(),
            model: model.unwrap_or(DEFAULT_MODEL).to_string(),
            created_unix_ms: now_unix_ms(),
            language: "auto".to_string(),
        }
    }

    pub fn add_user(&mut self, content: &str) {
        self.messages.push(ChatMessage {
            role: "user".to_string(),
            content: content.to_string(),
        });
    }

    pub fn add_assistant(&mut self, content: &str) {
        self.messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: content.to_string(),
        });
    }

    pub fn to_json(&self) -> String {
        let msgs: Vec<String> = self
            .messages
            .iter()
            .map(|m| {
                format!(
                    "{{\"role\":\"{}\",\"content\":\"{}\"}}",
                    json_escape(&m.role),
                    json_escape(&m.content)
                )
            })
            .collect();
        format!(
            "{{\"id\":\"{}\",\"model\":\"{}\",\"created_unix_ms\":{},\"language\":\"{}\",\"messages\":[{}]}}",
            json_escape(&self.id),
            json_escape(&self.model),
            self.created_unix_ms,
            json_escape(&self.language),
            msgs.join(",")
        )
    }
}

pub fn detect_language(text: &str) -> &'static str {
    let lower = text.to_lowercase();
    let ru_chars = lower.chars().filter(|c| "абвгдеёжзийклмнопрстуфхцчшщъыьэюя".contains(*c)).count();
    let total_chars = lower.chars().filter(|c| c.is_alphabetic()).count();
    if total_chars > 0 && ru_chars * 3 > total_chars {
        "ru"
    } else {
        "en"
    }
}

pub fn system_prompt(language: &str) -> &'static str {
    match language {
        "ru" => SYSTEM_PROMPT_RU,
        _ => SYSTEM_PROMPT_EN,
    }
}

pub fn chat(
    session: &mut ChatSession,
    user_input: &str,
    context: Option<&str>,
) -> Result<String> {
    let lang = detect_language(user_input);
    session.language = lang.to_string();

    let mut full_input = user_input.to_string();
    if let Some(ctx) = context {
        if !ctx.is_empty() {
            full_input = format!(
                "[Project context]\n{}\n\n[User request]\n{}",
                ctx, user_input
            );
        }
    }

    session.add_user(&full_input);

    let messages_json: Vec<String> = {
        let mut msgs = Vec::new();
        msgs.push(format!(
            "{{\"role\":\"system\",\"content\":\"{}\"}}",
            json_escape(system_prompt(lang))
        ));
        for m in &session.messages {
            msgs.push(format!(
                "{{\"role\":\"{}\",\"content\":\"{}\"}}",
                json_escape(&m.role),
                json_escape(&m.content)
            ));
        }
        msgs
    };

    let body = format!(
        "{{\"model\":\"{}\",\"messages\":[{}],\"stream\":false,\"options\":{{\"temperature\":0.7,\"top_p\":0.9,\"num_ctx\":8192}}}}",
        json_escape(&session.model),
        messages_json.join(",")
    );

    let response = ollama_request("/api/chat", &body)?;
    let assistant_content = extract_json_string(&response, "message")
        .and_then(|msg| extract_json_string(&msg, "content"))
        .unwrap_or_default();

    if assistant_content.is_empty() {
        return Err(Error::InvalidArgument(
            "empty response from Ollama; is the model running?".to_string(),
        ));
    }

    session.add_assistant(&assistant_content);
    save_session(session)?;

    Ok(assistant_content)
}

pub fn list_models() -> Result<Vec<String>> {
    let response = ollama_get_request("/api/tags")?;
    let mut models = Vec::new();
    let mut search = response.as_str();
    while let Some(pos) = search.find("\"name\"") {
        let after = &search[pos + 6..];
        if let Some(colon) = after.find(':') {
            let val_start = after[colon + 1..].find('"').map(|p| colon + 1 + p + 1);
            if let Some(start) = val_start {
                let rest = &after[start..];
                if let Some(end) = rest.find('"') {
                    models.push(rest[..end].to_string());
                    search = &rest[end + 1..];
                    continue;
                }
            }
        }
        search = &after[1..];
    }
    Ok(models)
}

pub fn ensure_model() -> Result<bool> {
    let models = list_models()?;
    let has_model = models.iter().any(|m| m.contains("qwen2.5-coder") || m.contains("1.5b"));
    Ok(has_model)
}

fn ollama_request(path: &str, body: &str) -> Result<String> {
    ollama_http("POST", path, body)
}

fn ollama_get_request(path: &str) -> Result<String> {
    ollama_http("GET", path, "")
}

fn ollama_http(method: &str, path: &str, body: &str) -> Result<String> {
    let address = format!("{OLLAMA_HOST}:{OLLAMA_PORT}");
    let mut stream = TcpStream::connect_timeout(
        &address
            .parse()
            .map_err(|e| Error::InvalidArgument(format!("invalid address: {e}")))?,
        Duration::from_secs(5),
    )
    .map_err(|e| Error::InvalidArgument(format!(
        "cannot connect to Ollama at {address}; is `ollama serve` running? ({e})"
    )))?;

    stream.set_read_timeout(Some(Duration::from_secs(120)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;

    let request = if body.is_empty() {
        format!(
            "{method} {path} HTTP/1.1\r\nHost: {OLLAMA_HOST}:{OLLAMA_PORT}\r\nConnection: close\r\n\r\n"
        )
    } else {
        format!(
            "{method} {path} HTTP/1.1\r\nHost: {OLLAMA_HOST}:{OLLAMA_PORT}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    };

    stream.write_all(request.as_bytes())?;

    let mut response_data = Vec::new();
    stream.read_to_end(&mut response_data)?;

    let response = String::from_utf8_lossy(&response_data);
    let body_start = response
        .find("\r\n\r\n")
        .ok_or_else(|| Error::InvalidArgument("malformed HTTP response from Ollama".to_string()))?;
    Ok(response[body_start + 4..].to_string())
}

fn extract_json_string(input: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let pos = input.find(&needle)?;
    let after = &input[pos + needle.len()..];
    let colon = after.find(':')?;
    let rest = &after[colon + 1..];
    let mut idx = 0;
    let bytes = rest.as_bytes();
    while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
        idx += 1;
    }
    if bytes.get(idx) != Some(&b'"') {
        return None;
    }
    idx += 1;
    let mut output = String::new();
    while idx < bytes.len() {
        match bytes[idx] {
            b'"' => return Some(output),
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
                        if let Some(hex) = rest.get(idx + 1..idx + 5) {
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
                if let Some(ch) = rest[idx..].chars().next() {
                    output.push(ch);
                    idx += ch.len_utf8() - 1;
                }
            }
        }
        idx += 1;
    }
    None
}

pub fn sessions_dir(root: &Path) -> std::path::PathBuf {
    root.join(".codespace").join("chat")
}

pub fn save_session(session: &ChatSession) -> Result<()> {
    let dir = sessions_dir(Path::new("."));
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", session.id));
    fs::write(&path, session.to_json())?;
    Ok(())
}

pub fn load_session(id: &str) -> Result<ChatSession> {
    let path = sessions_dir(Path::new(".")).join(format!("{id}.json"));
    let content = fs::read_to_string(&path)?;
    let model = extract_json_string(&content, "model").unwrap_or_else(|| DEFAULT_MODEL.to_string());
    let language = extract_json_string(&content, "language").unwrap_or_else(|| "auto".to_string());
    let mut session = ChatSession::new(Some(&model));
    session.id = id.to_string();
    session.language = language;
    let mut search = content.as_str();
    while let Some(pos) = search.find("\"role\"") {
        let after = &search[pos..];
        if let (Some(role), Some(content_val)) = (
            extract_json_string(after, "role"),
            extract_json_string(after, "content"),
        ) {
            session.messages.push(ChatMessage { role, content: content_val });
        }
        search = &after[6..];
    }
    Ok(session)
}

pub fn list_sessions() -> Result<Vec<(String, u128)>> {
    let dir = sessions_dir(Path::new("."));
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut sessions = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(id) = name.strip_suffix(".json") {
            let content = fs::read_to_string(entry.path())?;
            let ts = extract_json_string(&content, "created_unix_ms")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            sessions.push((id.to_string(), ts));
        }
    }
    sessions.sort_by(|a, b| b.1.cmp(&a.1));
    Ok(sessions)
}

pub fn interactive_chat(root: &Path, model: Option<&str>) -> Result<i32> {
    let has_model = ensure_model()?;
    if !has_model {
        eprintln!("Warning: qwen2.5-coder model not found in Ollama.");
        eprintln!("Run: ollama pull qwen2.5-coder:1.5b-instruct-q4_K_M");
        eprintln!("Continuing with available models...\n");
    }

    let mut session = ChatSession::new(model);
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();

    let lang = detect_language("hello");
    let _ = lang;
    println!("CodeSpace AI Chat (model: {})", session.model);
    println!("Type /exit to quit, /clear to reset, /lang to switch language");
    println!("---");

    loop {
        write!(stdout, "> ")?;
        stdout.flush()?;

        let mut input = String::new();
        if stdin.lock().read_line(&mut input).is_err() {
            break;
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        match input {
            "/exit" | "/quit" | "/q" => {
                println!("Goodbye!");
                break;
            }
            "/clear" | "/reset" => {
                session = ChatSession::new(model);
                println!("Session cleared.\n");
                continue;
            }
            "/lang" => {
                let new_lang = if session.language == "ru" { "en" } else { "ru" };
                session.language = new_lang.to_string();
                println!("Language: {new_lang}\n");
                continue;
            }
            "/help" => {
                println!("Commands: /exit, /clear, /lang, /help");
                continue;
            }
            _ if input.starts_with('/') => {
                println!("Unknown command. Type /help for commands.");
                continue;
            }
            _ => {}
        }

        let context = build_chat_context(root, input);

        print!("AI: ");
        stdout.flush()?;

        match chat(&mut session, input, context.as_deref()) {
            Ok(response) => {
                println!("{response}\n");
            }
            Err(e) => {
                eprintln!("Error: {e}\n");
            }
        }
    }

    Ok(0)
}

fn build_chat_context(root: &Path, query: &str) -> Option<String> {
    let graph = crate::storage::load(root).ok()?;
    let bundle = crate::context::build_context(
        root,
        &graph,
        query,
        &crate::context::ContextOptions {
            max_tokens: 2000,
            ..Default::default()
        },
    )
    .ok()?;

    let mut parts = Vec::new();
    for item in &bundle.items {
        parts.push(format!(
            "// {} ({}:{})\n{}",
            item.symbol, item.path, item.line_start, item.content
        ));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
}
