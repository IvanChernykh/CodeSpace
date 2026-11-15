use crate::model::{Error, Result};
use crate::util::{json_escape, now_unix_ms};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub created_unix_ms: u128,
    pub due_unix_ms: Option<u128>,
    pub completed_unix_ms: Option<u128>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Todo,
    InProgress,
    Done,
    Cancelled,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::InProgress => "in_progress",
            Self::Done => "done",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "todo" => Some(Self::Todo),
            "in_progress" | "progress" => Some(Self::InProgress),
            "done" | "completed" => Some(Self::Done),
            "cancelled" | "canceled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    Low,
    Medium,
    High,
    Critical,
}

impl TaskPriority {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "low" => Some(Self::Low),
            "medium" | "normal" => Some(Self::Medium),
            "high" => Some(Self::High),
            "critical" | "urgent" => Some(Self::Critical),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TaskBoard {
    pub tasks: BTreeMap<String, Task>,
}

impl TaskBoard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(
        &mut self,
        title: &str,
        description: &str,
        priority: TaskPriority,
        due: Option<u128>,
        tags: Vec<String>,
    ) -> &Task {
        static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = loop {
            let candidate = format!(
                "task-{}-{}",
                now_unix_ms(),
                SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            );
            if !self.tasks.contains_key(&candidate) {
                break candidate;
            }
        };
        let task = Task {
            id: id.clone(),
            title: title.to_string(),
            description: description.to_string(),
            status: TaskStatus::Todo,
            priority,
            created_unix_ms: now_unix_ms(),
            due_unix_ms: due,
            completed_unix_ms: None,
            tags,
        };
        self.tasks.entry(id).or_insert(task)
    }

    pub fn remove(&mut self, id: &str) -> Result<()> {
        if self.tasks.remove(id).is_none() {
            return Err(Error::InvalidArgument(format!("task not found: {id}")));
        }
        Ok(())
    }

    pub fn set_status(&mut self, id: &str, status: TaskStatus) -> Result<()> {
        let task = self
            .tasks
            .get_mut(id)
            .ok_or_else(|| Error::InvalidArgument(format!("task not found: {id}")))?;
        task.status = status;
        if status == TaskStatus::Done || status == TaskStatus::Cancelled {
            task.completed_unix_ms = Some(now_unix_ms());
        } else {
            task.completed_unix_ms = None;
        }
        Ok(())
    }

    pub fn set_priority(&mut self, id: &str, priority: TaskPriority) -> Result<()> {
        let task = self
            .tasks
            .get_mut(id)
            .ok_or_else(|| Error::InvalidArgument(format!("task not found: {id}")))?;
        task.priority = priority;
        Ok(())
    }

    pub fn list(&self) -> Vec<&Task> {
        let mut tasks: Vec<&Task> = self.tasks.values().collect();
        tasks.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| a.created_unix_ms.cmp(&b.created_unix_ms))
        });
        tasks
    }

    pub fn list_by_status(&self, status: TaskStatus) -> Vec<&Task> {
        self.list()
            .into_iter()
            .filter(|t| t.status == status)
            .collect()
    }

    pub fn list_upcoming(&self) -> Vec<&Task> {
        let now = now_unix_ms();
        self.list()
            .into_iter()
            .filter(|t| t.status == TaskStatus::Todo || t.status == TaskStatus::InProgress)
            .filter(|t| t.due_unix_ms.is_none_or(|due| due >= now))
            .collect()
    }

    pub fn to_json(&self) -> String {
        let tasks_json: Vec<String> = self.list()
            .iter()
            .map(|t| {
                format!(
                    "{{\"id\":\"{}\",\"title\":\"{}\",\"description\":\"{}\",\"status\":\"{}\",\"priority\":\"{}\",\"created_unix_ms\":{},\"due_unix_ms\":{},\"completed_unix_ms\":{},\"tags\":[{}]}}",
                    json_escape(&t.id),
                    json_escape(&t.title),
                    json_escape(&t.description),
                    t.status.as_str(),
                    t.priority.as_str(),
                    t.created_unix_ms,
                    t.due_unix_ms.map_or_else(|| "null".to_string(), |v| v.to_string()),
                    t.completed_unix_ms.map_or_else(|| "null".to_string(), |v| v.to_string()),
                    t.tags.iter().map(|tag| format!("\"{}\"", json_escape(tag))).collect::<Vec<_>>().join(",")
                )
            })
            .collect();
        format!("{{\"tasks\":[{}]}}", tasks_json.join(","))
    }
}

pub fn tasks_path(root: &Path) -> std::path::PathBuf {
    root.join(".codespace").join("tasks.json")
}

pub fn load_tasks(root: &Path) -> TaskBoard {
    let path = tasks_path(root);
    if !path.exists() {
        return TaskBoard::new();
    }
    match fs::read_to_string(&path) {
        Ok(content) => parse_tasks_json(&content),
        Err(_) => TaskBoard::new(),
    }
}

