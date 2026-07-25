import type { GraphSnapshot, SymbolRecord, TabId, WorkspaceRegistrySnapshot } from "./types.js";

export interface AppState {
  graph: GraphSnapshot | null;
  selectedSymbolId: number | null;
  activeTab: TabId;
  workspaces: WorkspaceRegistrySnapshot | null;
  connected: boolean;
  loadingGraph: boolean;
}

type Listener<T> = (value: T, previous: T) => void;

/** Minimal typed store with field-level subscriptions, avoiding any framework dependency. */
export class Store<T extends object> {
  private state: T;
  private readonly listeners = new Set<Listener<T>>();

  constructor(initial: T) {
    this.state = initial;
  }

  get(): Readonly<T> {
    return this.state;
  }

  set(patch: Partial<T>): void {
    const previous = this.state;
    this.state = { ...this.state, ...patch };
    for (const listener of this.listeners) {
      listener(this.state, previous);
    }
  }

  subscribe(listener: Listener<T>): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
}

export function createStore(): Store<AppState> {
  return new Store<AppState>({
    graph: null,
    selectedSymbolId: null,
    activeTab: "graph",
    workspaces: null,
    connected: false,
    loadingGraph: false,
  });
}

export function findSymbol(graph: GraphSnapshot | null, id: number | null): SymbolRecord | undefined {
  if (!graph || id === null) return undefined;
  return graph.symbols.find((s) => s.id === id);
}

export function fileForSymbol(graph: GraphSnapshot | null, symbol: SymbolRecord | undefined) {
  if (!graph || !symbol) return undefined;
  return graph.files.find((f) => f.id === symbol.file_id);
}
