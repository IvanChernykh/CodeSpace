import { ApiError } from "../api.js";
import { copyToClipboard, el, qs } from "../dom.js";
export class ContextPanel {
    constructor(root, api, toast) {
        this.root = root;
        this.api = api;
        this.toast = toast;
        this.queryInput = qs("[data-context-query]", root);
        this.tokensInput = qs("[data-context-tokens]", root);
        this.itemsInput = qs("[data-context-items]", root);
        this.runButton = qs("[data-context-run]", root);
        this.resultsEl = qs("[data-context-results]", root);
        this.runButton.addEventListener("click", () => void this.run());
        this.queryInput.addEventListener("keydown", (ev) => {
            if (ev.key === "Enter")
                void this.run();
        });
    }
    setQuery(query) {
        this.queryInput.value = query;
    }
    async run() {
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
        }
        catch (error) {
            this.resultsEl.replaceChildren(el("div", { class: "panel-error" }, [error instanceof ApiError ? error.message : "Failed to build context"]));
        }
        finally {
            this.runButton.disabled = false;
            this.runButton.textContent = "Build context";
        }
    }
    renderBundle(query, tokens, items, warnings) {
        if (items.length === 0) {
            this.resultsEl.replaceChildren(el("div", { class: "empty-panel" }, [`No context found for "${query}". Try a different symbol, file, or term.`]));
            return;
        }
        const summary = el("div", { class: "context-summary" }, [
            el("span", {}, [`~${tokens} tokens`]),
            el("span", {}, [`${items.length} item(s)`]),
        ]);
        const warningsEl = warnings.length > 0
            ? el("div", { class: "banner banner-warning" }, warnings.map((w) => el("div", {}, [w])))
            : el("div", {});
        const cards = items.map((item) => {
            const copyBtn = el("button", { class: "icon-button", type: "button", title: "Copy snippet" }, ["\u2398"]);
            copyBtn.addEventListener("click", async () => {
                const ok = await copyToClipboard(item.content);
                if (ok)
                    this.toast.success("Snippet copied");
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
