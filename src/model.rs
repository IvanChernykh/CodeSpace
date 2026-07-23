use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    InvalidArgument(String),
    NotInitialized(PathBuf),
    CorruptIndex(String),
    Git(String),
    Protocol(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "I/O error: {error}"),
            Self::InvalidArgument(message) => write!(f, "invalid argument: {message}"),
            Self::NotInitialized(path) => write!(f, "project is not initialized: {}", path.display()),
            Self::CorruptIndex(message) => write!(f, "corrupt index: {message}"),
            Self::Git(message) => write!(f, "git error: {message}"),
            Self::Protocol(message) => write!(f, "protocol error: {message}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Trait,
    Interface,
    Module,
    Constant,
    Variable,
    TypeAlias,
    Test,
    Unknown,
}

impl SymbolKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Method => "method",
            Self::Class => "class",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Trait => "trait",
            Self::Interface => "interface",
            Self::Module => "module",
            Self::Constant => "constant",
            Self::Variable => "variable",
            Self::TypeAlias => "type_alias",
            Self::Test => "test",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "function" | "fn" => Some(Self::Function),
            "method" => Some(Self::Method),
            "class" => Some(Self::Class),
            "struct" => Some(Self::Struct),
            "enum" => Some(Self::Enum),
            "trait" => Some(Self::Trait),
            "interface" => Some(Self::Interface),
            "module" | "mod" => Some(Self::Module),
            "constant" | "const" => Some(Self::Constant),
            "variable" | "var" => Some(Self::Variable),
            "type_alias" | "type" => Some(Self::TypeAlias),
            "test" => Some(Self::Test),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EdgeKind {
    Contains,
    Imports,
    Calls,
    References,
    Inherits,
    Implements,
    Extends,
    TestCovers,
    Configures,
    GeneratedFrom,
    DependsOn,
    RelatedDecision,
}

impl EdgeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Contains => "contains",
            Self::Imports => "imports",
            Self::Calls => "calls",
            Self::References => "references",
            Self::Inherits => "inherits",
            Self::Implements => "implements",
            Self::Extends => "extends",
            Self::TestCovers => "test-covers",
            Self::Configures => "configures",
            Self::GeneratedFrom => "generated-from",
            Self::DependsOn => "depends-on",
            Self::RelatedDecision => "related_decision",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "contains" => Some(Self::Contains),
            "imports" => Some(Self::Imports),
            "calls" => Some(Self::Calls),
            "references" => Some(Self::References),
            "inherits" => Some(Self::Inherits),
            "implements" => Some(Self::Implements),
            "extends" => Some(Self::Extends),
            "test-covers" => Some(Self::TestCovers),
            "configures" => Some(Self::Configures),
            "generated-from" => Some(Self::GeneratedFrom),
            "depends-on" => Some(Self::DependsOn),
            "related_decision" => Some(Self::RelatedDecision),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileRecord {
    pub id: u64,
    pub path: String,
    pub language: String,
    pub hash: u64,
    pub bytes: u64,
    pub modified_unix_ms: u128,
    pub line_count: usize,
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub id: u64,
    pub file_id: u64,
    pub name: String,
    pub qualified_name: String,
    pub kind: SymbolKind,
    pub line_start: usize,
    pub line_end: usize,
    pub signature: String,
    pub doc: String,
    pub complexity: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PrecisionTier {
    Exact,
    Parser,
    Heuristic,
    Inferred,
}

impl PrecisionTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Parser => "parser",
            Self::Heuristic => "heuristic",
            Self::Inferred => "inferred",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "exact" | "compiler" | "lsp" => Some(Self::Exact),
            "parser" => Some(Self::Parser),
            "heuristic" => Some(Self::Heuristic),
            "inferred" => Some(Self::Inferred),
            _ => None,
        }
    }
}

