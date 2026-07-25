import { ApiError } from "../api.js";
import { el, formatRelativeTime, qs } from "../dom.js";
export class WorkspacesPanel {
    constructor(root, api, toast, options) {
        this.api = api;
        this.toast = toast;
        this.options = options;
        this.listEl = qs("[data-workspaces-list]", root);
        this.form = qs("[data-workspace-form]", root);
        this.form.addEventListener("submit", (ev) => {
            ev.preventDefault();
            void this.register();
        });
    }
    async load() {
        this.listEl.replaceChildren(el("div", { class: "spinner-row" }, ["Loading workspaces\u2026"]));
        try {
            const snapshot = await this.api.workspaces();
            this.render(snapshot);
            this.options.onChanged(snapshot);
        }
        catch (error) {
            this.listEl.replaceChildren(el("div", { class: "panel-error" }, [error instanceof ApiError ? error.message : "Failed to load workspaces"]));
        }
    }
    render(snapshot) {
        if (snapshot.workspaces.length === 0) {
            this.listEl.replaceChildren(el("div", { class: "empty-panel" }, ["No workspaces registered yet. Add one with the form below."]));
            return;
        }
        this.listEl.replaceChildren(...snapshot.workspaces.map((ws) => this.card(ws)));
    }
    card(ws) {
        const selectBtn = el("button", { class: `secondary-button${ws.active ? " is-disabled" : ""}`, type: "button" }, [ws.active ? "Active" : "Switch to this"]);
        if (!ws.active) {
            selectBtn.addEventListener("click", async () => {
                try {
                    await this.api.selectWorkspace(ws.id);
                    this.toast.success(`Switched to ${ws.name}`);
                    await this.load();
                }
                catch (error) {
                    this.toast.error(error instanceof ApiError ? error.message : "Failed to switch workspace");
                }
            });
        }
        else {
            selectBtn.disabled = true;
        }
        const removeBtn = el("button", { class: "icon-button danger", type: "button", title: "Remove workspace" }, ["\u2715"]);
        removeBtn.addEventListener("click", async () => {
            try {
                await this.api.removeWorkspace(ws.id);
                this.toast.info(`Removed ${ws.name}`);
                await this.load();
            }
            catch (error) {
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
    async register() {
        const data = new FormData(this.form);
        const path = String(data.get("path") ?? "").trim();
        if (!path) {
            this.toast.warning("Enter a directory path to register");
            return;
        }
        const name = String(data.get("name") ?? "").trim();
        const submitButton = qs("[data-workspace-submit]", this.form);
        submitButton.disabled = true;
        try {
            const result = await this.api.registerWorkspace(path, name || undefined);
            this.toast.success(`Registered workspace "${result.name}"`);
            this.form.reset();
            await this.load();
        }
        catch (error) {
            this.toast.error(error instanceof ApiError ? error.message : "Failed to register workspace");
        }
        finally {
            submitButton.disabled = false;
        }
    }
}
