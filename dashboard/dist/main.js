"use strict";
const $ = (selector, root = document) => {
    const node = root.querySelector(selector);
    if (!node)
        throw new Error(`Missing required element: ${selector}`);
    return node;
};
const $$ = (selector, root = document) => Array.from(root.querySelectorAll(selector));
const text = (value) => (value === null || value === undefined ? "" : String(value));
const escapeHtml = (value) => text(value).replace(/[&<>'"]/g, (char) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", "'": "&#39;", '"': "&quot;" })[char] ?? char);
const compactNumber = (value) => new Intl.NumberFormat(undefined, { notation: "compact", maximumFractionDigits: 1 }).format(value);
const formatBytes = (bytes) => {
    if (bytes < 1024)
        return `${bytes} B`;
    const units = ["KB", "MB", "GB"];
    let current = bytes / 1024;
    let index = 0;
    while (current >= 1024 && index < units.length - 1) {
        current /= 1024;
        index += 1;
    }
    return `${current.toFixed(current >= 10 ? 0 : 1)} ${units[index] ?? "KB"}`;
};
const formatDate = (unixMs) => {
    if (!unixMs)
        return "—";
    try {
        return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(new Date(unixMs));
    }
    catch {
        return "—";
    }
};
const hash = (value) => {
    let result = 2166136261;
    for (let index = 0; index < value.length; index += 1) {
        result ^= value.charCodeAt(index);
        result = Math.imul(result, 16777619);
    }
    return result >>> 0;
};
class ApiError extends Error {
    constructor(status, message) {
        super(message);
        this.status = status;
        this.name = "ApiError";
    }
}
class ApiClient {
    constructor() {
        this.health = () => this.request("/health");
        this.stats = () => this.request("/stats");
        this.graph = () => this.request("/graph");
        this.workspaces = () => this.request("/workspaces");
        this.selectWorkspace = (id) => this.request(`/workspaces/select${this.query({ id })}`, { method: "POST" });
        this.registerWorkspace = (path, name) => this.request(`/workspaces/register${this.query({ path, name })}`, { method: "POST" });
        this.removeWorkspace = (id) => this.request(`/workspaces/remove${this.query({ id })}`, { method: "POST" });
        this.updateIndex = (force = false) => this.request(`/update${this.query({ force })}`, { method: "POST" });
        this.doctor = () => this.request("/doctor", { method: "POST" });
        this.readFile = (path, maxLines = 500) => this.request(`/read${this.query({ file: path, max_lines: maxLines })}`);
        this.context = (query, maxTokens, maxItems) => this.request(`/context${this.query({ q: query, max_tokens: maxTokens, max_items: maxItems })}`);
        this.impact = (from, to, depth) => this.request(`/impact${this.query({ from, to, depth })}`);
        this.history = (query, limit = 40) => this.request(`/history${this.query({ q: query, limit })}`);
        this.remember = (payload) => this.request(`/remember${this.query(payload)}`, { method: "POST" });
        this.aiChat = (query, model) => this.request("/ai/chat", { method: "POST", body: JSON.stringify({ query, model }) });
        this.tasks = () => this.request("/tasks");
        this.addTask = (payload) => this.request("/tasks", { method: "POST", body: JSON.stringify(payload) });
        this.setTaskStatus = (id, status) => this.request("/tasks/status", { method: "POST", body: JSON.stringify({ id, status }) });
        this.removeTask = (id) => this.request("/tasks/remove", { method: "POST", body: JSON.stringify({ id }) });
        this.skills = () => this.request("/skills");
        this.toggleSkill = (id, enabled) => this.request("/skills/toggle", { method: "POST", body: JSON.stringify({ id, enabled }) });
        this.mcp = () => this.request("/mcp");
        this.registerMcp = (payload) => this.request("/mcp/register", { method: "POST", body: JSON.stringify(payload) });
        this.mcpAction = (action, id) => this.request(`/mcp/${action}${this.query({ id })}`, { method: "POST" });
        this.settings = () => this.request("/settings");
        this.setSetting = (key, value, scope) => this.request("/settings", { method: "POST", body: JSON.stringify({ key, value, scope }) });
        this.githubStatus = () => this.request("/github/status");
        this.eventsUrl = () => `/api/v1/events${this.query({ token: this.token })}`;
        const meta = document.querySelector('meta[name="cse-session"]');
        this.token = meta?.content || (window.__CSE_BOOTSTRAP__?.token ?? "");
    }
    query(values) {
        const params = new URLSearchParams();
        Object.entries(values).forEach(([key, value]) => {
            if (value !== undefined && value !== "")
                params.set(key, String(value));
        });
        const serialized = params.toString();
        return serialized ? `?${serialized}` : "";
    }
    async request(path, init = {}) {
        const headers = new Headers(init.headers);
        if (this.token)
            headers.set("Authorization", `Bearer ${this.token}`);
        if (init.body)
            headers.set("Content-Type", "application/json");
        let response;
        try {
            response = await fetch(`/api/v1${path}`, { ...init, headers, cache: "no-store" });
        }
        catch {
            throw new ApiError(0, "CodeSpace server is unreachable");
        }
        const raw = await response.text();
        let body = null;
        if (raw) {
            try {
                body = JSON.parse(raw);
            }
            catch {
                body = raw;
            }
        }
        if (!response.ok) {
            const message = typeof body === "object" && body !== null && "error" in body
                ? text(body.error)
                : typeof body === "string" && body
                    ? body
                    : `Request failed (${response.status})`;
            throw new ApiError(response.status, message);
        }
        return body;
    }
}
class Toasts {
    constructor() {
        this.root = $("#toastRoot");
        this.success = (message) => this.show(message, "success");
        this.error = (message) => this.show(message, "error");
        this.info = (message) => this.show(message, "info");
    }
    show(message, tone = "info") {
        const item = document.createElement("div");
        item.className = `toast toast-${tone}`;
        item.innerHTML = `<span class="toast-icon">${tone === "success" ? "✓" : tone === "error" ? "!" : "·"}</span><span>${escapeHtml(message)}</span>`;
        this.root.append(item);
        window.setTimeout(() => item.classList.add("is-visible"), 10);
        window.setTimeout(() => {
            item.classList.remove("is-visible");
            window.setTimeout(() => item.remove(), 180);
        }, 4200);
    }
}
class ConnectionController {
    constructor(api, onEvent) {
        this.api = api;
        this.onEvent = onEvent;
        this.state = "starting";
        this.source = null;
        this.pollHandle = null;
        this.lastEventAt = 0;
    }
    start() {
        this.setState("starting", "Starting local runtime");
        this.connectEvents();
        void this.pollHealth();
        this.pollHandle = window.setInterval(() => void this.pollHealth(), 5000);
    }
    setState(state, message) {
        this.state = state;
        const chip = $("#connectionChip");
        chip.dataset.state = state;
        $("#connectionLabel").textContent = state === "online" ? "Live" : state === "degraded" ? "Degraded" : state === "offline" ? "Offline" : "Starting";
        $("#connectionDetail").textContent = message;
    }
    async pollHealth() {
        try {
            const health = await this.api.health();
            $("#runtimeVersion").textContent = `v${health.version}`;
            const liveEvents = this.source?.readyState === EventSource.OPEN;
            if (liveEvents) {
                this.setState("online", "REST and live events connected");
            }
            else {
                this.setState("degraded", "REST connected; live events reconnecting");
            }
        }
        catch {
            this.setState("offline", "Local server is unreachable");
        }
    }
    connectEvents() {
        this.source?.close();
        this.source = new EventSource(this.api.eventsUrl());
        this.source.onopen = () => {
            this.lastEventAt = Date.now();
            this.setState("online", "REST and live events connected");
        };
        this.source.onmessage = (message) => {
            this.lastEventAt = Date.now();
            try {
                const event = JSON.parse(message.data);
                this.onEvent(event);
            }
            catch {
            }
        };
        this.source.onerror = () => {
            if (this.state !== "offline")
                this.setState("degraded", "REST connected; live events reconnecting");
        };
    }
    stop() {
        this.source?.close();
        if (this.pollHandle !== null)
            window.clearInterval(this.pollHandle);
    }
}
class FileGraphView {
    constructor(onSelect) {
        this.onSelect = onSelect;
        this.svg = $("#fileGraphSvg");
        this.nodes = [];
        this.edges = [];
        this.graph = null;
        this.scale = 1;
        this.tx = 0;
        this.ty = 0;
        this.pan = null;
        this.dragged = null;
        this.selected = null;
        this.filter = "";
        const ns = "http://www.w3.org/2000/svg";
        this.viewport = document.createElementNS(ns, "g");
        this.edgeLayer = document.createElementNS(ns, "g");
        this.nodeLayer = document.createElementNS(ns, "g");
        this.edgeLayer.setAttribute("class", "file-edge-layer");
        this.nodeLayer.setAttribute("class", "file-node-layer");
        this.viewport.append(this.edgeLayer, this.nodeLayer);
        this.svg.append(this.viewport);
        this.svg.addEventListener("wheel", (event) => this.zoom(event), { passive: false });
        this.svg.addEventListener("pointerdown", (event) => this.pointerDown(event));
        window.addEventListener("pointermove", (event) => this.pointerMove(event));
        window.addEventListener("pointerup", () => this.pointerUp());
        this.svg.addEventListener("dblclick", () => this.fit());
    }
    setGraph(graph) {
        this.graph = graph;
        this.build();
    }
    setFilter(value) {
        this.filter = value.trim().toLowerCase();
        this.applyFilter();
    }
    build() {
        if (!this.graph)
            return;
        const graph = this.graph;
        this.edgeLayer.replaceChildren();
        this.nodeLayer.replaceChildren();
        this.nodes = [];
        this.edges = [];
        const symbolFile = new Map();
        const symbolCount = new Map();
        for (const symbol of graph.symbols) {
            symbolFile.set(symbol.id, symbol.file_id);
            symbolCount.set(symbol.file_id, (symbolCount.get(symbol.file_id) ?? 0) + 1);
        }
        const aggregates = new Map();
        const degree = new Map();
        for (const edge of graph.edges) {
            const fromFile = symbolFile.get(edge.from);
            const toFile = symbolFile.get(edge.to);
            if (fromFile === undefined || toFile === undefined || fromFile === toFile)
                continue;
            const low = Math.min(fromFile, toFile);
            const high = Math.max(fromFile, toFile);
            const key = `${low}:${high}`;
            const aggregate = aggregates.get(key) ?? { from: low, to: high, weight: 0, kinds: new Set() };
            aggregate.weight += 1;
            aggregate.kinds.add(edge.kind);
            aggregates.set(key, aggregate);
            degree.set(fromFile, (degree.get(fromFile) ?? 0) + 1);
            degree.set(toFile, (degree.get(toFile) ?? 0) + 1);
        }
        const maxFiles = 260;
        const selectedFiles = [...graph.files]
            .sort((a, b) => (degree.get(b.id) ?? 0) - (degree.get(a.id) ?? 0) || b.line_count - a.line_count)
            .slice(0, maxFiles);
        const selectedIds = new Set(selectedFiles.map((file) => file.id));
        const groups = new Map();
        for (const file of selectedFiles) {
            const group = file.path.includes("/") ? file.path.split("/")[0] ?? "root" : "root";
            const bucket = groups.get(group) ?? [];
            bucket.push(file);
            groups.set(group, bucket);
        }
        const rect = this.svg.getBoundingClientRect();
        const width = Math.max(rect.width, 900);
        const height = Math.max(rect.height, 620);
        const groupList = [...groups.entries()].sort((a, b) => b[1].length - a[1].length);
        groupList.forEach(([groupName, files], groupIndex) => {
            const angle = (groupIndex / Math.max(groupList.length, 1)) * Math.PI * 2 - Math.PI / 2;
            const clusterRadius = Math.min(width, height) * (groupList.length === 1 ? 0 : 0.28);
            const centerX = width / 2 + Math.cos(angle) * clusterRadius;
            const centerY = height / 2 + Math.sin(angle) * clusterRadius;
            files.forEach((file, fileIndex) => {
                const localAngle = (fileIndex / Math.max(files.length, 1)) * Math.PI * 2 + (hash(groupName) % 100) / 100;
                const ring = 70 + Math.floor(fileIndex / 10) * 68;
                const x = centerX + Math.cos(localAngle) * ring;
                const y = centerY + Math.sin(localAngle) * ring;
                const symbols = symbolCount.get(file.id) ?? 0;
                const nodeWidth = Math.min(220, Math.max(120, 96 + file.path.length * 2.4));
                const node = this.createNode(file, x, y, nodeWidth, 48, degree.get(file.id) ?? 0, symbols);
                this.nodes.push(node);
            });
        });
        const byId = new Map(this.nodes.map((node) => [node.id, node]));
        aggregates.forEach((aggregate) => {
            if (!selectedIds.has(aggregate.from) || !selectedIds.has(aggregate.to))
                return;
            const source = byId.get(aggregate.from);
            const target = byId.get(aggregate.to);
            if (!source || !target)
                return;
            const line = document.createElementNS("http://www.w3.org/2000/svg", "path");
            line.setAttribute("class", "file-edge");
            line.style.setProperty("--edge-weight", String(Math.min(aggregate.weight, 8)));
            this.edgeLayer.append(line);
            this.edges.push({ source, target, weight: aggregate.weight, kinds: aggregate.kinds, line });
        });
        this.relaxLayout(90, width, height);
        this.render();
        this.fit();
        this.applyFilter();
        $("#graphVisibleCount").textContent = `${this.nodes.length} files`;
        $("#graphEdgeCount").textContent = `${this.edges.length} links`;
    }
    createNode(file, x, y, width, height, degree, symbolCount) {
        const ns = "http://www.w3.org/2000/svg";
        const group = document.createElementNS(ns, "g");
        group.setAttribute("class", "file-node");
        group.dataset.id = String(file.id);
        const rect = document.createElementNS(ns, "rect");
        rect.setAttribute("x", String(-width / 2));
        rect.setAttribute("y", String(-height / 2));
        rect.setAttribute("width", String(width));
        rect.setAttribute("height", String(height));
        rect.setAttribute("rx", "10");
        const indicator = document.createElementNS(ns, "rect");
        indicator.setAttribute("x", String(-width / 2));
        indicator.setAttribute("y", String(-height / 2));
        indicator.setAttribute("width", "4");
        indicator.setAttribute("height", String(height));
        indicator.setAttribute("rx", "2");
        indicator.setAttribute("class", `language-indicator lang-${file.language.toLowerCase().replace(/[^a-z0-9]+/g, "-")}`);
        const name = document.createElementNS(ns, "text");
        name.setAttribute("x", String(-width / 2 + 14));
        name.setAttribute("y", "-4");
        name.setAttribute("class", "file-node-name");
        const fileName = file.path.split("/").pop() ?? file.path;
        name.textContent = fileName.length > 28 ? `${fileName.slice(0, 25)}…` : fileName;
        const meta = document.createElementNS(ns, "text");
        meta.setAttribute("x", String(-width / 2 + 14));
        meta.setAttribute("y", "14");
        meta.setAttribute("class", "file-node-meta");
        meta.textContent = `${file.language || "text"} · ${symbolCount} symbols · ${file.line_count} lines`;
        const title = document.createElementNS(ns, "title");
        title.textContent = file.path;
        group.append(rect, indicator, name, meta, title);
        const node = { id: file.id, file, x, y, width, height, degree, symbolCount, group };
        group.addEventListener("pointerdown", (event) => {
            event.stopPropagation();
            this.dragged = node;
            group.setPointerCapture?.(event.pointerId);
        });
        group.addEventListener("click", (event) => {
            event.stopPropagation();
            this.selected = node.id;
            this.nodes.forEach((candidate) => candidate.group.classList.toggle("is-selected", candidate.id === node.id));
            this.onSelect(node.file);
            this.focus(node);
        });
        this.nodeLayer.append(group);
        return node;
    }
    relaxLayout(iterations, width, height) {
        const nodeById = new Map(this.nodes.map((node) => [node.id, node]));
        for (let iteration = 0; iteration < iterations; iteration += 1) {
            const velocity = new Map();
            this.nodes.forEach((node) => velocity.set(node.id, { x: (width / 2 - node.x) * 0.0018, y: (height / 2 - node.y) * 0.0018 }));
            for (let leftIndex = 0; leftIndex < this.nodes.length; leftIndex += 1) {
                const left = this.nodes[leftIndex];
                if (!left)
                    continue;
                for (let rightIndex = leftIndex + 1; rightIndex < this.nodes.length; rightIndex += 1) {
                    const right = this.nodes[rightIndex];
                    if (!right)
                        continue;
                    const dx = left.x - right.x;
                    const dy = left.y - right.y;
                    const distanceSquared = Math.max(dx * dx + dy * dy, 900);
                    const distance = Math.sqrt(distanceSquared);
                    const force = 2400 / distanceSquared;
                    const leftVelocity = velocity.get(left.id);
                    const rightVelocity = velocity.get(right.id);
                    if (leftVelocity && rightVelocity) {
                        leftVelocity.x += (dx / distance) * force;
                        leftVelocity.y += (dy / distance) * force;
                        rightVelocity.x -= (dx / distance) * force;
                        rightVelocity.y -= (dy / distance) * force;
                    }
                }
            }
            this.edges.forEach((edge) => {
                const source = nodeById.get(edge.source.id);
                const target = nodeById.get(edge.target.id);
                if (!source || !target)
                    return;
                const dx = target.x - source.x;
                const dy = target.y - source.y;
                const distance = Math.max(Math.sqrt(dx * dx + dy * dy), 1);
                const ideal = 180 + Math.min(edge.weight, 6) * 8;
                const force = (distance - ideal) * 0.004;
                const sourceVelocity = velocity.get(source.id);
                const targetVelocity = velocity.get(target.id);
                if (sourceVelocity && targetVelocity) {
                    sourceVelocity.x += (dx / distance) * force;
                    sourceVelocity.y += (dy / distance) * force;
                    targetVelocity.x -= (dx / distance) * force;
                    targetVelocity.y -= (dy / distance) * force;
                }
            });
            this.nodes.forEach((node) => {
                const current = velocity.get(node.id);
                if (!current)
                    return;
                node.x += Math.max(-8, Math.min(8, current.x));
                node.y += Math.max(-8, Math.min(8, current.y));
            });
        }
    }
    render() {
        this.nodes.forEach((node) => node.group.setAttribute("transform", `translate(${node.x.toFixed(1)} ${node.y.toFixed(1)})`));
        this.edges.forEach((edge) => {
            const dx = edge.target.x - edge.source.x;
            const dy = edge.target.y - edge.source.y;
            const curve = Math.min(80, Math.sqrt(dx * dx + dy * dy) * 0.12);
            const mx = (edge.source.x + edge.target.x) / 2 - (dy / Math.max(Math.sqrt(dx * dx + dy * dy), 1)) * curve;
            const my = (edge.source.y + edge.target.y) / 2 + (dx / Math.max(Math.sqrt(dx * dx + dy * dy), 1)) * curve;
            edge.line.setAttribute("d", `M ${edge.source.x.toFixed(1)} ${edge.source.y.toFixed(1)} Q ${mx.toFixed(1)} ${my.toFixed(1)} ${edge.target.x.toFixed(1)} ${edge.target.y.toFixed(1)}`);
        });
        this.applyTransform();
    }
    applyFilter() {
        const visible = new Set();
        this.nodes.forEach((node) => {
            const matches = !this.filter || node.file.path.toLowerCase().includes(this.filter) || node.file.language.toLowerCase().includes(this.filter);
            node.group.classList.toggle("is-filtered", !matches);
            if (matches)
                visible.add(node.id);
        });
        this.edges.forEach((edge) => edge.line.classList.toggle("is-filtered", !visible.has(edge.source.id) || !visible.has(edge.target.id)));
    }
    fit() {
        if (this.nodes.length === 0)
            return;
        const bounds = this.nodes.reduce((result, node) => ({
            minX: Math.min(result.minX, node.x - node.width / 2),
            maxX: Math.max(result.maxX, node.x + node.width / 2),
            minY: Math.min(result.minY, node.y - node.height / 2),
            maxY: Math.max(result.maxY, node.y + node.height / 2),
        }), { minX: Infinity, maxX: -Infinity, minY: Infinity, maxY: -Infinity });
        const rect = this.svg.getBoundingClientRect();
        const contentWidth = Math.max(bounds.maxX - bounds.minX, 1);
        const contentHeight = Math.max(bounds.maxY - bounds.minY, 1);
        this.scale = Math.max(0.22, Math.min(1.1, Math.min((rect.width - 80) / contentWidth, (rect.height - 80) / contentHeight)));
        this.tx = rect.width / 2 - ((bounds.minX + bounds.maxX) / 2) * this.scale;
        this.ty = rect.height / 2 - ((bounds.minY + bounds.maxY) / 2) * this.scale;
        this.applyTransform();
    }
    focus(node) {
        const rect = this.svg.getBoundingClientRect();
        this.scale = Math.max(this.scale, 0.85);
        this.tx = rect.width / 2 - node.x * this.scale;
        this.ty = rect.height / 2 - node.y * this.scale;
        this.applyTransform();
    }
    zoom(event) {
        event.preventDefault();
        const rect = this.svg.getBoundingClientRect();
        const pointerX = event.clientX - rect.left;
        const pointerY = event.clientY - rect.top;
        const worldX = (pointerX - this.tx) / this.scale;
        const worldY = (pointerY - this.ty) / this.scale;
        const nextScale = Math.max(0.18, Math.min(2.4, this.scale * Math.exp(-event.deltaY * 0.0012)));
        this.tx = pointerX - worldX * nextScale;
        this.ty = pointerY - worldY * nextScale;
        this.scale = nextScale;
        this.applyTransform();
    }
    pointerDown(event) {
        if (event.button !== 0)
            return;
        this.pan = { x: event.clientX, y: event.clientY, tx: this.tx, ty: this.ty };
        this.svg.classList.add("is-panning");
    }
    pointerMove(event) {
        if (this.dragged) {
            const rect = this.svg.getBoundingClientRect();
            this.dragged.x = (event.clientX - rect.left - this.tx) / this.scale;
            this.dragged.y = (event.clientY - rect.top - this.ty) / this.scale;
            this.render();
            return;
        }
        if (!this.pan)
            return;
        this.tx = this.pan.tx + event.clientX - this.pan.x;
        this.ty = this.pan.ty + event.clientY - this.pan.y;
        this.applyTransform();
    }
    pointerUp() {
        this.pan = null;
        this.dragged = null;
        this.svg.classList.remove("is-panning");
    }
    applyTransform() {
        this.viewport.setAttribute("transform", `translate(${this.tx.toFixed(1)} ${this.ty.toFixed(1)}) scale(${this.scale.toFixed(3)})`);
    }
}
class CodeSpaceApp {
    constructor() {
        this.api = new ApiClient();
        this.toasts = new Toasts();
        this.connection = new ConnectionController(this.api, (event) => this.handleEvent(event));
        this.graphView = new FileGraphView((file) => void this.selectFile(file));
        this.activeTab = "overview";
        this.graph = null;
        this.stats = null;
        this.workspaces = null;
    }
    start() {
        this.bindNavigation();
        this.bindGlobalActions();
        this.bindPanels();
        this.connection.start();
        void this.bootstrap();
    }
    async bootstrap() {
        this.setBusy("Loading workspace intelligence…");
        const results = await Promise.allSettled([this.api.health(), this.api.stats(), this.api.workspaces(), this.api.graph()]);
        const health = results[0].status === "fulfilled" ? results[0].value : null;
        this.stats = results[1].status === "fulfilled" ? results[1].value : null;
        this.workspaces = results[2].status === "fulfilled" ? results[2].value : null;
        this.graph = results[3].status === "fulfilled" ? results[3].value : null;
        if (health)
            $("#runtimeVersion").textContent = `v${health.version}`;
        this.renderOverview();
        this.renderWorkspaceSwitcher();
        if (this.graph)
            this.renderGraph(this.graph);
        else
            this.renderGraphError("Index is unavailable. Initialize or update the selected repository.");
        this.clearBusy();
    }
    bindNavigation() {
        $$("[data-nav]").forEach((button) => button.addEventListener("click", () => this.switchTab(button.dataset.nav)));
        document.addEventListener("keydown", (event) => {
            const target = event.target;
            const typing = target?.tagName === "INPUT" || target?.tagName === "TEXTAREA" || target?.tagName === "SELECT";
            if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
                event.preventDefault();
                $("#globalSearch").focus();
                return;
            }
            if (typing)
                return;
            const shortcuts = { "1": "overview", "2": "graph", "3": "context", "4": "assistant", "5": "tasks" };
            const tab = shortcuts[event.key];
            if (tab)
                this.switchTab(tab);
        });
    }
    switchTab(tab) {
        this.activeTab = tab;
        $$("[data-nav]").forEach((node) => node.classList.toggle("is-active", node.dataset.nav === tab));
        $$("[data-view]").forEach((node) => node.classList.toggle("is-active", node.dataset.view === tab));
        const labels = {
            overview: ["Command Center", "Runtime, index, and repository health"],
            graph: ["Repository Map", "File-level dependency network"],
            context: ["Context Builder", "Build precise, token-bounded context"],
            impact: ["Impact Analysis", "Trace change propagation before editing"],
            assistant: ["IDE Assistant", "Local model with repository context"],
            tasks: ["Engineering Tasks", "Repository-native execution board"],
            history: ["Decision Memory", "Search and capture technical decisions"],
            workspaces: ["Repositories", "Manage local working repositories"],
            skills: ["Skills", "Extend CodeSpace with controlled capabilities"],
            mcp: ["MCP Control", "Manage local Model Context Protocol servers"],
            settings: ["Settings", "Global and repository-scoped configuration"],
            github: ["GitHub", "Repository connection and delivery state"],
        };
        const [title, subtitle] = labels[tab];
        $("#pageTitle").textContent = title;
        $("#pageSubtitle").textContent = subtitle;
        if (tab === "tasks")
            void this.loadTasks();
        if (tab === "history")
            void this.loadHistory();
        if (tab === "workspaces")
            void this.loadWorkspaces();
        if (tab === "skills")
            void this.loadSkills();
        if (tab === "mcp")
            void this.loadMcp();
        if (tab === "settings")
            void this.loadSettings();
        if (tab === "github")
            void this.loadGithub();
        if (tab === "graph")
            window.setTimeout(() => this.graphView.fit(), 30);
    }
    bindGlobalActions() {
        $("#updateIndexBtn").addEventListener("click", () => void this.updateIndex());
        $("#doctorBtn").addEventListener("click", () => void this.runDoctor());
        $("#workspaceSelect").addEventListener("change", (event) => void this.selectWorkspace(event.target.value));
        $("#globalSearch").addEventListener("keydown", (event) => {
            if (event.key !== "Enter")
                return;
            const value = event.target.value.trim();
            if (!value)
                return;
            this.switchTab("graph");
            $("#graphFilter").setAttribute("value", value);
            const filter = $("#graphFilter");
            filter.value = value;
            this.graphView.setFilter(value);
        });
        $$("[data-quick-tab]").forEach((button) => button.addEventListener("click", () => this.switchTab(button.dataset.quickTab)));
    }
    bindPanels() {
        $("#graphFilter").addEventListener("input", (event) => this.graphView.setFilter(event.target.value));
        $("#graphFitBtn").addEventListener("click", () => this.graphView.fit());
        $("#graphReloadBtn").addEventListener("click", () => void this.reloadGraph());
        $("#contextForm").addEventListener("submit", (event) => void this.runContext(event));
        $("#impactForm").addEventListener("submit", (event) => void this.runImpact(event));
        $("#assistantForm").addEventListener("submit", (event) => void this.sendAssistantMessage(event));
        $("#taskForm").addEventListener("submit", (event) => void this.addTask(event));
        $("#historySearchForm").addEventListener("submit", (event) => void this.loadHistory(event));
        $("#rememberForm").addEventListener("submit", (event) => void this.rememberDecision(event));
        $("#workspaceForm").addEventListener("submit", (event) => void this.registerWorkspace(event));
        $("#mcpForm").addEventListener("submit", (event) => void this.registerMcp(event));
        $("#settingForm").addEventListener("submit", (event) => void this.saveSetting(event));
        $$(".prompt-card").forEach((button) => button.addEventListener("click", () => {
            const input = $("#assistantInput");
            input.value = button.textContent?.trim() ?? "";
            input.focus();
        }));
    }
    renderOverview() {
        const stats = this.stats;
        const graph = this.graph;
        $("#metricFiles").textContent = compactNumber(stats?.files ?? graph?.files.length ?? 0);
        $("#metricSymbols").textContent = compactNumber(stats?.symbols ?? graph?.symbols.length ?? 0);
        $("#metricEdges").textContent = compactNumber(stats?.edges ?? graph?.edges.length ?? 0);
        $("#metricRevision").textContent = `r${stats?.index_revision ?? graph?.index_revision ?? 0}`;
        $("#metricUpdated").textContent = formatDate(stats?.updated_unix_ms ?? graph?.updated_unix_ms);
        const active = this.workspaces?.workspaces.find((workspace) => workspace.active);
        $("#activeRepositoryName").textContent = active?.name ?? this.graph?.project_root.split(/[\\/]/).pop() ?? "Current directory";
        $("#activeRepositoryPath").textContent = active?.path ?? this.graph?.project_root ?? "No active repository";
        const languages = new Map();
        graph?.files.forEach((file) => languages.set(file.language || "text", (languages.get(file.language || "text") ?? 0) + 1));
        const topLanguages = [...languages.entries()].sort((a, b) => b[1] - a[1]).slice(0, 6);
        const languageRoot = $("#languageBreakdown");
        languageRoot.innerHTML = topLanguages.length
            ? topLanguages.map(([language, count]) => `<div class="language-row"><span><i class="language-dot lang-${escapeHtml(language.toLowerCase())}"></i>${escapeHtml(language)}</span><strong>${count}</strong></div>`).join("")
            : `<div class="empty-inline">No indexed language data</div>`;
        $("#indexHealthText").textContent = graph && graph.files.length > 0 ? "Index ready" : "Index requires attention";
        $("#indexHealthText").parentElement?.setAttribute("data-tone", graph && graph.files.length > 0 ? "success" : "warning");
    }
    renderWorkspaceSwitcher() {
        const select = $("#workspaceSelect");
        const workspaces = this.workspaces?.workspaces ?? [];
        select.replaceChildren();
        if (workspaces.length === 0) {
            select.append(new Option("Current directory", ""));
            return;
        }
        workspaces.forEach((workspace) => select.append(new Option(workspace.name, workspace.id, false, workspace.active)));
    }
    renderGraph(graph) {
        this.graphView.setGraph(graph);
        $("#graphEmpty").classList.toggle("is-hidden", graph.files.length > 0);
        $("#fileGraphSvg").classList.toggle("is-hidden", graph.files.length === 0);
        $("#graphRevision").textContent = `revision ${graph.index_revision}`;
    }
    renderGraphError(message) {
        $("#graphEmpty").classList.remove("is-hidden");
        $("#graphEmpty").innerHTML = `<strong>Repository map unavailable</strong><span>${escapeHtml(message)}</span>`;
        $("#fileGraphSvg").classList.add("is-hidden");
    }
    async selectFile(file) {
        const root = $("#fileInspector");
        if (!file || !this.graph) {
            root.innerHTML = `<div class="empty-state compact"><span class="empty-icon">◇</span><strong>Select a file</strong><span>Inspect symbols and relationships.</span></div>`;
            return;
        }
        const symbols = this.graph.symbols.filter((symbol) => symbol.file_id === file.id).sort((a, b) => a.line_start - b.line_start);
        const symbolFile = new Map(this.graph.symbols.map((symbol) => [symbol.id, symbol.file_id]));
        const related = new Map();
        this.graph.edges.forEach((edge) => {
            const fromFile = symbolFile.get(edge.from);
            const toFile = symbolFile.get(edge.to);
            if (fromFile === file.id && toFile !== undefined && toFile !== file.id)
                related.set(toFile, (related.get(toFile) ?? 0) + 1);
            if (toFile === file.id && fromFile !== undefined && fromFile !== file.id)
                related.set(fromFile, (related.get(fromFile) ?? 0) + 1);
        });
        const topRelated = [...related.entries()].sort((a, b) => b[1] - a[1]).slice(0, 8).map(([id, weight]) => ({ file: this.graph?.files.find((candidate) => candidate.id === id), weight })).filter((item) => item.file);
        root.innerHTML = `
      <div class="inspector-heading"><span class="eyebrow">Selected file</span><h3>${escapeHtml(file.path.split("/").pop() ?? file.path)}</h3><code>${escapeHtml(file.path)}</code></div>
      <div class="inspector-metrics"><div><span>Language</span><strong>${escapeHtml(file.language || "text")}</strong></div><div><span>Size</span><strong>${formatBytes(file.bytes)}</strong></div><div><span>Lines</span><strong>${file.line_count}</strong></div><div><span>Symbols</span><strong>${symbols.length}</strong></div></div>
      <div class="inspector-section"><div class="section-heading"><span>Symbols</span><strong>${symbols.length}</strong></div><div class="symbol-list">${symbols.slice(0, 30).map((symbol) => `<button class="symbol-row" data-file-line="${symbol.line_start}"><span class="symbol-kind">${escapeHtml(symbol.kind)}</span><span><strong>${escapeHtml(symbol.name)}</strong><small>line ${symbol.line_start}${symbol.complexity ? ` · complexity ${symbol.complexity}` : ""}</small></span></button>`).join("") || `<div class="empty-inline">No symbols indexed</div>`}</div></div>
      <div class="inspector-section"><div class="section-heading"><span>Connected files</span><strong>${topRelated.length}</strong></div><div class="relation-list">${topRelated.map((item) => `<div class="relation-row"><span>${escapeHtml(item.file?.path)}</span><strong>${item.weight}</strong></div>`).join("") || `<div class="empty-inline">No cross-file links</div>`}</div></div>
      <button class="button primary full" id="readSelectedFileBtn">Read source</button>
      <pre class="source-preview" id="selectedFileSource"></pre>`;
        $("#readSelectedFileBtn").addEventListener("click", async () => {
            const button = $("#readSelectedFileBtn");
            button.disabled = true;
            button.textContent = "Reading…";
            try {
                const response = await this.api.readFile(file.path, 700);
                $("#selectedFileSource").textContent = response.content;
            }
            catch (error) {
                this.toasts.error(this.errorMessage(error));
            }
            finally {
                button.disabled = false;
                button.textContent = "Read source";
            }
        });
    }
    async reloadGraph() {
        try {
            this.graph = await this.api.graph();
            this.renderGraph(this.graph);
            this.renderOverview();
            this.toasts.success("Repository map refreshed");
        }
        catch (error) {
            this.renderGraphError(this.errorMessage(error));
            this.toasts.error(this.errorMessage(error));
        }
    }
    async updateIndex() {
        const button = $("#updateIndexBtn");
        button.disabled = true;
        button.dataset.loading = "true";
        try {
            await this.api.updateIndex(false);
            this.toasts.success("Index updated");
            const [graph, stats] = await Promise.all([this.api.graph(), this.api.stats()]);
            this.graph = graph;
            this.stats = stats;
            this.renderGraph(graph);
            this.renderOverview();
        }
        catch (error) {
            this.toasts.error(this.errorMessage(error));
        }
        finally {
            button.disabled = false;
            delete button.dataset.loading;
        }
    }
    async runDoctor() {
        const button = $("#doctorBtn");
        button.disabled = true;
        try {
            const result = await this.api.doctor();
            const messages = result.messages ?? [];
            this.toasts.info(messages.length ? messages.join(" · ") : "Doctor found no issues");
        }
        catch (error) {
            this.toasts.error(this.errorMessage(error));
        }
        finally {
            button.disabled = false;
        }
    }
    async selectWorkspace(id) {
        if (!id)
            return;
        try {
            await this.api.selectWorkspace(id);
            this.toasts.success("Repository switched");
            const [workspaces, graph, stats] = await Promise.all([this.api.workspaces(), this.api.graph(), this.api.stats()]);
            this.workspaces = workspaces;
            this.graph = graph;
            this.stats = stats;
            this.renderWorkspaceSwitcher();
            this.renderGraph(graph);
            this.renderOverview();
            if (this.activeTab === "workspaces")
                this.renderWorkspaces(workspaces);
        }
        catch (error) {
            this.toasts.error(this.errorMessage(error));
        }
    }
    async runContext(event) {
        event.preventDefault();
        const query = $("#contextQuery").value.trim();
        const maxTokens = Number($("#contextTokens").value) || 1600;
        const maxItems = Number($("#contextItems").value) || 10;
        if (!query)
            return;
        const root = $("#contextResults");
        root.innerHTML = this.loadingCard("Building repository context");
        try {
            const bundle = await this.api.context(query, maxTokens, maxItems);
            root.innerHTML = `<div class="result-summary"><div><span>Estimated tokens</span><strong>${bundle.estimated_tokens}</strong></div><div><span>Returned</span><strong>${formatBytes(bundle.returned_bytes)}</strong></div><div><span>Source scanned</span><strong>${formatBytes(bundle.source_bytes)}</strong></div><div><span>Items</span><strong>${bundle.items.length}</strong></div></div>${bundle.warnings.map((warning) => `<div class="callout warning">${escapeHtml(warning)}</div>`).join("")}${bundle.items.map((item) => `<article class="code-result"><header><div><span class="badge">${escapeHtml(item.kind)}</span><strong>${escapeHtml(item.symbol || item.path)}</strong></div><code>${escapeHtml(item.path)}:${item.line_start}-${item.line_end}</code></header><pre>${escapeHtml(item.content)}</pre></article>`).join("") || `<div class="empty-state"><strong>No context matched</strong><span>Try a symbol, file path, or more specific question.</span></div>`}`;
        }
        catch (error) {
            root.innerHTML = this.errorCard(this.errorMessage(error));
        }
    }
    async runImpact(event) {
        event.preventDefault();
        const from = $("#impactFrom").value.trim() || "HEAD~1";
        const to = $("#impactTo").value.trim() || "HEAD";
        const depth = Number($("#impactDepth").value) || 3;
        const root = $("#impactResults");
        root.innerHTML = this.loadingCard("Tracing change propagation");
        try {
            const report = await this.api.impact(from, to, depth);
            const tone = report.risk_score >= 70 ? "danger" : report.risk_score >= 35 ? "warning" : "success";
            root.innerHTML = `<div class="risk-card" data-tone="${tone}"><div><span>Risk score</span><strong>${report.risk_score}</strong></div><p>${escapeHtml(from)} → ${escapeHtml(to)}</p></div><div class="result-grid"><section class="surface"><div class="section-heading"><span>Changed files</span><strong>${report.changed_files.length}</strong></div>${report.changed_files.map((file) => `<div class="relation-row"><code>${escapeHtml(file)}</code></div>`).join("") || `<div class="empty-inline">No changed files detected</div>`}</section><section class="surface"><div class="section-heading"><span>Affected symbols</span><strong>${report.affected.length}</strong></div>${report.affected.slice(0, 80).map((item) => `<div class="impact-row"><span class="depth-chip">d${item.depth}</span><div><strong>${escapeHtml(item.symbol)}</strong><small>${escapeHtml(item.path)} · ${escapeHtml(item.reason)}</small></div></div>`).join("") || `<div class="empty-inline">No affected symbols detected</div>`}</section></div>${report.warnings.map((warning) => `<div class="callout warning">${escapeHtml(warning)}</div>`).join("")}`;
        }
        catch (error) {
            root.innerHTML = this.errorCard(this.errorMessage(error));
        }
    }
    async sendAssistantMessage(event) {
        event.preventDefault();
        const input = $("#assistantInput");
        const model = $("#assistantModel").value.trim();
        const query = input.value.trim();
        if (!query)
            return;
        input.value = "";
        const messages = $("#assistantMessages");
        messages.insertAdjacentHTML("beforeend", `<div class="chat-message user"><span>You</span><div>${escapeHtml(query)}</div></div><div class="chat-message assistant is-thinking" data-thinking><span>CodeSpace</span><div><i></i><i></i><i></i></div></div>`);
        messages.scrollTop = messages.scrollHeight;
        const thinking = $("[data-thinking]", messages);
        try {
            const result = await this.api.aiChat(query, model);
            thinking.classList.remove("is-thinking");
            thinking.removeAttribute("data-thinking");
            thinking.innerHTML = `<span>CodeSpace</span><div>${escapeHtml(result.response || result.error || "No response")}</div>`;
        }
        catch (error) {
            thinking.classList.remove("is-thinking");
            thinking.innerHTML = `<span>CodeSpace</span><div class="chat-error">${escapeHtml(this.errorMessage(error))}</div>`;
        }
        messages.scrollTop = messages.scrollHeight;
    }
    async loadTasks() {
        const root = $("#taskBoard");
        root.innerHTML = this.loadingCard("Loading engineering tasks");
        try {
            const response = await this.api.tasks();
            const statuses = ["todo", "in_progress", "done"];
            root.innerHTML = statuses.map((status) => {
                const tasks = response.tasks.filter((task) => task.status === status);
                const label = status === "todo" ? "Backlog" : status === "in_progress" ? "In progress" : "Done";
                return `<section class="task-column" data-task-column="${status}"><header><span>${label}</span><strong>${tasks.length}</strong></header><div>${tasks.map((task) => this.taskCard(task)).join("") || `<div class="task-empty">No tasks</div>`}</div></section>`;
            }).join("");
            $$("[data-task-status]", root).forEach((button) => button.addEventListener("click", () => void this.changeTaskStatus(button.dataset.taskId ?? "", button.dataset.taskStatus ?? "")));
            $$("[data-task-remove]", root).forEach((button) => button.addEventListener("click", () => void this.removeTask(button.dataset.taskRemove ?? "")));
        }
        catch (error) {
            root.innerHTML = this.errorCard(this.errorMessage(error));
        }
    }
    taskCard(task) {
        const next = task.status === "todo" ? "in_progress" : task.status === "in_progress" ? "done" : "todo";
        const nextLabel = next === "in_progress" ? "Start" : next === "done" ? "Complete" : "Reopen";
        return `<article class="task-card priority-${escapeHtml(task.priority)}"><div class="task-card-top"><span class="priority-label">${escapeHtml(task.priority)}</span><button class="icon-only" data-task-remove="${escapeHtml(task.id)}" aria-label="Remove task">×</button></div><h4>${escapeHtml(task.title)}</h4>${task.description ? `<p>${escapeHtml(task.description)}</p>` : ""}<div class="task-tags">${task.tags.map((tag) => `<span>${escapeHtml(tag)}</span>`).join("")}</div><footer><small>${task.due_unix_ms ? `Due ${formatDate(task.due_unix_ms)}` : task.id}</small><button class="button ghost small" data-task-id="${escapeHtml(task.id)}" data-task-status="${next}">${nextLabel}</button></footer></article>`;
    }
    async addTask(event) {
        event.preventDefault();
        const form = event.currentTarget;
        const data = new FormData(form);
        const title = text(data.get("title")).trim();
        if (!title)
            return;
        try {
            await this.api.addTask({ title, description: text(data.get("description")), priority: text(data.get("priority")) || "medium", tags: text(data.get("tags")) });
            form.reset();
            this.toasts.success("Task created");
            await this.loadTasks();
        }
        catch (error) {
            this.toasts.error(this.errorMessage(error));
        }
    }
    async changeTaskStatus(id, status) {
        if (!id || !status)
            return;
        try {
            await this.api.setTaskStatus(id, status);
            await this.loadTasks();
        }
        catch (error) {
            this.toasts.error(this.errorMessage(error));
        }
    }
    async removeTask(id) {
        if (!id)
            return;
        try {
            await this.api.removeTask(id);
            this.toasts.success("Task removed");
            await this.loadTasks();
        }
        catch (error) {
            this.toasts.error(this.errorMessage(error));
        }
    }
    async loadHistory(event) {
        event?.preventDefault();
        const query = $("#historyQuery").value.trim();
        const root = $("#historyResults");
        root.innerHTML = this.loadingCard("Loading decision memory");
        try {
            const decisions = await this.api.history(query);
            root.innerHTML = decisions.map((decision) => `<article class="decision-card"><header><div><span class="badge">${escapeHtml(decision.file || "project")}</span><strong>${escapeHtml(decision.summary)}</strong></div><time>${formatDate(decision.timestamp_unix_ms)}</time></header>${decision.rationale ? `<p>${escapeHtml(decision.rationale)}</p>` : ""}<footer><code>${escapeHtml(decision.symbol)}</code><div>${decision.tags.map((tag) => `<span>${escapeHtml(tag)}</span>`).join("")}</div></footer></article>`).join("") || `<div class="empty-state"><strong>No decisions recorded</strong><span>Capture architecture and implementation choices as durable project memory.</span></div>`;
        }
        catch (error) {
            root.innerHTML = this.errorCard(this.errorMessage(error));
        }
    }
    async rememberDecision(event) {
        event.preventDefault();
        const form = event.currentTarget;
        const data = new FormData(form);
        const summary = text(data.get("summary")).trim();
        if (!summary)
            return;
        try {
            await this.api.remember({ file: text(data.get("file")), symbol: text(data.get("symbol")), summary, rationale: text(data.get("rationale")), tags: text(data.get("tags")), session: "dashboard", agent: "user" });
            form.reset();
            this.toasts.success("Decision stored");
            await this.loadHistory();
        }
        catch (error) {
            this.toasts.error(this.errorMessage(error));
        }
    }
    async loadWorkspaces() {
        const root = $("#workspaceList");
        root.innerHTML = this.loadingCard("Loading repositories");
        try {
            this.workspaces = await this.api.workspaces();
            this.renderWorkspaceSwitcher();
            this.renderWorkspaces(this.workspaces);
        }
        catch (error) {
            root.innerHTML = this.errorCard(this.errorMessage(error));
        }
    }
    renderWorkspaces(snapshot) {
        $("#workspaceList").innerHTML = snapshot.workspaces.map((workspace) => `<article class="workspace-card ${workspace.active ? "is-active" : ""}"><div class="workspace-icon">${escapeHtml(workspace.name.slice(0, 2).toUpperCase())}</div><div><h4>${escapeHtml(workspace.name)}</h4><code>${escapeHtml(workspace.path)}</code><small>Last active ${formatDate(workspace.last_active_unix_ms)}</small></div><div class="workspace-actions">${workspace.active ? `<span class="status-pill success">Active</span>` : `<button class="button ghost small" data-workspace-select="${escapeHtml(workspace.id)}">Open</button>`}<button class="icon-only" data-workspace-remove="${escapeHtml(workspace.id)}" aria-label="Remove repository">×</button></div></article>`).join("") || `<div class="empty-state"><strong>No repositories registered</strong><span>Add a local repository directory below.</span></div>`;
        $$("[data-workspace-select]").forEach((button) => button.addEventListener("click", () => void this.selectWorkspace(button.dataset.workspaceSelect ?? "")));
        $$("[data-workspace-remove]").forEach((button) => button.addEventListener("click", () => void this.removeWorkspace(button.dataset.workspaceRemove ?? "")));
    }
    async registerWorkspace(event) {
        event.preventDefault();
        const form = event.currentTarget;
        const data = new FormData(form);
        const path = text(data.get("path")).trim();
        if (!path)
            return;
        try {
            await this.api.registerWorkspace(path, text(data.get("name")).trim());
            form.reset();
            this.toasts.success("Repository registered");
            await this.loadWorkspaces();
        }
        catch (error) {
            this.toasts.error(this.errorMessage(error));
        }
    }
    async removeWorkspace(id) {
        if (!id)
            return;
        try {
            await this.api.removeWorkspace(id);
            this.toasts.success("Repository removed");
            await this.loadWorkspaces();
        }
        catch (error) {
            this.toasts.error(this.errorMessage(error));
        }
    }
    async loadSkills() {
        const root = $("#skillsGrid");
        root.innerHTML = this.loadingCard("Loading skills");
        try {
            const response = await this.api.skills();
            root.innerHTML = response.skills.map((skill) => `<article class="skill-card ${skill.enabled ? "is-enabled" : ""}"><header><div class="skill-icon">${this.skillIcon(skill.tags)}</div><label class="switch"><input type="checkbox" data-skill-id="${escapeHtml(skill.id)}" ${skill.enabled ? "checked" : ""}><span></span></label></header><h4>${escapeHtml(skill.name)}</h4><p>${escapeHtml(skill.description)}</p><div class="skill-meta"><span>v${escapeHtml(skill.version)}</span><span>${escapeHtml(skill.source)}</span></div><div class="permission-list">${skill.permissions.map((permission) => `<span>${escapeHtml(permission)}</span>`).join("")}</div></article>`).join("") || `<div class="empty-state"><strong>No skills installed</strong><span>Install trusted skill packs to extend CodeSpace.</span></div>`;
            $$("[data-skill-id]", root).forEach((input) => input.addEventListener("change", () => void this.toggleSkill(input)));
        }
        catch (error) {
            root.innerHTML = this.errorCard(this.errorMessage(error));
        }
    }
    skillIcon(tags) {
        const joined = tags.join(" ").toLowerCase();
        if (joined.includes("security"))
            return "⌁";
        if (joined.includes("design"))
            return "◇";
        if (joined.includes("test"))
            return "✓";
        if (joined.includes("docs"))
            return "≡";
        return "✦";
    }
    async toggleSkill(input) {
        const id = input.dataset.skillId ?? "";
        input.disabled = true;
        try {
            await this.api.toggleSkill(id, input.checked);
            this.toasts.success(input.checked ? "Skill enabled" : "Skill disabled");
        }
        catch (error) {
            input.checked = !input.checked;
            this.toasts.error(this.errorMessage(error));
        }
        finally {
            input.disabled = false;
        }
    }
    async loadMcp() {
        const root = $("#mcpList");
        root.innerHTML = this.loadingCard("Inspecting MCP servers");
        try {
            const response = await this.api.mcp();
            root.innerHTML = response.servers.map((server) => `<article class="mcp-card"><div class="mcp-status" data-status="${escapeHtml(server.status)}"></div><div><h4>${escapeHtml(server.name)}</h4><code>${escapeHtml([server.command, ...server.args].join(" "))}</code>${server.last_error ? `<p class="error-text">${escapeHtml(server.last_error)}</p>` : ""}<small>${server.auto_start ? "Autostart enabled" : "Manual start"}${server.env_keys.length ? ` · env: ${server.env_keys.map(escapeHtml).join(", ")}` : ""}</small></div><div class="mcp-actions">${server.status === "running" ? `<button class="button ghost small" data-mcp-action="stop" data-mcp-id="${escapeHtml(server.id)}">Stop</button>` : `<button class="button primary small" data-mcp-action="start" data-mcp-id="${escapeHtml(server.id)}">Start</button>`}<button class="icon-only" data-mcp-action="remove" data-mcp-id="${escapeHtml(server.id)}">×</button></div></article>`).join("") || `<div class="empty-state"><strong>No external MCP servers</strong><span>Register a local command below. Environment values are never exposed in the UI.</span></div>`;
            $$("[data-mcp-action]", root).forEach((button) => button.addEventListener("click", () => void this.runMcpAction(button.dataset.mcpAction, button.dataset.mcpId ?? "")));
        }
        catch (error) {
            root.innerHTML = this.errorCard(this.errorMessage(error));
        }
    }
    async registerMcp(event) {
        event.preventDefault();
        const form = event.currentTarget;
        const data = new FormData(form);
        const name = text(data.get("name")).trim();
        const command = text(data.get("command")).trim();
        if (!name || !command)
            return;
        try {
            await this.api.registerMcp({ name, command, args: text(data.get("args")), auto_start: data.get("auto_start") === "on" });
            form.reset();
            this.toasts.success("MCP server registered");
            await this.loadMcp();
        }
        catch (error) {
            this.toasts.error(this.errorMessage(error));
        }
    }
    async runMcpAction(action, id) {
        if (!id)
            return;
        try {
            await this.api.mcpAction(action, id);
            this.toasts.success(`MCP server ${action === "remove" ? "removed" : `${action}ed`}`);
            await this.loadMcp();
        }
        catch (error) {
            this.toasts.error(this.errorMessage(error));
        }
    }
    async loadSettings() {
        const root = $("#settingsTable");
        root.innerHTML = this.loadingCard("Loading settings");
        try {
            const response = await this.api.settings();
            const entries = Object.entries(response.effective ?? {}).sort(([left], [right]) => left.localeCompare(right));
            root.innerHTML = entries.map(([key, value]) => `<div class="setting-row"><code>${escapeHtml(key)}</code><strong>${escapeHtml(value)}</strong><span>${key in (response.workspace ?? {}) ? "workspace" : "global"}</span></div>`).join("") || `<div class="empty-state compact"><strong>No custom settings</strong><span>Defaults are active.</span></div>`;
        }
        catch (error) {
            root.innerHTML = this.errorCard(this.errorMessage(error));
        }
    }
    async saveSetting(event) {
        event.preventDefault();
        const form = event.currentTarget;
        const data = new FormData(form);
        const key = text(data.get("key")).trim();
        if (!key)
            return;
        try {
            await this.api.setSetting(key, text(data.get("value")), text(data.get("scope")) || "workspace");
            form.reset();
            this.toasts.success("Setting saved");
            await this.loadSettings();
        }
        catch (error) {
            this.toasts.error(this.errorMessage(error));
        }
    }
    async loadGithub() {
        const root = $("#githubPanel");
        root.innerHTML = this.loadingCard("Loading GitHub connection");
        try {
            const status = await this.api.githubStatus();
            const configured = Boolean(status.token_set ?? status.configured);
            root.innerHTML = configured
                ? `<div class="integration-hero success"><div class="integration-logo">GH</div><div><span class="eyebrow">Connected</span><h3>@${escapeHtml(status.username)}</h3><p>${escapeHtml(status.default_owner && status.default_repo ? `${status.default_owner}/${status.default_repo}` : "No default repository selected")}</p></div></div><div class="callout info">GitHub credentials remain in the local repository configuration. CodeSpace does not send source code to GitHub unless you invoke an explicit Git operation.</div>`
                : `<div class="integration-hero"><div class="integration-logo">GH</div><div><span class="eyebrow">Not connected</span><h3>Link GitHub from the CLI</h3><p><code>cse github link --token &lt;token&gt; --username &lt;user&gt;</code></p></div></div><div class="callout warning">Do not paste personal access tokens into browser forms. Link them through the local CLI so the value never enters page state.</div>`;
        }
        catch (error) {
            root.innerHTML = this.errorCard(this.errorMessage(error));
        }
    }
    handleEvent(event) {
        const type = text(event.type);
        if (type === "index.updated") {
            this.toasts.info("Index updated in the background");
            void this.reloadGraph();
        }
        else if (type.startsWith("workspace.")) {
            void this.loadWorkspaces();
        }
        else if (type.startsWith("skill.") && this.activeTab === "skills") {
            void this.loadSkills();
        }
        else if (type.startsWith("mcp.server.") && this.activeTab === "mcp") {
            void this.loadMcp();
        }
    }
    setBusy(message) {
        const overlay = $("#bootOverlay");
        overlay.classList.remove("is-hidden");
        $("#bootMessage").textContent = message;
    }
    clearBusy() {
        $("#bootOverlay").classList.add("is-hidden");
    }
    loadingCard(message) {
        return `<div class="loading-card"><span class="spinner"></span><span>${escapeHtml(message)}</span></div>`;
    }
    errorCard(message) {
        return `<div class="empty-state error"><strong>Request failed</strong><span>${escapeHtml(message)}</span></div>`;
    }
    errorMessage(error) {
        return error instanceof Error ? error.message : "Unexpected error";
    }
}
const start = () => {
    try {
        new CodeSpaceApp().start();
    }
    catch (error) {
        console.error(error);
        const overlay = document.querySelector("#bootOverlay");
        if (overlay) {
            overlay.classList.remove("is-hidden");
            overlay.innerHTML = `<div class="boot-card error"><strong>Dashboard failed to start</strong><span>${escapeHtml(error instanceof Error ? error.message : "Unknown bootstrap error")}</span></div>`;
        }
    }
};
if (document.readyState === "loading")
    document.addEventListener("DOMContentLoaded", start, { once: true });
else
    start();
