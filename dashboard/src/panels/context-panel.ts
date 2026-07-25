import type { ApiClient } from "../api.js";
import { ApiError } from "../api.js";
import { copyToClipboard, el, qs } from "../dom.js";
import type { ToastManager } from "../toast.js";

export class ContextPanel {
  private readonly root: HTMLElement;
  private readonly api: ApiClient;
  private readonly toast: ToastManager;
  private readonly queryInput: HTMLInputElement;
  private readonly tokensInput: HTMLInputElement;
  private readonly itemsInput: HTMLInputElement;
  private readonly runButton: HTMLButtonElement;
  private readonly resultsEl: HTMLElement;

  constructor(root: HTMLElement, api: ApiClient, toast: ToastManager) {
    this.root = root;
    this.api = api;
    this.toast = toast;
    this.queryInput = qs<HTMLInputElement>("[data-context-query]", root);
    this.tokensInput = qs<HTMLInputElement>("[data-context-tokens]", root);
    this.itemsInput = qs<HTMLInputElement>("[data-context-items]", root);
    this.runButton = qs<HTMLButtonElement>("[data-context-run]", root);
    this.resultsEl = qs("[data-context-results]", root);

    this.runButton.addEventListener("click", () => void this.run());
    this.queryInput.addEventListener("keydown", (ev) => {
      if (ev.key === "Enter") void this.run();
    });
  }

  setQuery(query: string): void {
    this.queryInput.value = query;
  }

  async run(): Promise<void> {
    const query = this.queryInput.value.trim();
    if (!query) {
      this.toast.warning("Enter a query to build context for");
      return;
    }
    this.runButton.disabled = true;
    this.runButton.textContent = "Building\u2026";
    this.resultsEl.replaceChildren(el("div", { class: "spinner-row" }, ["Building context bundle\u2026"]));
    try {
      const bundle = await this.api.context(query, {
        max_tokens: Number(this.tokensInput.value) || 1200,
        max_items: Number(this.itemsInput.value) || 8,
      });
      this.renderBundle(bundle.query, bundle.estimated_tokens, bundle.items, bundle.warnings);
    } catch (error) {
      this.resultsEl.replaceChildren(
        el("div", { class: "panel-error" }, [error instanceof ApiError ? error.message : "Failed to build context"]),
      );
    } finally {
      this.runButton.disabled = false;
      this.runButton.textContent = "Build context";
    }
  }

  private renderBundle(
    query: string,
    tokens: number,
    items: { path: string; symbol: string; kind: string; line_start: number; line_end: number; score_milli: number; content: string; redactions: number }[],
    warnings: string[],
  ): void {
    if (items.length === 0) {
      this.resultsEl.replaceChildren(
        el("div", { class: "empty-panel" }, [`No context found for "${query}". Try a different symbol, file, or term.`]),
      );
      return;
    }
    const summary = el("div", { class: "context-summary" }, [
      el("span", {}, [`~${tokens} tokens`]),
      el("span", {}, [`${items.length} item(s)`]),
    ]);
    const warningsEl =
      warnings.length > 0
        ? el(
            "div",
            { class: "banner banner-warning" },
            warnings.map((w) => el("div", {}, [w])),
          )
        : el("div", {});

    const cards = items.map((item) => {
      const copyBtn = el("button", { class: "icon-button", type: "button", title: "Copy snippet" }, ["\u2398"]);
      copyBtn.addEventListener("click", async () => {
        const ok = await copyToClipboard(item.content);
        if (ok) this.toast.success("Snippet copied");
      });
      return el("div", { class: "ctx-card" }, [
        el("div", { class: "ctx-card-header" }, [
          el("code", { class: "ctx-symbol" }, [item.symbol || item.path]),
          el("span", { class: "ctx-meta" }, [
            `${item.path}:${item.line_start}-${item.line_end} \u00b7 score ${(item.score_milli / 10).toFixed(0)}%`,
          ]),
          copyBtn,
        ]),
        item.redactions > 0
          ? el("div", { class: "banner banner-warning small" }, [`${item.redactions} secret(s) redacted`])
          : el("div", {}),
        el("pre", { class: "code-block" }, [item.content]),
      ]);
    });

    this.resultsEl.replaceChildren(summary, warningsEl, ...cards);
  }
}
