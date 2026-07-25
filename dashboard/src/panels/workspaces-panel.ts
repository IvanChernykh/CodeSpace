import type { ApiClient } from "../api.js";
import { ApiError } from "../api.js";
import type { WorkspaceEntry, WorkspaceRegistrySnapshot } from "../types.js";
import { el, formatRelativeTime, qs } from "../dom.js";
import type { ToastManager } from "../toast.js";

export interface WorkspacesPanelOptions {
  onChanged: (snapshot: WorkspaceRegistrySnapshot) => void;
}

export class WorkspacesPanel {
  private readonly api: ApiClient;
  private readonly toast: ToastManager;
  private readonly listEl: HTMLElement;
  private readonly form: HTMLFormElement;
  private readonly options: WorkspacesPanelOptions;

  constructor(root: HTMLElement, api: ApiClient, toast: ToastManager, options: WorkspacesPanelOptions) {
    this.api = api;
    this.toast = toast;
    this.options = options;
    this.listEl = qs("[data-workspaces-list]", root);
    this.form = qs<HTMLFormElement>("[data-workspace-form]", root);
    this.form.addEventListener("submit", (ev) => {
      ev.preventDefault();
      void this.register();
    });
  }

  async load(): Promise<void> {
    this.listEl.replaceChildren(el("div", { class: "spinner-row" }, ["Loading workspaces\u2026"]));
    try {
      const snapshot = await this.api.workspaces();
      this.render(snapshot);
      this.options.onChanged(snapshot);
    } catch (error) {
      this.listEl.replaceChildren(
        el("div", { class: "panel-error" }, [error instanceof ApiError ? error.message : "Failed to load workspaces"]),
      );
    }
  }

  private render(snapshot: WorkspaceRegistrySnapshot): void {
    if (snapshot.workspaces.length === 0) {
      this.listEl.replaceChildren(
        el("div", { class: "empty-panel" }, ["No workspaces registered yet. Add one with the form below."]),
      );
      return;
    }
    this.listEl.replaceChildren(
      ...snapshot.workspaces.map((ws) => this.card(ws)),
    );
  }

  private card(ws: WorkspaceEntry): HTMLElement {
    const selectBtn = el(
      "button",
      { class: `secondary-button${ws.active ? " is-disabled" : ""}`, type: "button" },
      [ws.active ? "Active" : "Switch to this"],
    );
    if (!ws.active) {
      selectBtn.addEventListener("click", async () => {
        try {
          await this.api.selectWorkspace(ws.id);
          this.toast.success(`Switched to ${ws.name}`);
          await this.load();
        } catch (error) {
          this.toast.error(error instanceof ApiError ? error.message : "Failed to switch workspace");
        }
      });
    } else {
      selectBtn.disabled = true;
    }
    const removeBtn = el("button", { class: "icon-button danger", type: "button", title: "Remove workspace" }, ["\u2715"]);
    removeBtn.addEventListener("click", async () => {
      try {
        await this.api.removeWorkspace(ws.id);
        this.toast.info(`Removed ${ws.name}`);
        await this.load();
      } catch (error) {
        this.toast.error(error instanceof ApiError ? error.message : "Failed to remove workspace");
      }
    });

    return el("div", { class: `workspace-card${ws.active ? " is-active" : ""}` }, [
      el("div", { class: "workspace-card-header" }, [
        el("strong", {}, [ws.name]),
        ws.active ? el("span", { class: "chip static accent" }, ["active"]) : el("span", {}),
      ]),
      el("code", { class: "workspace-path" }, [ws.path]),
      el("div", { class: "muted small" }, [`last active ${formatRelativeTime(ws.last_active_unix_ms)}`]),
      el("div", { class: "workspace-actions" }, [selectBtn, removeBtn]),
    ]);
  }

  private async register(): Promise<void> {
    const data = new FormData(this.form);
    const path = String(data.get("path") ?? "").trim();
    if (!path) {
      this.toast.warning("Enter a directory path to register");
      return;
    }
    const name = String(data.get("name") ?? "").trim();
    const submitButton = qs<HTMLButtonElement>("[data-workspace-submit]", this.form);
    submitButton.disabled = true;
    try {
      const result = await this.api.registerWorkspace(path, name || undefined);
      this.toast.success(`Registered workspace "${result.name}"`);
      this.form.reset();
      await this.load();
    } catch (error) {
      this.toast.error(error instanceof ApiError ? error.message : "Failed to register workspace");
    } finally {
      submitButton.disabled = false;
    }
  }
}
