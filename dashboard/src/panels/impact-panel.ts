import type { ApiClient } from "../api.js";
import { ApiError } from "../api.js";
import type { ImpactNode } from "../types.js";
import { el, qs } from "../dom.js";

export class ImpactPanel {
  private readonly api: ApiClient;
  private readonly fromInput: HTMLInputElement;
  private readonly toInput: HTMLInputElement;
  private readonly depthInput: HTMLInputElement;
  private readonly runButton: HTMLButtonElement;
  private readonly resultsEl: HTMLElement;
  private readonly onFocusSymbol: (id: number) => void;

  constructor(root: HTMLElement, api: ApiClient, onFocusSymbol: (id: number) => void) {
    this.api = api;
    this.onFocusSymbol = onFocusSymbol;
    this.fromInput = qs<HTMLInputElement>("[data-impact-from]", root);
    this.toInput = qs<HTMLInputElement>("[data-impact-to]", root);
    this.depthInput = qs<HTMLInputElement>("[data-impact-depth]", root);
    this.runButton = qs<HTMLButtonElement>("[data-impact-run]", root);
    this.resultsEl = qs("[data-impact-results]", root);
    this.runButton.addEventListener("click", () => void this.run());
  }

  async run(): Promise<void> {
    const from = this.fromInput.value.trim() || "HEAD~1";
    const to = this.toInput.value.trim() || "HEAD";
    const depth = Number(this.depthInput.value) || 3;
    this.runButton.disabled = true;
    this.runButton.textContent = "Analyzing\u2026";
    this.resultsEl.replaceChildren(el("div", { class: "spinner-row" }, ["Analyzing blast radius\u2026"]));
    try {
      const report = await this.api.impact(from, to, depth);
      this.renderReport(report);
    } catch (error) {
      this.resultsEl.replaceChildren(
        el("div", { class: "panel-error" }, [error instanceof ApiError ? error.message : "Impact analysis failed"]),
      );
    } finally {
      this.runButton.disabled = false;
      this.runButton.textContent = "Analyze impact";
    }
  }

  private renderReport(report: {
    from: string;
    to: string;
    risk_score: number;
    changed_files: string[];
    changed_symbols: ImpactNode[];
    affected: ImpactNode[];
    warnings: string[];
  }): void {
    const riskClass = report.risk_score >= 70 ? "risk-high" : report.risk_score >= 35 ? "risk-medium" : "risk-low";
    const header = el("div", { class: "impact-header" }, [
      el("div", { class: `risk-gauge ${riskClass}` }, [
        el("span", { class: "risk-value" }, [String(report.risk_score)]),
        el("span", { class: "risk-label" }, ["risk score"]),
      ]),
      el("div", { class: "impact-summary" }, [
        el("div", {}, [`${report.from} \u2192 ${report.to}`]),
        el("div", { class: "muted" }, [
          `${report.changed_files.length} file(s) changed \u00b7 ${report.changed_symbols.length} symbol(s) changed \u00b7 ${report.affected.length} affected`,
        ]),
      ]),
    ]);

    const warningsEl =
      report.warnings.length > 0
        ? el(
            "div",
            { class: "banner banner-warning" },
            report.warnings.map((w) => el("div", {}, [w])),
          )
        : el("div", {});

    const filesList =
      report.changed_files.length > 0
        ? el(
            "ul",
            { class: "plain-list" },
            report.changed_files.map((f) => el("li", {}, [el("code", {}, [f])])),
          )
        : el("p", { class: "muted" }, ["No file changes detected between these refs."]);

    this.resultsEl.replaceChildren(
      header,
      warningsEl,
      el("div", { class: "field" }, [el("div", { class: "field-label" }, ["Changed files"]), filesList]),
      this.nodeSection("Changed symbols", report.changed_symbols),
      this.nodeSection("Affected (blast radius)", report.affected),
    );
  }

  private nodeSection(title: string, nodes: ImpactNode[]): HTMLElement {
    if (nodes.length === 0) {
      return el("div", { class: "field" }, [
        el("div", { class: "field-label" }, [title]),
        el("p", { class: "muted" }, ["None."]),
      ]);
    }
    const rows = nodes.map((node) =>
      el(
        "li",
        { class: `impact-node depth-${Math.min(node.depth, 4)}${node.symbol_id ? " is-clickable" : ""}` },
        [
          el("span", { class: `kind-dot kind-${node.kind}` }),
          el("code", {}, [node.symbol]),
          el("span", { class: "muted small" }, [` ${node.path} \u00b7 depth ${node.depth} \u00b7 ${node.reason}`]),
        ],
      ),
    );
    for (const [index, node] of nodes.entries()) {
      if (node.symbol_id) {
        rows[index]?.addEventListener("click", () => this.onFocusSymbol(node.symbol_id));
      }
    }
    return el("div", { class: "field" }, [
      el("div", { class: "field-label" }, [`${title} (${nodes.length})`]),
      el("ul", { class: "impact-list" }, rows),
    ]);
  }
}