impl Default for PrecisionTier {
    fn default() -> Self {
        Self::Parser
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Edge {
    pub from: u64,
    pub to: u64,
    pub kind: EdgeKind,
    pub confidence_milli: u16,
    pub precision: PrecisionTier,
    pub evidence: String,
}

#[derive(Debug, Clone)]
pub struct Decision {
    pub id: u64,
    pub timestamp_unix_ms: u128,
    pub file: String,
    pub symbol: String,
    pub session: String,
    pub agent: String,
    pub summary: String,
    pub rationale: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GraphIndex {
    pub schema_version: u32,
    pub project_root: String,
    pub created_unix_ms: u128,
    pub updated_unix_ms: u128,
    pub index_revision: u64,
    pub files: BTreeMap<u64, FileRecord>,
    pub files_by_path: BTreeMap<String, u64>,
    pub symbols: BTreeMap<u64, Symbol>,
    pub symbols_by_name: BTreeMap<String, BTreeSet<u64>>,
    pub edges: BTreeSet<Edge>,
    pub decisions: BTreeMap<u64, Decision>,
}

impl GraphIndex {
    pub fn empty(project_root: String, now: u128) -> Self {
        Self {
            schema_version: 2,
            project_root,
            created_unix_ms: now,
            updated_unix_ms: now,
            index_revision: 0,
            files: BTreeMap::new(),
            files_by_path: BTreeMap::new(),
            symbols: BTreeMap::new(),
            symbols_by_name: BTreeMap::new(),
            edges: BTreeSet::new(),
            decisions: BTreeMap::new(),
        }
    }

    pub fn rebuild_indexes(&mut self) {
        self.files_by_path.clear();
        self.symbols_by_name.clear();
        for (id, file) in &self.files {
            self.files_by_path.insert(file.path.clone(), *id);
        }
        for (id, symbol) in &self.symbols {
            self.symbols_by_name
                .entry(symbol.name.to_ascii_lowercase())
                .or_default()
                .insert(*id);
        }
    }

    pub fn remove_file(&mut self, path: &str) {
        let Some(file_id) = self.files_by_path.remove(path) else {
            return;
        };
        self.files.remove(&file_id);
        let symbol_ids: BTreeSet<u64> = self
            .symbols
            .iter()
            .filter_map(|(id, symbol)| (symbol.file_id == file_id).then_some(*id))
            .collect();
        self.symbols.retain(|id, _| !symbol_ids.contains(id));
        self.edges
            .retain(|edge| !symbol_ids.contains(&edge.from) && !symbol_ids.contains(&edge.to));
        self.rebuild_indexes();
    }

    pub fn insert_file(&mut self, file: FileRecord, symbols: Vec<Symbol>, edges: Vec<Edge>) {
        self.remove_file(&file.path);
        let file_id = file.id;
        self.files_by_path.insert(file.path.clone(), file_id);
        self.files.insert(file_id, file);
        for symbol in symbols {
            self.symbols_by_name
                .entry(symbol.name.to_ascii_lowercase())
                .or_default()
                .insert(symbol.id);
            self.symbols.insert(symbol.id, symbol);
        }
        self.edges.extend(edges);
    }

    pub fn file_for_symbol(&self, symbol: &Symbol) -> Option<&FileRecord> {
        self.files.get(&symbol.file_id)
    }

    pub fn outgoing(&self, id: u64) -> impl Iterator<Item = &Edge> {
        self.edges.iter().filter(move |edge| edge.from == id)
    }

    pub fn incoming(&self, id: u64) -> impl Iterator<Item = &Edge> {
        self.edges.iter().filter(move |edge| edge.to == id)
    }
}

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub symbol_id: u64,
    pub score_milli: i64,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ContextItem {
    pub path: String,
    pub language: String,
    pub symbol: String,
    pub kind: SymbolKind,
    pub line_start: usize,
    pub line_end: usize,
    pub score_milli: i64,
    pub content: String,
    pub redactions: usize,
}

#[derive(Debug, Clone)]
pub struct ContextBundle {
    pub query: String,
    pub generated_unix_ms: u128,
    pub estimated_tokens: usize,
    pub source_bytes: usize,
    pub returned_bytes: usize,
    pub items: Vec<ContextItem>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ImpactNode {
    pub symbol_id: u64,
    pub path: String,
    pub symbol: String,
    pub kind: SymbolKind,
    pub depth: usize,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct ImpactReport {
    pub from_ref: String,
    pub to_ref: String,
    pub changed_files: Vec<String>,
    pub changed_symbols: Vec<ImpactNode>,
    pub affected: Vec<ImpactNode>,
    pub risk_score: u8,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct IndexStats {
    pub files_scanned: usize,
    pub files_indexed: usize,
    pub files_skipped_unchanged: usize,
    pub files_removed: usize,
    pub symbols: usize,
    pub edges: usize,
    pub bytes_scanned: u64,
    pub elapsed_ms: u128,
}
