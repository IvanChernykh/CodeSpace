import type { ApiClient } from "../api.js";
import { ApiError } from "../api.js";
import type { Decision } from "../types.js";
import { el, formatTimestamp, qs } from "../dom.js";
import type { ToastManager } from "../toast.js";

export class HistoryPanel {
  private readonly api: ApiClient;
  private readonly toast: ToastManager;
  private readonly searchInput: HTMLInputElement;
  private readonly searchButton: HTMLButtonElement;
  private readonly resultsEl: HTMLElement;
  private readonly form: HTMLFormElement;

  constructor(root: HTMLElement, api: ApiClient, toast: ToastManager) {
    this.api = api;
    this.toast = toast;
    this.searchInput = qs<HTMLInputElement>("[data-history-query]", root);
    this.searchButton = qs<HTMLButtonElement>("[data-history-run]", root);
    this.resultsEl = qs("[data-history-results]", root);
    this.form = qs<HTMLFormElement>("[data-remember-form]", root);

    this.searchButton.addEventListener("click", () => void this.run());
    this.searchInput.addEventListener("keydown", (ev) => {
      if (ev.key === "Enter") void this.run();
    });
    this.form.addEventListener("submit", (ev) => {
      ev.preventDefault();
      void this.submitRemember();
    });
  }

  async run(): Promise<void> {
    const query = this.searchInput.value.trim();
    this.resultsEl.replaceChildren(el("div", { class: "spinner-row" }, ["Loading decisions\u2026"]));
    try {
      const decisions = await this.api.history(query, 50);
      this.renderDecisions(decisions);
    } catch (error) {
      this.resultsEl.replaceChildren(
        el("div", { class: "panel-error" }, [error instanceof ApiError ? error.message : "Failed to load history"]),
      );
    }
  }

  private renderDecisions(decisions: Decision[]): void {
    if (decisions.length === 0) {
      this.resultsEl.replaceChildren(
        el("div", { class: "empty-panel" }, ["No engineering decisions recorded yet. Use the form to add one."]),
      );
      return;
    }
    this.resultsEl.replaceChildren(
      ...decisions.map((decision) =>
        el("div", { class: "decision-card" }, [
          el("div", { class: "decision-header" }, [
            el("strong", {}, [decision.summary]),
            el("span", { class: "muted small" }, [formatTimestamp(decision.timestamp_unix_ms)]),
          ]),
          el("div", { class: "decision-meta" }, [
            decision.file ? el("code", {}, [decision.file]) : el("span", {}),
            decision.symbol ? el("code", { class: "accent" }, [decision.symbol]) : el("span", {}),
          ]),
          decision.rationale ? el("p", { class: "decision-rationale" }, [decision.rationale]) : el("p", {}),
          el(
            "div",
            { class: "chip-row" },
            decision.tags.map((tag) => el("span", { class: "chip static" }, [tag])),
          ),
        ]),
      ),
    );
  }

  private async submitRemember(): Promise<void> {
    const data = new FormData(this.form);
    const summary = String(data.get("summary") ?? "").trim();
    if (!summary) {
      this.toast.warning("A summary is required to remember a decision");
      return;
    }
    const submitButton = qs<HTMLButtonElement>("[data-remember-submit]", this.form);
    submitButton.disabled = true;
    try {
      await this.api.remember({
        file: String(data.get("file") ?? ""),
        symbol: String(data.get("symbol") ?? ""),
        summary,
        rationale: String(data.get("rationale") ?? ""),
        session: String(data.get("session") ?? "dashboard"),
        agent: String(data.get("agent") ?? "dashboard-user"),
        tags: String(data.get("tags") ?? ""),
      });
      this.toast.success("Decision remembered");
      this.form.reset();
      await this.run();
    } catch (error) {
      this.toast.error(error instanceof ApiError ? error.message : "Failed to remember decision");
    } finally {
      submitButton.disabled = false;
    }
  }
}
