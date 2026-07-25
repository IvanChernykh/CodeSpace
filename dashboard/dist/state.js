export class Store {
    constructor(initial) {
        this.listeners = new Set();
        this.state = initial;
    }
    get() {
        return this.state;
    }
    set(patch) {
        const previous = this.state;
        this.state = { ...this.state, ...patch };
        for (const listener of this.listeners) {
            listener(this.state, previous);
        }
    }
    subscribe(listener) {
        this.listeners.add(listener);
        return () => this.listeners.delete(listener);
    }
}
export function createStore() {
    return new Store({
        graph: null,
        selectedSymbolId: null,
        activeTab: "graph",
        workspaces: null,
        connected: false,
        loadingGraph: false,
    });
}
export function findSymbol(graph, id) {
    if (!graph || id === null)
        return undefined;
    return graph.symbols.find((s) => s.id === id);
}
export function fileForSymbol(graph, symbol) {
    if (!graph || !symbol)
        return undefined;
    return graph.files.find((f) => f.id === symbol.file_id);
}
