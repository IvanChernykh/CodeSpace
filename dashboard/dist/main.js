import { ApiClient, ApiError } from "./api.js";
import { createStore, findSymbol } from "./state.js";
import { el, qs, qsa } from "./dom.js";
import { ToastManager } from "./toast.js";
import { GraphView } from "./graph-view.js";
import { Sidebar } from "./sidebar.js";
import { Inspector } from "./inspector.js";
import { ContextPanel } from "./panels/context-panel.js";
import { ImpactPanel } from "./panels/impact-panel.js";
import { HistoryPanel } from "./panels/history-panel.js";
import { WorkspacesPanel } from "./panels/workspaces-panel.js";
import { CommandPalette } from "./command-palette.js";
import { LiveEvents } from "./sse.js";
const TABS = ["graph", "context", "impact", "history", "workspaces"];
function bootstrap() {
    const token = window.__CSE_TOKEN__ ?? "";
    const api = new ApiClient(token);
    const toast = new ToastManager(qs("#toastRoot"));
    const store = createStore();
    const statusDot = qs("#statusDot");
    const statusText = qs("#statusText");
    const updateButton = qs("#updateIndexBtn");
    const doctorButton = qs("#doctorBtn");
    const paletteTrigger = qs("#paletteTrigger");
    const workspaceSelect = qs("#workspaceSelect");
    const graphSvg = qs("#graphSvg");
    const graphEmptyState = qs("#graphEmptyState");
    const sidebarRoot = qs("#sidebar");
    const inspectorRoot = qs("#inspector");
    const contextRoot = qs("#panel-context");
    const impactRoot = qs("#panel-impact");
    const historyRoot = qs("#panel-history");
    const workspacesRoot = qs("#panel-workspaces");
    const paletteOverlay = qs("#commandPalette");
    function selectSymbol(id) {
        store.set({ selectedSymbolId: id });
        const graph = store.get().graph;
        sidebar.setSelected(id);
        graphView.selectNode(id);
        if (id === null) {
            inspector.renderEmpty();
            return;
        }
        const symbol = findSymbol(graph, id);
        if (symbol && graph) {
            inspector.render(symbol, graph);
        }
    }
    const graphView = new GraphView(graphSvg, {
        onSelectNode: (id) => selectSymbol(id),
        maxNodes: 160,
    });
    const sidebar = new Sidebar(sidebarRoot, {
        onSelect: (id) => {
            selectSymbol(id);
            graphView.focus(id);
            switchTab("graph");
        },
    });
    const inspector = new Inspector(inspectorRoot, {
        api,
        toast,
        onFocusSymbol: (id) => {
            selectSymbol(id);
            graphView.focus(id);
            switchTab("graph");
        },
    });
    const contextPanel = new ContextPanel(contextRoot, api, toast);
    const impactPanel = new ImpactPanel(impactRoot, api, (id) => {
        selectSymbol(id);
        graphView.focus(id);
        switchTab("graph");
    });
    const historyPanel = new HistoryPanel(historyRoot, api, toast);
    const workspacesPanel = new WorkspacesPanel(workspacesRoot, api, toast, {
        onChanged: (snapshot) => refreshWorkspaceSelect(snapshot),
    });
    function refreshWorkspaceSelect(snapshot) {
        store.set({ workspaces: snapshot });
        workspaceSelect.replaceChildren(...snapshot.workspaces.map((ws) => el("option", { value: ws.id, selected: ws.active }, [ws.name])));
        if (snapshot.workspaces.length === 0) {
            workspaceSelect.replaceChildren(el("option", { value: "" }, ["No workspaces registered"]));
        }
    }
    workspaceSelect.addEventListener("change", async () => {
        const id = workspaceSelect.value;
        if (!id)
            return;
        try {
            await api.selectWorkspace(id);
            toast.success("Workspace switched");
            await workspacesPanel.load();
            await loadGraph();
        }
        catch (error) {
            toast.error(error instanceof ApiError ? error.message : "Failed to switch workspace");
        }
    });
    function switchTab(tab) {
        store.set({ activeTab: tab });
        for (const nav of qsa("[data-tab]")) {
            nav.classList.toggle("is-active", nav.dataset.tab === tab);
        }
        for (const panel of qsa("[data-panel]")) {
            panel.classList.toggle("is-active", panel.dataset.panel === tab);
        }
    }
    for (const nav of qsa("[data-tab]")) {
        nav.addEventListener("click", () => {
            const tab = nav.dataset.tab;
            switchTab(tab);
            if (tab === "workspaces")
                void workspacesPanel.load();
        });
    }
    async function loadGraph() {
        store.set({ loadingGraph: true });
        graphEmptyState.style.display = "flex";
        graphEmptyState.textContent = "Loading graph\u2026";
        try {
            const graph = await api.graph();
            store.set({ graph, loadingGraph: false });
            if (graph.symbols.length === 0) {
                graphEmptyState.style.display = "flex";
                graphEmptyState.textContent = "Index is empty. Run cse init in this project, then Update Index.";
                graphSvg.style.display = "none";
                sidebar.setGraph(graph);
                return;
            }
            graphEmptyState.style.display = "none";
            graphSvg.style.display = "block";
            graphView.setGraph(graph);
            sidebar.setGraph(graph);
        }
        catch (error) {
            store.set({ loadingGraph: false });
            graphEmptyState.style.display = "flex";
            graphEmptyState.textContent =
                error instanceof ApiError ? `Failed to load graph: ${error.message}` : "Failed to load graph";
            toast.error(error instanceof ApiError ? error.message : "Failed to load graph");
        }
    }
    updateButton.addEventListener("click", async () => {
        updateButton.disabled = true;
        updateButton.textContent = "Updating\u2026";
        try {
            await api.updateIndex();
            toast.success("Index updated");
            await loadGraph();
        }
        catch (error) {
            toast.error(error instanceof ApiError ? error.message : "Failed to update index");
        }
        finally {
            updateButton.disabled = false;
            updateButton.textContent = "Update Index";
        }
    });
    doctorButton.addEventListener("click", async () => {
        doctorButton.disabled = true;
        try {
            const result = await api.doctor(false);
            for (const message of result.messages)
                toast.info(message);
            if (result.messages.length === 0)
                toast.success("Everything looks healthy");
        }
        catch (error) {
            toast.error(error instanceof ApiError ? error.message : "Doctor check failed");
        }
        finally {
            doctorButton.disabled = false;
        }
    });
    for (const checkbox of qsa("[data-edge-filter]")) {
        checkbox.addEventListener("change", () => {
            const kind = checkbox.dataset.edgeFilter;
            if (kind)
                graphView.setEdgeFilter(kind, checkbox.checked);
        });
    }
    const palette = new CommandPalette(paletteOverlay, {
        api,
        onSelectSymbol: (id) => {
            selectSymbol(id);
            graphView.focus(id);
            switchTab("graph");
        },
        onSwitchTab: switchTab,
        commands: [
            { id: "tab-graph", label: "Go to Graph", hint: "1", run: () => switchTab("graph") },
            { id: "tab-context", label: "Go to Context", hint: "2", run: () => switchTab("context") },
            { id: "tab-impact", label: "Go to Impact", hint: "3", run: () => switchTab("impact") },
            { id: "tab-history", label: "Go to History", hint: "4", run: () => switchTab("history") },
            { id: "tab-workspaces", label: "Go to Workspaces", hint: "5", run: () => switchTab("workspaces") },
            { id: "update-index", label: "Update Index", run: () => updateButton.click() },
            { id: "run-doctor", label: "Run Doctor Check", run: () => doctorButton.click() },
        ],
    });
    paletteTrigger.addEventListener("click", () => palette.open());
    document.addEventListener("keydown", (ev) => {
        const target = ev.target;
        const typing = target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA");
        if ((ev.metaKey || ev.ctrlKey) && ev.key.toLowerCase() === "k") {
            ev.preventDefault();
            palette.toggle();
            return;
        }
        if (palette.isOpen())
            return;
        if (typing)
            return;
        if (ev.key === "/") {
            ev.preventDefault();
            switchTab("graph");
            qs("[data-sidebar-filter]", sidebarRoot).focus();
            return;
        }
        const index = TABS.indexOf(ev.key);
        if (ev.key >= "1" && ev.key <= "5") {
            const tab = TABS[Number(ev.key) - 1];
            if (tab)
                switchTab(tab);
        }
        void index;
        if (ev.key === "Escape") {
            selectSymbol(null);
        }
    });
    const liveEvents = new LiveEvents();
    liveEvents.onStatusChange((connected) => {
        store.set({ connected });
        statusDot.classList.toggle("is-online", connected);
        statusDot.classList.toggle("is-offline", !connected);
        statusText.textContent = connected ? "Live" : "Reconnecting\u2026";
    });
    liveEvents.onEvent((event) => {
        switch (event.type) {
            case "index.updated":
                toast.info("Index updated in the background");
                void loadGraph();
                break;
            case "workspace.registered":
            case "workspace.removed":
            case "workspace.selected":
                void workspacesPanel.load();
                break;
            case "decision.added":
                if (store.get().activeTab === "history")
                    void historyPanel.run();
                break;
            default:
                break;
        }
    });
    liveEvents.connect(api.eventsUrl());
    window.addEventListener("resize", () => {
        const graph = store.get().graph;
        if (graph)
            graphView.setGraph(graph);
    });
    void (async () => {
        try {
            const health = await api.health();
            statusText.textContent = "Live";
            void health;
        }
        catch {
            toast.error("Unable to reach the CodeSpace server");
        }
        await Promise.all([loadGraph(), workspacesPanel.load()]);
    })();
    void contextPanel;
    void impactPanel;
}
if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", bootstrap);
}
else {
    bootstrap();
}
