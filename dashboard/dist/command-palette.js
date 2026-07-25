import { debounce, el, escapeHtml, qs } from "./dom.js";
export class CommandPalette {
    constructor(overlay, options) {
        this.entries = [];
        this.activeIndex = 0;
        this.overlay = overlay;
        this.options = options;
        this.input = qs("[data-palette-input]", overlay);
        this.listEl = qs("[data-palette-list]", overlay);
        this.debouncedSearch = debounce((query) => void this.search(query), 180);
        this.input.addEventListener("input", () => this.debouncedSearch(this.input.value.trim()));
        this.input.addEventListener("keydown", (ev) => this.handleKeydown(ev));
        this.overlay.addEventListener("click", (ev) => {
            if (ev.target === this.overlay)
                this.close();
        });
        qs("[data-palette-close]", overlay).addEventListener("click", () => this.close());
    }
    open() {
        this.overlay.classList.add("is-open");
        this.input.value = "";
        this.input.focus();
        this.renderCommandsOnly();
    }
    close() {
        this.overlay.classList.remove("is-open");
    }
    toggle() {
        if (this.overlay.classList.contains("is-open"))
            this.close();
        else
            this.open();
    }
    isOpen() {
        return this.overlay.classList.contains("is-open");
    }
    renderCommandsOnly() {
        this.entries = this.options.commands.map((command) => ({ kind: "command", command }));
        this.activeIndex = 0;
        this.renderList();
    }
    async search(query) {
        if (!query) {
            this.renderCommandsOnly();
            return;
        }
        const matchingCommands = this.options.commands
            .filter((c) => c.label.toLowerCase().includes(query.toLowerCase()))
            .map((command) => ({ kind: "command", command }));
        try {
            const hits = await this.options.api.search(query, { limit: 15 });
            this.entries = [...matchingCommands, ...hits.map((hit) => ({ kind: "symbol", hit }))];
        }
        catch {
            this.entries = matchingCommands;
        }
        this.activeIndex = 0;
        this.renderList();
    }
    renderList() {
        if (this.entries.length === 0) {
            this.listEl.replaceChildren(el("div", { class: "palette-empty" }, ["No matches."]));
            return;
        }
        const items = this.entries.map((entry, index) => {
            const active = index === this.activeIndex;
            if (entry.kind === "command") {
                const item = el("div", { class: `palette-item${active ? " is-active" : ""}` }, [
                    el("span", { class: "palette-item-label" }, [entry.command.label]),
                    entry.command.hint ? el("span", { class: "palette-item-hint" }, [entry.command.hint]) : el("span", {}),
                ]);
                item.addEventListener("click", () => this.activate(index));
                return item;
            }
            const item = el("div", { class: `palette-item${active ? " is-active" : ""}` }, [
                el("span", { class: `kind-dot kind-${entry.hit.kind}` }),
                el("span", { class: "palette-item-label" }, [entry.hit.qualified_name]),
                el("span", { class: "palette-item-hint" }, [escapeHtml(entry.hit.path)]),
            ]);
            item.addEventListener("click", () => this.activate(index));
            return item;
        });
        this.listEl.replaceChildren(...items);
    }
    activate(index) {
        const entry = this.entries[index];
        if (!entry)
            return;
        if (entry.kind === "command") {
            entry.command.run();
        }
        else {
            this.options.onSelectSymbol(entry.hit.id);
        }
        this.close();
    }
    handleKeydown(ev) {
        if (ev.key === "Escape") {
            this.close();
            return;
        }
        if (ev.key === "ArrowDown") {
            ev.preventDefault();
            this.activeIndex = Math.min(this.activeIndex + 1, this.entries.length - 1);
            this.renderList();
            return;
        }
        if (ev.key === "ArrowUp") {
            ev.preventDefault();
            this.activeIndex = Math.max(this.activeIndex - 1, 0);
            this.renderList();
            return;
        }
        if (ev.key === "Enter") {
            ev.preventDefault();
            this.activate(this.activeIndex);
        }
    }
}
