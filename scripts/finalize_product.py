#!/usr/bin/env python3
"""Apply the final dashboard and public-site product polish deterministically."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace_once(path: Path, old: str, new: str) -> None:
    content = path.read_text(encoding="utf-8")
    count = content.count(old)
    if count != 1:
        raise SystemExit(f"expected one match in {path}, found {count}")
    path.write_text(content.replace(old, new, 1), encoding="utf-8")


def patch_dashboard_html() -> None:
    path = ROOT / "src/dashboard.rs"
    replace_once(path, '<div class="repo-avatar">R</div>', '<div class="repo-avatar" id="repositoryInitial">R</div>')

    old = '''                <button class="quick-action" data-quick-tab="impact"><span class="quick-action-icon">↗</span><span><strong>Analyze a change</strong><small>Estimate propagation and risk</small></span><span>→</span></button>
              </div>
            </article>

            <article class="surface metric-card"><div class="metric-top"><span>Files</span><span class="metric-icon">▦</span></div><strong id="metricFiles">0</strong><small>indexed repository files</small></article>'''
    new = '''                <button class="quick-action" data-quick-tab="impact"><span class="quick-action-icon">↗</span><span><strong>Analyze a change</strong><small>Estimate propagation and risk</small></span><span>→</span></button>
              </div>
            </article>

            <article class="surface architecture-panel">
              <div class="surface-header"><div><h3>Architecture pulse</h3><p>Most connected files in the active repository</p></div><button class="button ghost small" data-quick-tab="graph">Open full map</button></div>
              <div class="surface-body architecture-overview">
                <div id="overviewRepositoryMap" class="topology-map"><div class="empty-inline">Loading repository topology…</div></div>
                <div class="runtime-status-grid">
                  <div class="runtime-status"><span>AI runtime</span><strong id="aiRuntimeStatus">Checking…</strong><small>Repository-aware local inference</small></div>
                  <div class="runtime-status"><span>Skills</span><strong id="skillsRuntimeStatus">Checking…</strong><small>Controlled engineering capabilities</small></div>
                  <div class="runtime-status"><span>MCP</span><strong id="mcpRuntimeStatus">Checking…</strong><small>Verified local tool servers</small></div>
                  <div class="runtime-status"><span>GitHub</span><strong id="githubRuntimeStatus">Checking…</strong><small>Optional delivery integration</small></div>
                </div>
              </div>
            </article>

            <article class="surface metric-card"><div class="metric-top"><span>Files</span><span class="metric-icon">▦</span></div><strong id="metricFiles">0</strong><small>indexed repository files</small></article>'''
    replace_once(path, old, new)

    old = '''                <div class="health-row" data-tone="warning"><span>Semantic index</span><strong id="indexHealthText">Checking…</strong></div>
                <div class="health-row" data-tone="success"><span>Source boundary</span><strong>Local filesystem</strong></div>
                <div class="health-row" data-tone="success"><span>API exposure</span><strong>127.0.0.1 only</strong></div>
                <div class="health-row"><span>Live synchronization</span><strong>SSE + health polling</strong></div>'''
    new = '''                <div class="health-row" data-tone="warning"><span>Semantic index</span><strong id="indexHealthText">Checking…</strong></div>
                <div class="health-row"><span>AI runtime</span><strong id="aiHealthText">Checking…</strong></div>
                <div class="health-row"><span>Skills registry</span><strong id="skillsHealthText">Checking…</strong></div>
                <div class="health-row"><span>MCP servers</span><strong id="mcpHealthText">Checking…</strong></div>
                <div class="health-row"><span>GitHub delivery</span><strong id="githubHealthText">Checking…</strong></div>
                <div class="health-row" data-tone="success"><span>API exposure</span><strong>127.0.0.1 only</strong></div>'''
    replace_once(path, old, new)


def patch_dashboard_typescript() -> None:
    path = ROOT / "dashboard/src/main.ts"
    replace_once(
        path,
        '''    this.renderOverview();
    this.renderWorkspaceSwitcher();
    if (this.graph) this.renderGraph(this.graph);''',
        '''    this.renderOverview();
    this.renderWorkspaceSwitcher();
    void this.loadOverviewSubsystems();
    if (this.graph) this.renderGraph(this.graph);''',
    )

    old = '''    $("#indexHealthText").textContent = graph && graph.files.length > 0 ? "Index ready" : "Index requires attention";
    $("#indexHealthText").parentElement?.setAttribute("data-tone", graph && graph.files.length > 0 ? "success" : "warning");
  }

  private renderWorkspaceSwitcher(): void {
    const select = $("#workspaceSelect") as HTMLSelectElement;
    const workspaces = this.workspaces?.workspaces ?? [];
    select.replaceChildren();
    if (workspaces.length === 0) {
      select.append(new Option("Current directory", ""));
      return;
    }
    workspaces.forEach((workspace) => select.append(new Option(workspace.name, workspace.id, false, workspace.active)));
  }'''
    new = '''    $("#indexHealthText").textContent = graph && graph.files.length > 0 ? "Index ready" : "Index requires attention";
    $("#indexHealthText").parentElement?.setAttribute("data-tone", graph && graph.files.length > 0 ? "success" : "warning");
    const repositoryName = active?.name ?? graph?.project_root.split(/[\\\\/]/).pop() ?? "Repository";
    $("#repositoryInitial").textContent = repositoryName.slice(0, 2).toUpperCase();
    this.renderOverviewTopology(graph);
  }

  private renderOverviewTopology(graph: GraphSnapshot | null): void {
    const root = $("#overviewRepositoryMap");
    if (!graph || graph.files.length === 0) {
      root.innerHTML = `<div class="empty-state compact"><span class="empty-icon">◇</span><strong>No topology yet</strong><span>Update the index to build the repository map.</span></div>`;
      return;
    }
    const symbolFiles = new Map(graph.symbols.map((symbol) => [symbol.id, symbol.file_id]));
    const degree = new Map(graph.files.map((file) => [file.id, 0]));
    graph.edges.forEach((edge) => {
      const fromFile = symbolFiles.get(edge.from);
      const toFile = symbolFiles.get(edge.to);
      if (fromFile === undefined || toFile === undefined || fromFile === toFile) return;
      degree.set(fromFile, (degree.get(fromFile) ?? 0) + 1);
      degree.set(toFile, (degree.get(toFile) ?? 0) + 1);
    });
    const topFiles = graph.files
      .map((file) => ({ file, degree: degree.get(file.id) ?? 0 }))
      .sort((left, right) => right.degree - left.degree || right.file.line_count - left.file.line_count)
      .slice(0, 8);
    const maxDegree = Math.max(1, ...topFiles.map((entry) => entry.degree));
    root.innerHTML = topFiles.map((entry, index) => {
      const name = entry.file.path.split("/").pop() ?? entry.file.path;
      const activity = Math.max(12, Math.round((entry.degree / maxDegree) * 100));
      return `<button class="topology-node topology-node-${index + 1}" type="button" data-overview-file="${entry.file.id}" style="--activity:${activity}%"><span class="topology-node-language lang-${escapeHtml(entry.file.language.toLowerCase().replace(/[^a-z0-9]+/g, "-"))}"></span><strong>${escapeHtml(name)}</strong><small>${escapeHtml(entry.file.language || "text")} · ${entry.degree} links</small><i></i></button>`;
    }).join("");
    $$<HTMLButtonElement>("[data-overview-file]", root).forEach((button) => button.addEventListener("click", () => {
      const file = graph.files.find((candidate) => candidate.id === Number(button.dataset.overviewFile));
      if (!file) return;
      this.switchTab("graph");
      const filter = $("#graphFilter") as HTMLInputElement;
      filter.value = file.path;
      this.graphView.setFilter(file.path);
    }));
  }

  private async loadOverviewSubsystems(): Promise<void> {
    const [skillsResult, mcpResult, settingsResult, githubResult] = await Promise.allSettled([
      this.api.skills(),
      this.api.mcp(),
      this.api.settings(),
      this.api.githubStatus(),
    ]);
    const setStatus = (primaryId: string, healthId: string, label: string, tone: "success" | "warning" | "neutral"): void => {
      $(primaryId).textContent = label;
      $(healthId).textContent = label;
      const healthRow = $(healthId).parentElement;
      if (tone === "neutral") healthRow?.removeAttribute("data-tone");
      else healthRow?.setAttribute("data-tone", tone);
    };

    if (skillsResult.status === "fulfilled") {
      const enabled = skillsResult.value.skills.filter((skill) => skill.enabled).length;
      const total = skillsResult.value.skills.length;
      setStatus("#skillsRuntimeStatus", "#skillsHealthText", `${enabled}/${total} enabled`, enabled > 0 ? "success" : "warning");
    } else {
      setStatus("#skillsRuntimeStatus", "#skillsHealthText", "Unavailable", "warning");
    }

    if (mcpResult.status === "fulfilled") {
      const running = mcpResult.value.servers.filter((server) => server.status.toLowerCase() === "running").length;
      const total = mcpResult.value.servers.length;
      const label = total === 0 ? "No servers" : `${running}/${total} running`;
      setStatus("#mcpRuntimeStatus", "#mcpHealthText", label, running > 0 ? "success" : "neutral");
    } else {
      setStatus("#mcpRuntimeStatus", "#mcpHealthText", "Unavailable", "warning");
    }

    if (settingsResult.status === "fulfilled") {
      const effective = settingsResult.value.effective;
      const model = text(effective["ollama_model"] ?? effective["ai.model"] ?? effective["model"]);
      const label = model || "Local Ollama";
      setStatus("#aiRuntimeStatus", "#aiHealthText", label, model ? "success" : "neutral");
    } else {
      setStatus("#aiRuntimeStatus", "#aiHealthText", "Not configured", "warning");
    }

    if (githubResult.status === "fulfilled") {
      const status = githubResult.value;
      const identity = text(status["username"] ?? status["login"] ?? status["user"]);
      const connected = Boolean(status["connected"] ?? status["authenticated"] ?? identity);
      setStatus("#githubRuntimeStatus", "#githubHealthText", connected ? identity || "Connected" : "Optional", connected ? "success" : "neutral");
    } else {
      setStatus("#githubRuntimeStatus", "#githubHealthText", "Optional", "neutral");
    }
  }

  private renderWorkspaceSwitcher(): void {
    const select = $("#workspaceSelect") as HTMLSelectElement;
    const workspaces = this.workspaces?.workspaces ?? [];
    select.replaceChildren();
    if (workspaces.length === 0) {
      const projectRoot = this.graph?.project_root ?? "";
      const name = projectRoot.split(/[\\\\/]/).pop() || "Current directory";
      select.append(new Option(name, "", true, true));
      select.title = projectRoot || name;
      return;
    }
    workspaces.forEach((workspace) => select.append(new Option(workspace.name, workspace.id, false, workspace.active)));
    select.title = workspaces.find((workspace) => workspace.active)?.path ?? "Active repository";
  }'''
    replace_once(path, old, new)


def patch_dashboard_css() -> None:
    path = ROOT / "dashboard/dist/dashboard.css"
    content = path.read_text(encoding="utf-8")
    marker = "/* product-finish-v1 */"
    if marker in content:
      raise SystemExit("dashboard product styles already present")
    addition = r'''

/* product-finish-v1 */
.architecture-panel { grid-column: span 12; overflow: hidden; }
.architecture-panel .surface-header { align-items: center; }
.architecture-overview { display: grid; grid-template-columns: minmax(0, 1.65fr) minmax(270px, .75fr); gap: 14px; }
.topology-map {
  position: relative;
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 11px;
  min-height: 214px;
  padding: 18px;
  overflow: hidden;
  border: 1px solid var(--line);
  border-radius: 12px;
  background:
    radial-gradient(circle at 50% 48%, rgba(86,217,245,.1), transparent 34%),
    radial-gradient(rgba(148,163,184,.14) .7px, transparent .7px),
    rgba(7,9,15,.42);
  background-size: auto, 18px 18px, auto;
}
.topology-map::before,
.topology-map::after { content: ""; position: absolute; height: 1px; left: 8%; right: 8%; background: linear-gradient(90deg, transparent, rgba(86,217,245,.22), transparent); transform-origin: center; pointer-events: none; }
.topology-map::before { top: 42%; transform: rotate(8deg); }
.topology-map::after { top: 64%; transform: rotate(-7deg); }
.topology-node {
  position: relative;
  z-index: 1;
  align-self: center;
  min-width: 0;
  min-height: 74px;
  padding: 12px 11px 17px;
  overflow: hidden;
  border: 1px solid rgba(148,163,184,.2);
  border-radius: 10px;
  background: linear-gradient(150deg, rgba(23,33,50,.94), rgba(12,17,27,.96));
  box-shadow: 0 10px 24px rgba(0,0,0,.2);
  text-align: left;
  transition: 150ms ease;
}
.topology-node:hover { transform: translateY(-2px); border-color: rgba(86,217,245,.48); box-shadow: 0 14px 30px rgba(0,0,0,.28); }
.topology-node strong { display: block; overflow: hidden; padding-right: 8px; font: 600 10px/1.35 var(--font-mono); text-overflow: ellipsis; white-space: nowrap; }
.topology-node small { display: block; margin-top: 6px; overflow: hidden; color: var(--muted); font-size: 8px; text-overflow: ellipsis; white-space: nowrap; }
.topology-node i { position: absolute; left: 0; right: auto; bottom: 0; width: var(--activity); height: 3px; background: linear-gradient(90deg, var(--cyan), var(--violet)); box-shadow: 0 0 13px rgba(86,217,245,.38); }
.topology-node-language { position: absolute; top: 10px; right: 9px; width: 7px; height: 7px; border-radius: 50%; background: var(--muted-2); }
.topology-node-2, .topology-node-7 { transform: translateY(12px); }
.topology-node-3, .topology-node-6 { transform: translateY(-8px); }
.topology-node-2:hover, .topology-node-7:hover { transform: translateY(10px); }
.topology-node-3:hover, .topology-node-6:hover { transform: translateY(-10px); }
.runtime-status-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; }
.runtime-status { min-width: 0; padding: 13px; border: 1px solid var(--line); border-radius: 11px; background: rgba(7,9,15,.34); }
.runtime-status span { display: block; color: var(--muted); font-size: 9px; font-weight: 700; letter-spacing: .08em; text-transform: uppercase; }
.runtime-status strong { display: block; margin-top: 9px; overflow: hidden; color: var(--text); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
.runtime-status small { display: block; margin-top: 6px; color: var(--muted-2); font-size: 8px; line-height: 1.4; }
.health-panel { grid-column: span 6; }
.languages-panel { grid-column: span 3; }
.delivery-panel { grid-column: span 3; }
@media (max-width: 1100px) {
  .architecture-overview { grid-template-columns: 1fr; }
  .topology-map { min-height: 190px; }
  .health-panel, .languages-panel, .delivery-panel { grid-column: span 12; }
}
@media (max-width: 760px) {
  .topology-map { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .runtime-status-grid { grid-template-columns: 1fr; }
}
'''
    path.write_text(content.rstrip() + addition + "\n", encoding="utf-8")


def patch_site() -> None:
    path = ROOT / "site/index.html"
    replacements = [
        (
            '<meta name="description" content="CodeSpace is a local-first semantic code graph and compact context engine for AI coding agents.">',
            '<meta name="description" content="CodeSpace is a local-first repository intelligence platform with a semantic graph, compact AI context, impact analysis, MCP, skills, tasks, memory, and a localhost dashboard.">',
        ),
        (
            '<p class="hero-lede">CodeSpace builds a live semantic graph of your repository, then returns precise, bounded context instead of dumping the whole codebase into the model.</p>',
            '<p class="hero-lede">CodeSpace builds a live semantic graph of your repository and turns it into a complete local control plane: precise AI context, impact analysis, skills, MCP servers, tasks, decisions, and a production-grade localhost dashboard.</p>',
        ),
        (
            '''          <div><dt>Local-first</dt><dd>No cloud required</dd></div>
          <div><dt>5 tools</dt><dd>Minimal MCP surface</dd></div>
          <div><dt>1 binary</dt><dd>CLI · MCP · REST</dd></div>''',
            '''          <div><dt>Local-first</dt><dd>Source stays on your machine</dd></div>
          <div><dt>12 MCP tools</dt><dd>Verified agent interface</dd></div>
          <div><dt>1 binary</dt><dd>CLI · Dashboard · MCP · REST</dd></div>''',
        ),
        (
            '<span>Semantic graph</span><i></i><span>Context compaction</span><i></i><span>Blast-radius analysis</span><i></i><span>Decision memory</span><i></i><span>MCP-native</span>',
            '<span>Semantic graph</span><i></i><span>Context compaction</span><i></i><span>Blast-radius analysis</span><i></i><span>Skills & MCP control</span><i></i><span>Local dashboard</span>',
        ),
        (
            '<h2>One engine. Four hard problems solved.</h2>',
            '<h2>One engine. A complete local control plane.</h2>',
        ),
        (
            '''          <article><span>CLI</span><strong>cse</strong><small>Fast shell workflows</small></article>
          <article><span>MCP</span><strong>5 focused tools</strong><small>IDE and agent clients</small></article>
          <article><span>REST</span><strong>Loopback API</strong><small>Local integrations</small></article>
          <article><span>RUST</span><strong>Library crate</strong><small>Embed the engine</small></article>''',
            '''          <article><span>CLI</span><strong>cse</strong><small>Fast shell workflows</small></article>
          <article><span>MCP</span><strong>12 verified tools</strong><small>IDE and agent clients</small></article>
          <article><span>UI</span><strong>Local dashboard</strong><small>Graph, tasks, skills, servers</small></article>
          <article><span>API</span><strong>REST + Rust crate</strong><small>Local integrations and embedding</small></article>''',
        ),
        (
            'data-copy="cargo install --git https://github.com/IvanChernykh/CodeSpace\ncse init\ncse context --query &quot;authentication returns 500&quot;"',
            'data-copy="cargo install --git https://github.com/IvanChernykh/CodeSpace\ncse init\ncse dashboard\ncse context --query &quot;authentication returns 500&quot;"',
        ),
        (
            '''<span class="code-comment"># Index and retrieve</span>
 cse init
 cse context --query <span class="code-string">"authentication returns 500"</span>''',
            '''<span class="code-comment"># Index, open the control plane, and retrieve</span>
 cse init
 cse dashboard
 cse context --query <span class="code-string">"authentication returns 500"</span>''',
        ),
    ]
    for old, new in replacements:
        replace_once(path, old, new)


def main() -> None:
    patch_dashboard_html()
    patch_dashboard_typescript()
    patch_dashboard_css()
    patch_site()
    obsolete = ROOT / "scripts/render_dashboard.py"
    if obsolete.exists():
        obsolete.unlink()
    else:
        raise SystemExit("obsolete render_dashboard.py was already absent")
    print("final product polish applied")


if __name__ == "__main__":
    main()
