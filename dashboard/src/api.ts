import type {
  GraphSnapshot,
  SearchHit,
  ContextBundle,
  ImpactReport,
  Decision,
  IndexStatsSummary,
  WorkspaceRegistrySnapshot,
  HealthResponse,
  ActionMetaInfo,
  ReadFileResponse,
  DoctorResponse,
  RememberInput,
} from "./types.js";

export class ApiError extends Error {
  readonly status: number;
  constructor(status: number, message: string) {
    super(message);
    this.status = status;
    this.name = "ApiError";
  }
}

/** Typed client for the CodeSpace local REST API. All calls are same-origin. */
export class ApiClient {
  private readonly token: string;
  private readonly base: string;

  constructor(token: string, base = "/api/v1") {
    this.token = token;
    this.base = base;
  }

  /** Build a URL for connecting the native EventSource, which cannot set headers. */
  eventsUrl(): string {
    return `${this.base}/events?token=${encodeURIComponent(this.token)}`;
  }

  private async request<T>(
    path: string,
    init: RequestInit = {},
  ): Promise<T> {
    const headers = new Headers(init.headers);
    headers.set("Authorization", `Bearer ${this.token}`);
    if (init.body) {
      headers.set("Content-Type", "application/json");
    }
    let response: Response;
    try {
      response = await fetch(`${this.base}${path}`, { ...init, headers });
    } catch {
      throw new ApiError(0, "Network error: is the CodeSpace server running?");
    }
    const text = await response.text();
    let parsed: unknown = undefined;
    if (text.length > 0) {
      try {
        parsed = JSON.parse(text);
      } catch {
        throw new ApiError(response.status, "Invalid JSON response from server");
      }
    }
    if (!response.ok) {
      const message =
        parsed && typeof parsed === "object" && parsed !== null && "error" in parsed
          ? String((parsed as { error: unknown }).error)
          : `Request failed with status ${response.status}`;
      throw new ApiError(response.status, message);
    }
    return parsed as T;
  }

  private qs(params: Record<string, string | number | boolean | undefined>): string {
    const search = new URLSearchParams();
    for (const [key, value] of Object.entries(params)) {
      if (value !== undefined && value !== "") {
        search.set(key, String(value));
      }
    }
    const s = search.toString();
    return s.length > 0 ? `?${s}` : "";
  }

  health(): Promise<HealthResponse> {
    return this.request<HealthResponse>("/health");
  }

  listActions(): Promise<{ actions: ActionMetaInfo[] }> {
    return this.request("/actions");
  }

  graph(): Promise<GraphSnapshot> {
    return this.request<GraphSnapshot>("/graph");
  }

  search(query: string, opts: { limit?: number; kind?: string } = {}): Promise<SearchHit[]> {
    return this.request<SearchHit[]>(`/search${this.qs({ q: query, ...opts })}`);
  }

  context(
    query: string,
    opts: { max_tokens?: number; max_items?: number } = {},
  ): Promise<ContextBundle> {
    return this.request<ContextBundle>(`/context${this.qs({ q: query, ...opts })}`);
  }

  stats(): Promise<IndexStatsSummary> {
    return this.request<IndexStatsSummary>("/stats");
  }

  impact(from: string, to: string, depth: number): Promise<ImpactReport> {
    return this.request<ImpactReport>(`/impact${this.qs({ from, to, depth })}`);
  }

  history(query: string, limit = 25): Promise<Decision[]> {
    return this.request<Decision[]>(`/history${this.qs({ q: query, limit })}`);
  }

  remember(input: RememberInput): Promise<{ message: string }> {
    return this.request(`/remember${this.qs({ ...input })}`, { method: "POST" });
  }

  readFile(path: string, maxLines = 400): Promise<ReadFileResponse> {
    return this.request<ReadFileResponse>(`/read${this.qs({ file: path, max_lines: maxLines })}`);
  }

  updateIndex(force = false): Promise<string> {
    return this.request(`/update${this.qs({ force })}`, { method: "POST" });
  }

  doctor(repair = false): Promise<DoctorResponse> {
    return this.request<DoctorResponse>(`/doctor${this.qs({ repair })}`, { method: "POST" });
  }

  workspaces(): Promise<WorkspaceRegistrySnapshot> {
    return this.request<WorkspaceRegistrySnapshot>("/workspaces");
  }

  registerWorkspace(path: string, name?: string): Promise<{ id: string; name: string; path: string }> {
    return this.request(`/workspaces/register${this.qs({ path, name })}`, { method: "POST" });
  }

  selectWorkspace(id: string): Promise<{ status: string }> {
    return this.request(`/workspaces/select${this.qs({ id })}`, { method: "POST" });
  }

  removeWorkspace(id: string): Promise<{ status: string }> {
    return this.request(`/workspaces/remove${this.qs({ id })}`, { method: "POST" });
  }
}
