import type { ApiClient } from "./api.js";
import type { SearchHit, TabId } from "./types.js";
import { debounce, el, escapeHtml, qs } from "./dom.js";

export interface PaletteCommand {
  id: string;
  label: string;
  hint?: string;
  run: () => void;
}

export interface CommandPaletteOptions {
  api: ApiClient;
  commands: PaletteCommand[];
  onSelectSymbol: (id: number) => void;
  onSwitchTab: (tab: TabId) => void;
}

type ResultEntry =
  | { kind: "command"; command: PaletteCommand }
  | { kind: "symbol"; hit: SearchHit };

export class CommandPalette {
  private readonly overlay: HTMLElement;
  private readonly input: HTMLInputElement;
  private readonly listEl: HTMLElement;
  private readonly options: CommandPaletteOptions;
  private entries: ResultEntry[] = [];
  private activeIndex = 0;
  private readonly debouncedSearch: (query: string) => void;

  constructor(overlay: HTMLElement, options: CommandPaletteOptions) {
    this.overlay = overlay;
    this.options = options;
    this.input = qs<HTMLInputElement>("[data-palette-input]", overlay);
    this.listEl = qs("[data-palette-list]", overlay);

    this.debouncedSearch = debounce((query: string) => void this.search(query), 180);

    this.input.addEventListener("input", () => this.debouncedSearch(this.input.value.trim()));
    this.input.addEventListener("keydown", (ev) => this.handleKeydown(ev));
    this.overlay.addEventListener("click", (ev) => {
      if (ev.target === this.overlay) this.close();
    });
    qs<HTMLButtonElement>("[data-palette-close]", overlay).addEventListener("click", () => this.close());
  }

  open(): void {
    this.overlay.classList.add("is-open");
    this.input.value = "";
    this.input.focus();
    this.renderCommandsOnly();
  }

  close(): void {
    this.overlay.classList.remove("is-open");
  }

  toggle(): void {
    if (this.overlay.classList.contains("is-open")) this.close();
    else this.open();
  }

  isOpen(): boolean {
    return this.overlay.classList.contains("is-open");
  }

  private renderCommandsOnly(): void {
    this.entries = this.options.commands.map((command) => ({ kind: "command", command }));
    this.activeIndex = 0;
    this.renderList();
  }

  private async search(query: string): Promise<void> {
    if (!query) {
      this.renderCommandsOnly();
      return;
    }
    const matchingCommands = this.options.commands
      .filter((c) => c.label.toLowerCase().includes(query.toLowerCase()))
      .map((command): ResultEntry => ({ kind: "command", command }));
    try {
      const hits = await this.options.api.search(query, { limit: 15 });
      this.entries = [...matchingCommands, ...hits.map((hit): ResultEntry => ({ kind: "symbol", hit }))];
    } catch {
      this.entries = matchingCommands;
    }
    this.activeIndex = 0;
    this.renderList();
  }

  private renderList(): void {
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

  private activate(index: number): void {
    const entry = this.entries[index];
    if (!entry) return;
    if (entry.kind === "command") {
      entry.command.run();
    } else {
      this.options.onSelectSymbol(entry.hit.id);
    }
    this.close();
  }

  private handleKeydown(ev: KeyboardEvent): void {
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
