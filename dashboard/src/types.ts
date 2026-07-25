// Types mirroring the JSON shapes produced by the Rust backend
// (src/export.rs, src/model.rs, src/application.rs, src/workspace.rs, src/events.rs).

export type SymbolKind =
  | "function"
  | "method"
  | "class"
  | "struct"
  | "enum"
  | "trait"
  | "interface"
  | "module"
  | "constant"
  | "variable"
  | "type_alias"
  | "test"
  | "unknown";

export type EdgeKind =
  | "contains"
  | "imports"
  | "calls"
  | "references"
  | "inherits"
  | "implements"
  | "extends"
  | "test-covers"
  | "configures"
  | "generated-from"
  | "depends-on"
  | "related_decision";

export type PrecisionTier = "exact" | "parser" | "heuristic" | "inferred";

export interface FileRecord {
  id: number;
  path: string;
  language: string;
  hash: number;
  bytes: number;
  modified_unix_ms: number;
  line_count: number;
}

export interface SymbolRecord {
  id: number;
  file_id: number;
  name: string;
  qualified_name: string;
  kind: SymbolKind;
  line_start: number;
  line_end: number;
  signature: string;
  doc: string;
  complexity: number;
}

export interface EdgeRecord {
  from: number;
  to: number;
  kind: EdgeKind;
  confidence_milli: number;
  precision: PrecisionTier;
  evidence: string;
}

export interface GraphSnapshot {
  schema_version: number;
  project_root: string;
  created_unix_ms: number;
  updated_unix_ms: number;
  index_revision: number;
  files: FileRecord[];
  symbols: SymbolRecord[];
  edges: EdgeRecord[];
}

export interface SearchHit {
  id: number;
  name: string;
  qualified_name: string;
  kind: SymbolKind;
  path: string;
  line_start: number;
  line_end: number;
  score_milli: number;
  reasons: string[];
}

export interface ContextItem {
  path: string;
  language: string;
  symbol: string;
  kind: SymbolKind;
  line_start: number;
  line_end: number;
  score_milli: number;
  redactions: number;
  content: string;
}

export interface ContextBundle {
  query: string;
  generated_unix_ms: number;
  estimated_tokens: number;
  source_bytes: number;
  returned_bytes: number;
  items: ContextItem[];
  warnings: string[];
}

export interface ImpactNode {
  symbol_id: number;
  path: string;
  symbol: string;
  kind: SymbolKind;
  depth: number;
  reason: string;
}

export interface ImpactReport {
  from: string;
  to: string;
  risk_score: number;
  changed_files: string[];
  changed_symbols: ImpactNode[];
  affected: ImpactNode[];
  warnings: string[];
}

export interface Decision {
  id: number;
  timestamp_unix_ms: number;
  file: string;
  symbol: string;
  session: string;
  agent: string;
  summary: string;
  rationale: string;
  tags: string[];
}

export interface IndexStatsSummary {
  files: number;
  symbols: number;
  edges: number;
  decisions: number;
  index_revision: number;
  schema_version: number;
  updated_unix_ms: number;
}

export interface WorkspaceEntry {
  id: string;
  name: string;
  path: string;
  registered_unix_ms: number;
  last_active_unix_ms: number;
  active: boolean;
}

export interface WorkspaceRegistrySnapshot {
  workspaces: WorkspaceEntry[];
  active_id: string | null;
}

export interface HealthResponse {
  status: "ok";
  version: string;
  started_unix_ms: number;
  workspaces: number;
}

export interface ActionMetaInfo {
  name: string;
  description: string;
  category: string;
  read_only: boolean;
}

export interface ReadFileResponse {
  content: string;
}

export interface DoctorResponse {
  messages: string[];
}

export interface ApiErrorBody {
  error: string;
}

export type SseEventType =
  | "index.updated"
  | "index.stale"
  | "decision.added"
  | "workspace.registered"
  | "workspace.removed"
  | "workspace.selected"
  | "settings.changed"
  | "server.started"
  | "server.stopping"
  | "skill.installed"
  | "skill.removed"
  | "mcp.server.started"
  | "mcp.server.stopped";

export interface SseEvent {
  type: SseEventType;
  workspace_id: string;
  state_version: number;
  timestamp_unix_ms: number;
  data: Record<string, string>;
}

export type TabId = "graph" | "context" | "impact" | "history" | "workspaces";

export interface RememberInput {
  file: string;
  symbol: string;
  summary: string;
  rationale: string;
  session: string;
  agent: string;
  tags: string;
}