pub fn save_tasks(root: &Path, board: &TaskBoard) -> Result<()> {
    check_task_store(root)?;
    let path = tasks_path(root);
    let mut random = [0_u8; 8];
    getrandom::fill(&mut random)
        .map_err(|_| Error::Protocol("task storage entropy unavailable".to_string()))?;
    let temporary = path.with_extension(format!("tmp-{:x}", u64::from_le_bytes(random)));
    let serialized = board.to_json();
    if serialized.len() > 4_194_304 {
        return Err(Error::InvalidArgument(
            "task board exceeds 4 MiB".to_string(),
        ));
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| -> Result<()> {
        file.write_all(serialized.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, &path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result?;
    Ok(())
}

fn check_task_store(root: &Path) -> Result<()> {
    for path in [
        root.join(".codespace"),
        tasks_path(root),
        root.join(".codespace/tasks.lock"),
    ] {
        match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                #[cfg(windows)]
                let linked = {
                    use std::os::windows::fs::MetadataExt;
                    metadata.file_attributes() & 0x400 != 0
                };
                #[cfg(not(windows))]
                let linked = metadata.file_type().is_symlink();
                if linked {
                    return Err(Error::InvalidArgument(
                        "linked task storage is blocked".to_string(),
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    fs::create_dir_all(root.join(".codespace"))?;
    Ok(())
}

pub fn load_tasks_checked(root: &Path) -> Result<TaskBoard> {
    check_task_store(root)?;
    let path = tasks_path(root);
    if !path.exists() {
        return Ok(TaskBoard::new());
    }
    if fs::metadata(&path)?.len() > 4_194_304 {
        return Err(Error::InvalidArgument(
            "task board exceeds 4 MiB".to_string(),
        ));
    }
    let content = fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&content).map_err(|_| {
        Error::CorruptIndex("invalid tasks JSON; original file retained".to_string())
    })?;
    let rows = value["tasks"]
        .as_array()
        .ok_or_else(|| Error::CorruptIndex("tasks must be an array".to_string()))?;
    let board = parse_tasks_json(&content);
    if board.tasks.len() != rows.len() {
        return Err(Error::CorruptIndex(
            "invalid or duplicate task records; original file retained".to_string(),
        ));
    }
    Ok(board)
}

/// Hold an OS lock over the full read-modify-save cycle. Callers retain this
/// guard until save_tasks completes, so CLI and desktop updates cannot be lost.
pub struct TaskTransaction {
    board: TaskBoard,
    _lock: fs::File,
}
impl std::ops::Deref for TaskTransaction {
    type Target = TaskBoard;
    fn deref(&self) -> &Self::Target {
        &self.board
    }
}
impl std::ops::DerefMut for TaskTransaction {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.board
    }
}
pub fn transaction(root: &Path) -> Result<TaskTransaction> {
    check_task_store(root)?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(".codespace/tasks.lock"))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        match fs2::FileExt::try_lock_exclusive(&lock) {
            Ok(()) => break,
            Err(error)
                if (error.kind() == std::io::ErrorKind::WouldBlock
                    || error.raw_os_error() == Some(33))
                    && std::time::Instant::now() < deadline =>
            {
                std::thread::sleep(std::time::Duration::from_millis(10))
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(TaskTransaction {
        board: load_tasks_checked(root)?,
        _lock: lock,
    })
}

fn parse_tasks_json(content: &str) -> TaskBoard {
    let mut board = TaskBoard::new();
    let mut idx = 0;
    let bytes = content.as_bytes();

    while idx < bytes.len() {
        if bytes[idx] == b'"' {
            if let Some((key, end)) = parse_string(content, idx) {
                idx = end;
                idx = skip_ws(bytes, idx);
                if idx < bytes.len() && bytes[idx] == b':' {
                    idx = skip_ws(bytes, idx + 1);
                    if key == "tasks" {
                        if let Some((items, next)) = parse_array(content, idx) {
                            for item in items {
                                if let Some(task) = parse_task(&item) {
                                    board.tasks.insert(task.id.clone(), task);
                                }
                            }
                            idx = next;
                        }
                    } else {
                        idx = skip_value(bytes, idx);
                    }
                }
            } else {
                idx += 1;
            }
        } else {
            idx += 1;
        }
    }
    board
}

fn parse_task(content: &str) -> Option<Task> {
    let mut id = String::new();
    let mut title = String::new();
    let mut description = String::new();
    let mut status = TaskStatus::Todo;
    let mut priority = TaskPriority::Medium;
    let mut created = 0_u128;
    let mut due = None;
    let mut completed = None;
    let mut tags = Vec::new();
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
                        "id" => {
                            if let Some((v, n)) = parse_string(content, idx) {
                                id = v;
                                idx = n;
                            } else {
                                idx = skip_value(bytes, idx);
                            }
                        }
                        "title" => {
                            if let Some((v, n)) = parse_string(content, idx) {
                                title = v;
                                idx = n;
                            } else {
                                idx = skip_value(bytes, idx);
                            }
                        }
                        "description" => {
                            if let Some((v, n)) = parse_string(content, idx) {
                                description = v;
                                idx = n;
                            } else {
                                idx = skip_value(bytes, idx);
                            }
                        }
                        "status" => {
                            if let Some((v, n)) = parse_string(content, idx) {
                                status = TaskStatus::parse(&v).unwrap_or(TaskStatus::Todo);
                                idx = n;
                            } else {
                                idx = skip_value(bytes, idx);
                            }
                        }
                        "priority" => {
                            if let Some((v, n)) = parse_string(content, idx) {
                                priority = TaskPriority::parse(&v).unwrap_or(TaskPriority::Medium);
                                idx = n;
                            } else {
                                idx = skip_value(bytes, idx);
                            }
                        }
                        "created_unix_ms" => {
                            if let Some((v, n)) = parse_number(content, idx) {
                                created = v;
                                idx = n;
                            } else {
                                idx = skip_value(bytes, idx);
                            }
                        }
                        "due_unix_ms" => {
                            if let Some((v, n)) = parse_number(content, idx) {
                                due = Some(v);
                                idx = n;
                            } else {
                                idx = skip_value(bytes, idx);
                            }
                        }
                        "completed_unix_ms" => {
                            if let Some((v, n)) = parse_number(content, idx) {
                                completed = Some(v);
                                idx = n;
                            } else {
                                idx = skip_value(bytes, idx);
                            }
                        }
                        "tags" => {
                            if let Some((items, n)) = parse_string_array(content, idx) {
                                tags = items;
                                idx = n;
                            } else {
                                idx = skip_value(bytes, idx);
                            }
                        }
                        _ => {
                            idx = skip_value(bytes, idx);
                        }
                    }
                }
            } else {
                idx += 1;
            }
        } else {
            idx += 1;
        }
    }

    if !id.is_empty() {
        Some(Task {
            id,
            title,
            description,
            status,
            priority,
            created_unix_ms: created,
            due_unix_ms: due,
            completed_unix_ms: completed,
            tags,
        })
    } else {
        None
    }
}

fn parse_string(content: &str, start: usize) -> Option<(String, usize)> {
    let bytes = content.as_bytes();
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

fn parse_number(content: &str, start: usize) -> Option<(u128, usize)> {
    let bytes = content.as_bytes();
    let mut idx = start;
    let s = idx;
    while idx < bytes.len() && bytes[idx].is_ascii_digit() {
        idx += 1;
    }
    if idx > s {
        content[s..idx].parse().ok().map(|v| (v, idx))
    } else {
        None
    }
}

fn parse_array(content: &str, start: usize) -> Option<(Vec<String>, usize)> {
    let bytes = content.as_bytes();
    if bytes.get(start) != Some(&b'[') {
        return None;
    }
    let mut items = Vec::new();
    let mut idx = start + 1;
    let mut depth = 0;
    let mut item_start = idx;
    while idx < bytes.len() {
        match bytes[idx] {
            b'{' => {
                if depth == 0 {
                    item_start = idx;
                }
                depth += 1;
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    items.push(content[item_start..=idx].to_string());
                }
            }
            b']' if depth == 0 => return Some((items, idx + 1)),
            b'"' => {
                if let Some((_, end)) = parse_string(content, idx) {
                    idx = end - 1;
                }
            }
            _ => {}
        }
        idx += 1;
    }
    None
}

fn parse_string_array(content: &str, start: usize) -> Option<(Vec<String>, usize)> {
    let bytes = content.as_bytes();
    if bytes.get(start) != Some(&b'[') {
        return None;
    }
    let mut items = Vec::new();
    let mut idx = start + 1;
    loop {
        idx = skip_ws(bytes, idx);
        if idx >= bytes.len() {
            return None;
        }
        if bytes[idx] == b']' {
            return Some((items, idx + 1));
        }
        if bytes[idx] == b',' {
            idx += 1;
            continue;
        }
        if let Some((s, end)) = parse_string(content, idx) {
            items.push(s);
            idx = end;
        } else {
            idx += 1;
        }
    }
}

fn skip_ws(bytes: &[u8], mut idx: usize) -> usize {
    while idx < bytes.len() && matches!(bytes[idx], b' ' | b'\t' | b'\n' | b'\r') {
        idx += 1;
    }
    idx
}

fn skip_value(bytes: &[u8], mut idx: usize) -> usize {
    let mut depth = 0;
    while idx < bytes.len() {
        match bytes[idx] {
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                if depth == 0 {
                    return idx;
                }
                depth -= 1;
            }
            b',' if depth == 0 => return idx,
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
    idx
}
