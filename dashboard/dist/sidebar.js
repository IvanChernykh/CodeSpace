import { el, escapeHtml, qs } from "./dom.js";
const KIND_ORDER = [
    "class",
    "struct",
    "interface",
    "trait",
    "enum",
    "function",
    "method",
    "constant",
    "variable",
    "type_alias",
    "module",
    "test",
    "unknown",
];
export class Sidebar {
    constructor(root, options) {
        this.graph = null;
        this.activeKinds = new Set();
        this.query = "";
        this.selectedId = null;
        this.root = root;
        this.options = options;
        this.filterInput = qs("[data-sidebar-filter]", root);
        this.chipsEl = qs("[data-sidebar-chips]", root);
        this.listEl = qs("[data-sidebar-list]", root);
        this.countEl = qs("[data-sidebar-count]", root);
        this.filterInput.addEventListener("input", () => {
            this.query = this.filterInput.value.trim().toLowerCase();
            this.renderList();
        });
    }
    setGraph(graph) {
        this.graph = graph;
        const kinds = new Set(graph.symbols.map((s) => s.kind));
        this.activeKinds = new Set(kinds);
        this.renderChips(kinds);
        this.renderList();
    }
    setSelected(id) {
        this.selectedId = id;
        for (const item of this.listEl.querySelectorAll("[data-symbol-id]")) {
            item.classList.toggle("is-active", Number(item.dataset.symbolId) === id);
        }
    }
    renderChips(kinds) {
        this.chipsEl.replaceChildren();
        const ordered = KIND_ORDER.filter((k) => kinds.has(k));
        for (const kind of ordered) {
            const chip = el("button", { class: "chip is-active", type: "button", "data-kind": kind }, [kind]);
            chip.addEventListener("click", () => {
                if (this.activeKinds.has(kind)) {
                    this.activeKinds.delete(kind);
                    chip.classList.remove("is-active");
                }
                else {
                    this.activeKinds.add(kind);
                    chip.classList.add("is-active");
                }
                this.renderList();
            });
            this.chipsEl.append(chip);
        }
    }
    matching() {
        if (!this.graph)
            return [];
        return this.graph.symbols
            .filter((s) => this.activeKinds.has(s.kind))
            .filter((s) => this.query === "" || s.qualified_name.toLowerCase().includes(this.query))
            .sort((a, b) => a.qualified_name.localeCompare(b.qualified_name));
    }
    renderList() {
        const items = this.matching();
        this.countEl.textContent = `${items.length}`;
        if (items.length === 0) {
            this.listEl.replaceChildren(el("li", { class: "sidebar-empty" }, ["No symbols match the current filters."]));
            return;
        }
        const fragment = document.createDocumentFragment();
        for (const symbol of items.slice(0, 400)) {
            const item = el("li", {
                class: `sidebar-item${symbol.id === this.selectedId ? " is-active" : ""}`,
                "data-symbol-id": String(symbol.id),
            }, [
                el("span", { class: `kind-dot kind-${symbol.kind}` }),
                el("span", { class: "sidebar-item-name" }, [symbol.name]),
                el("span", { class: "sidebar-item-kind" }, [symbol.kind]),
            ]);
            item.title = symbol.qualified_name;
            item.addEventListener("click", () => this.options.onSelect(symbol.id));
            fragment.append(item);
        }
        this.listEl.replaceChildren(fragment);
    }
}
export function renderEmptyState(container, message) {
    container.replaceChildren(el("div", { class: "empty-panel" }, [escapeHtml(message)]));
}
