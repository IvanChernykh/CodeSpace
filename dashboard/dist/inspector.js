import { ApiError } from "./api.js";
import { copyToClipboard, el, escapeHtml } from "./dom.js";
export class Inspector {
    constructor(root, options) {
        this.root = root;
        this.options = options;
        this.renderEmpty();
    }
    renderEmpty() {
        this.root.replaceChildren(el("div", { class: "inspector-empty" }, [
            el("p", {}, ["Select a symbol from the graph or the list to inspect it."]),
            el("p", { class: "muted" }, ["Tip: press / to search, or click a node to focus it."]),
        ]));
    }
    render(symbol, graph) {
        const file = graph.files.find((f) => f.id === symbol.file_id);
        const outgoing = graph.edges.filter((e) => e.from === symbol.id);
        const incoming = graph.edges.filter((e) => e.to === symbol.id);
        this.root.replaceChildren(el("div", { class: "inspector-header" }, [
            el("span", { class: `kind-dot kind-${symbol.kind}` }),
            el("h3", {}, [symbol.name]),
        ]), el("div", { class: "inspector-qname" }, [
            el("code", {}, [symbol.qualified_name]),
            this.copyButton(symbol.qualified_name),
        ]), this.field("Location", file ? `${file.path}:${symbol.line_start}-${symbol.line_end}` : "unknown"), this.field("Complexity", String(symbol.complexity)), el("div", { class: "field" }, [
            el("div", { class: "field-label" }, ["Signature"]),
            el("pre", { class: "code-block" }, [symbol.signature || "(no signature captured)"]),
        ]), symbol.doc
            ? el("div", { class: "field" }, [
                el("div", { class: "field-label" }, ["Documentation"]),
                el("p", { class: "doc-text" }, [symbol.doc]),
            ])
            : el("div", {}), this.relationshipsSection(graph, outgoing, incoming), this.sourceSection(file, symbol));
    }
    field(label, value) {
        return el("div", { class: "field" }, [
            el("div", { class: "field-label" }, [label]),
            el("div", { class: "field-value" }, [value]),
        ]);
    }
    copyButton(value) {
        const button = el("button", { class: "icon-button", type: "button", title: "Copy qualified name" }, ["\u2398"]);
        button.addEventListener("click", async () => {
            const ok = await copyToClipboard(value);
            if (ok)
                this.options.toast.success("Copied qualified name");
            else
                this.options.toast.error("Clipboard is unavailable");
        });
        return button;
    }
    relationshipsSection(graph, outgoing, incoming) {
        const total = outgoing.length + incoming.length;
        const rows = [];
        for (const edge of outgoing) {
            rows.push(this.relationshipRow(graph, edge, edge.to, "out"));
        }
        for (const edge of incoming) {
            rows.push(this.relationshipRow(graph, edge, edge.from, "in"));
        }
        return el("div", { class: "field" }, [
            el("div", { class: "field-label" }, [`Relationships (${total})`]),
            total === 0
                ? el("p", { class: "muted" }, ["No recorded relationships."])
                : el("table", { class: "edge-table" }, [
                    el("thead", {}, [
                        el("tr", {}, [
                            el("th", {}, ["Dir"]),
                            el("th", {}, ["Kind"]),
                            el("th", {}, ["Target"]),
                            el("th", {}, ["Conf."]),
                        ]),
                    ]),
                    el("tbody", {}, rows),
                ]),
        ]);
    }
    relationshipRow(graph, edge, targetId, direction) {
        const targetSymbol = graph.symbols.find((s) => s.id === targetId);
        const targetFile = graph.files.find((f) => f.id === targetId);
        const label = targetSymbol?.qualified_name ?? targetFile?.path ?? `#${targetId}`;
        const row = el("tr", { class: "edge-row" }, [
            el("td", {}, [el("span", { class: `dir-badge dir-${direction}` }, [direction])]),
            el("td", {}, [edge.kind]),
            el("td", {}, [el("code", {}, [label])]),
            el("td", {}, [(edge.confidence_milli / 10).toFixed(0) + "%"]),
        ]);
        if (targetSymbol) {
            row.classList.add("is-clickable");
            row.addEventListener("click", () => this.options.onFocusSymbol(targetSymbol.id));
        }
        return row;
    }
    sourceSection(file, symbol) {
        const container = el("div", { class: "field" }, [
            el("div", { class: "field-label" }, ["Source"]),
        ]);
        const button = el("button", { class: "secondary-button", type: "button" }, ["Read source"]);
        const output = el("pre", { class: "code-block source-preview" }, []);
        button.addEventListener("click", async () => {
            if (!file)
                return;
            button.disabled = true;
            button.textContent = "Loading\u2026";
            try {
                const response = await this.options.api.readFile(file.path, symbol.line_end + 20);
                output.textContent = response.content;
            }
            catch (error) {
                output.textContent = error instanceof ApiError ? error.message : "Failed to read file";
            }
            finally {
                button.disabled = false;
                button.textContent = "Read source";
            }
        });
        container.append(button, output);
        return container;
    }
}
export function selectedSymbolBanner(root, text) {
    root.replaceChildren(el("div", { class: "inspector-empty" }, [escapeHtml(text)]));
}
