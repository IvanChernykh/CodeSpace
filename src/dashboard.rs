use crate::util::json_escape;

pub fn render_dashboard(token: &str) -> String {
    let safe_token = json_escape(token);
    format!(
        r##"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">
  <meta name="color-scheme" content="dark">
  <meta name="theme-color" content="#07090f">
  <meta name="cse-session" content="{safe_token}">
  <title>CodeSpace — IDE Assistant</title>
  <link rel="stylesheet" href="/assets/dashboard.css">
</head>
<body>
  <div class="app-shell">
    <aside class="sidebar-nav" aria-label="Primary navigation">
      <div class="brand">
        <div class="brand-mark">CS</div>
        <div class="brand-copy"><strong>CodeSpace</strong><span>IDE Assistant</span></div>
      </div>
      <div class="nav-scroll">
        <div class="nav-section">
          <span class="nav-label">Workspace</span>
          <button class="nav-item is-active" data-nav="overview"><span class="nav-icon">⌂</span><span>Command Center</span></button>
          <button class="nav-item" data-nav="graph"><span class="nav-icon">⌘</span><span>Repository Map</span></button>
          <button class="nav-item" data-nav="context"><span class="nav-icon">{{ }}</span><span>Context Builder</span></button>
          <button class="nav-item" data-nav="impact"><span class="nav-icon">↗</span><span>Impact Analysis</span></button>
          <button class="nav-item" data-nav="history"><span class="nav-icon">◷</span><span>Decision Memory</span></button>
        </div>
        <div class="nav-section">
          <span class="nav-label">Assistant</span>
          <button class="nav-item" data-nav="assistant"><span class="nav-icon">✦</span><span>IDE Assistant</span></button>
          <button class="nav-item" data-nav="tasks"><span class="nav-icon">✓</span><span>Engineering Tasks</span></button>
          <button class="nav-item" data-nav="skills"><span class="nav-icon">◇</span><span>Skills</span></button>
          <button class="nav-item" data-nav="mcp"><span class="nav-icon">⇄</span><span>MCP Control</span></button>
        </div>
        <div class="nav-section">
          <span class="nav-label">System</span>
          <button class="nav-item" data-nav="workspaces"><span class="nav-icon">▦</span><span>Repositories</span></button>
          <button class="nav-item" data-nav="github"><span class="nav-icon">GH</span><span>GitHub</span></button>
          <button class="nav-item" data-nav="settings"><span class="nav-icon">⚙</span><span>Settings</span></button>
        </div>
      </div>
      <div class="sidebar-footer">
        <div class="runtime-row"><span>Runtime</span><code id="runtimeVersion">v—</code></div>
        <div class="local-only">Localhost only</div>
      </div>
    </aside>

    <div class="main-shell">
      <header class="topbar">
        <div class="page-heading"><h1 id="pageTitle">Command Center</h1><p id="pageSubtitle">Runtime, index, and repository health</p></div>
        <label class="global-search">
          <input id="globalSearch" type="search" autocomplete="off" placeholder="Find a file, symbol, action…">
          <span class="keycap">Ctrl K</span>
        </label>
        <div class="topbar-actions">
          <select id="workspaceSelect" class="workspace-select" aria-label="Active repository"><option>Current directory</option></select>
          <button id="doctorBtn" class="button ghost">Doctor</button>
          <button id="updateIndexBtn" class="button primary">Update Index</button>
          <div class="connection-wrap">
            <div id="connectionChip" class="connection-chip" data-state="starting"><span id="connectionLabel">Starting</span></div>
            <div id="connectionDetail" class="connection-detail">Starting local runtime</div>
          </div>
        </div>
      </header>

      <main class="content">
        <section class="view is-active" data-view="overview">
          <div class="overview-grid">
            <article class="surface hero-repository">
              <div class="hero-copy">
                <span class="eyebrow">Local-first repository intelligence</span>
                <h2>Understand the codebase as a <span>connected system</span>, not a folder tree.</h2>
                <p>CodeSpace unifies the CLI, localhost dashboard, MCP tools, skills, decisions, tasks, and local AI around one indexed repository state.</p>
                <div class="repository-identity">
                  <div class="repo-avatar" id="repositoryInitial">R</div>
                  <div><strong id="activeRepositoryName">Current directory</strong><code id="activeRepositoryPath">Loading active repository…</code></div>
                </div>
              </div>
            </article>

            <article class="surface quick-actions">
              <div class="surface-header"><div><h3>Start here</h3><p>Common engineering workflows</p></div></div>
              <div class="surface-body">
                <button class="quick-action" data-quick-tab="graph"><span class="quick-action-icon">⌘</span><span><strong>Explore repository</strong><small>Open the file dependency map</small></span><span>→</span></button>
                <button class="quick-action" data-quick-tab="context"><span class="quick-action-icon">{{ }}</span><span><strong>Build AI context</strong><small>Retrieve only relevant source</small></span><span>→</span></button>
                <button class="quick-action" data-quick-tab="impact"><span class="quick-action-icon">↗</span><span><strong>Analyze a change</strong><small>Estimate propagation and risk</small></span><span>→</span></button>
              </div>
            </article>

            <article class="surface architecture-panel">
              <div class="surface-header"><div><h3>Architecture pulse</h3><p>Most connected files in the active repository</p></div><button class="button ghost small" data-quick-tab="graph">Open full map</button></div>
              <div class="surface-body architecture-overview">
                <div id="overviewRepositoryMap" class="topology-map"><div class="empty-inline">Loading repository topology…</div></div>
                <div class="runtime-status-grid">
                  <div class="runtime-status"><span>AI runtime</span><strong id="aiRuntimeStatus">Local Ollama</strong><small>Repository-aware local inference</small></div>
                  <div class="runtime-status"><span>Skills</span><strong id="skillsRuntimeStatus">Built-in registry</strong><small>Controlled engineering capabilities</small></div>
                  <div class="runtime-status"><span>MCP</span><strong id="mcpRuntimeStatus">No servers</strong><small>Verified local tool servers</small></div>
                  <div class="runtime-status"><span>GitHub</span><strong id="githubRuntimeStatus">Optional</strong><small>Optional delivery integration</small></div>
                </div>
              </div>
            </article>

            <article class="surface metric-card"><div class="metric-top"><span>Files</span><span class="metric-icon">▦</span></div><strong id="metricFiles">0</strong><small>indexed repository files</small></article>
            <article class="surface metric-card"><div class="metric-top"><span>Symbols</span><span class="metric-icon">ƒ</span></div><strong id="metricSymbols">0</strong><small>functions, types, modules, tests</small></article>
            <article class="surface metric-card"><div class="metric-top"><span>Relationships</span><span class="metric-icon">⇄</span></div><strong id="metricEdges">0</strong><small>calls, imports, dependencies</small></article>
            <article class="surface metric-card"><div class="metric-top"><span>Revision</span><span class="metric-icon">#</span></div><strong id="metricRevision">r0</strong><small id="metricUpdated">not indexed</small></article>

            <article class="surface health-panel">
              <div class="surface-header"><div><h3>System health</h3><p>Operational status of the local assistant</p></div></div>
              <div class="surface-body health-list">
                <div class="health-row" data-tone="warning"><span>Semantic index</span><strong id="indexHealthText">Checking…</strong></div>
                <div class="health-row"><span>AI runtime</span><strong id="aiHealthText">Local Ollama</strong></div>
                <div class="health-row"><span>Skills registry</span><strong id="skillsHealthText">Built-in registry</strong></div>
                <div class="health-row"><span>MCP servers</span><strong id="mcpHealthText">No servers</strong></div>
                <div class="health-row"><span>GitHub delivery</span><strong id="githubHealthText">Optional</strong></div>
                <div class="health-row" data-tone="success"><span>API exposure</span><strong>127.0.0.1 only</strong></div>
              </div>
            </article>
            <article class="surface languages-panel">
              <div class="surface-header"><div><h3>Languages</h3><p>Indexed file distribution</p></div></div>
              <div id="languageBreakdown" class="surface-body"></div>
            </article>
            <article class="surface delivery-panel">
              <div class="surface-header"><div><h3>Interfaces</h3><p>One core, multiple clients</p></div></div>
              <div class="surface-body delivery-stack">
                <div class="delivery-item"><span>Terminal</span><strong>cse CLI</strong></div>
                <div class="delivery-item"><span>IDE agents</span><strong>MCP tools</strong></div>
                <div class="delivery-item"><span>Browser</span><strong>localhost dashboard</strong></div>
              </div>
            </article>
          </div>
        </section>

        <section class="view graph-view" data-view="graph">
          <div class="graph-layout">
            <div class="graph-stage">
              <div class="graph-toolbar">
                <label class="search-field"><input id="graphFilter" type="search" placeholder="Filter files or languages…"></label>
                <button id="graphFitBtn" class="button ghost small">Fit</button>
                <button id="graphReloadBtn" class="button ghost small">Reload</button>
                <div class="toolbar-meta"><span id="graphVisibleCount" class="meta-chip">0 files</span><span id="graphEdgeCount" class="meta-chip">0 links</span><span id="graphRevision" class="meta-chip">revision 0</span></div>
              </div>
              <div class="graph-canvas">
                <div id="graphEmpty" class="graph-empty"><strong>Building repository map</strong><span>Loading indexed files and relationships…</span></div>
                <svg id="fileGraphSvg" class="is-hidden" xmlns="http://www.w3.org/2000/svg" aria-label="Repository file dependency network"></svg>
              </div>
            </div>
            <aside id="fileInspector" class="graph-inspector"><div class="empty-state compact"><span class="empty-icon">◇</span><strong>Select a file</strong><span>Inspect symbols and relationships.</span></div></aside>
          </div>
        </section>

        <section class="view" data-view="context">
          <div class="panel-page">
            <div class="panel-intro"><div><span class="eyebrow">Retrieval</span><h2>Context Builder</h2><p>Create a compact source bundle for IDE agents and local models. Results are bounded by token and item limits and pass through secret redaction.</p></div></div>
            <form id="contextForm" class="surface surface-body form-grid">
              <div class="field wide"><label for="contextQuery">Question, symbol, or file</label><input id="contextQuery" type="text" placeholder="How does workspace selection affect graph loading?" required></div>
              <div class="field"><label for="contextTokens">Token budget</label><input id="contextTokens" type="number" min="100" max="32000" value="1600"></div>
              <div class="field"><label for="contextItems">Maximum items</label><input id="contextItems" type="number" min="1" max="50" value="10"></div>
              <div class="form-actions"><button class="button primary" type="submit">Build Context</button></div>
            </form>
            <div id="contextResults" class="results"></div>
          </div>
        </section>

        <section class="view" data-view="impact">
          <div class="panel-page">
            <div class="panel-intro"><div><span class="eyebrow">Change intelligence</span><h2>Impact Analysis</h2><p>Compare Git revisions and trace potentially affected symbols through the repository graph before making or merging a change.</p></div></div>
            <form id="impactForm" class="surface surface-body form-grid">
              <div class="field"><label for="impactFrom">From revision</label><input id="impactFrom" value="HEAD~1"></div>
              <div class="field"><label for="impactTo">To revision</label><input id="impactTo" value="HEAD"></div>
              <div class="field"><label for="impactDepth">Traversal depth</label><input id="impactDepth" type="number" min="1" max="10" value="3"></div>
              <div class="form-actions"><button class="button primary" type="submit">Analyze Impact</button></div>
            </form>
            <div id="impactResults" class="results"></div>
          </div>
        </section>

        <section class="view assistant-view" data-view="assistant">
          <div class="assistant-layout">
            <div class="chat-shell">
              <div id="assistantMessages" class="chat-messages">
                <div class="chat-welcome">
                  <div class="assistant-orb">✦</div>
                  <h2>Repository-aware local assistant</h2>
                  <p>Ask about architecture, debugging, refactoring, security, tests, and implementation. The backend supplies indexed repository context to the local model.</p>
                  <div class="prompt-grid">
                    <button class="prompt-card" type="button">Explain the current architecture and its weakest boundary.</button>
                    <button class="prompt-card" type="button">Find likely security risks in the local HTTP server.</button>
                    <button class="prompt-card" type="button">Plan a safe refactor for the dashboard state layer.</button>
                    <button class="prompt-card" type="button">Identify missing tests for workspace synchronization.</button>
                  </div>
                </div>
              </div>
              <form id="assistantForm" class="chat-composer">
                <div class="composer-box"><textarea id="assistantInput" placeholder="Ask CodeSpace about this repository…" required></textarea><button class="button primary" type="submit">Send</button></div>
                <div class="composer-footer"><span>Local Ollama endpoint · source stays on this machine</span><input id="assistantModel" value="qwen2.5-coder:1.5b-instruct-q4_K_M" aria-label="Ollama model"></div>
              </form>
            </div>
            <aside class="assistant-context">
              <span class="eyebrow">Active context</span>
              <div class="context-card"><span>Repository</span><strong id="assistantRepository">Selected workspace</strong><p>Uses the same active root as CLI and MCP.</p></div>
              <div class="context-card"><span>Index policy</span><strong>Secret-redacted</strong><p>Generated and vendor directories are excluded.</p></div>
              <div class="context-card"><span>Model runtime</span><strong>Ollama localhost</strong><p>Configure the model in the composer footer.</p></div>
            </aside>
          </div>
        </section>

        <section class="view" data-view="tasks">
          <div class="panel-page">
            <div class="panel-intro"><div><span class="eyebrow">Execution</span><h2>Engineering Tasks</h2><p>Track work next to repository state. Task mutations are available through the same local API used by the CLI and dashboard.</p></div></div>
            <form id="taskForm" class="surface surface-body form-grid task-controls">
              <div class="field"><label>Title</label><input name="title" placeholder="Fix workspace root resolution" required></div>
              <div class="field wide"><label>Description</label><input name="description" placeholder="Expected behavior and acceptance criteria"></div>
              <div class="field"><label>Priority</label><select name="priority"><option>low</option><option selected>medium</option><option>high</option><option>critical</option></select></div>
              <div class="field"><label>Tags</label><input name="tags" placeholder="backend,stability"></div>
              <div class="form-actions"><button class="button primary" type="submit">Create Task</button></div>
            </form>
            <div id="taskBoard" class="task-board"></div>
          </div>
        </section>

        <section class="view" data-view="history">
          <div class="panel-page">
            <div class="panel-intro"><div><span class="eyebrow">Durable project memory</span><h2>Decision Memory</h2><p>Record why architecture and implementation choices were made, then retrieve them through CLI, MCP, and the dashboard.</p></div></div>
            <div class="decision-layout">
              <div>
                <form id="historySearchForm" class="surface surface-body form-grid"><div class="field wide"><label>Search decisions</label><input id="historyQuery" placeholder="workspace, security, dashboard…"></div><div class="form-actions"><button class="button ghost" type="submit">Search</button></div></form>
                <div id="historyResults" class="results"></div>
              </div>
              <form id="rememberForm" class="surface surface-body form-grid">
                <div class="field full"><label>Decision summary</label><textarea name="summary" placeholder="What was decided?" required></textarea></div>
                <div class="field full"><label>Rationale</label><textarea name="rationale" placeholder="Why was this choice made?"></textarea></div>
                <div class="field full"><label>File</label><input name="file" placeholder="src/server.rs"></div>
                <div class="field full"><label>Symbol</label><input name="symbol" placeholder="handle_connection"></div>
                <div class="field full"><label>Tags</label><input name="tags" placeholder="architecture,security"></div>
                <div class="form-actions"><button class="button primary" type="submit">Remember</button></div>
              </form>
            </div>
          </div>
        </section>

        <section class="view" data-view="workspaces">
          <div class="panel-page">
            <div class="panel-intro"><div><span class="eyebrow">Workspace manager</span><h2>Repositories</h2><p>Register and switch local repositories. Every dashboard API request resolves the active workspace before executing a core action.</p></div></div>
            <div id="workspaceList" class="workspace-list"></div>
            <form id="workspaceForm" class="surface surface-body form-grid" style="margin-top:14px">
              <div class="field wide"><label>Local directory path</label><input name="path" placeholder="/home/user/projects/service" required></div>
              <div class="field"><label>Display name</label><input name="name" placeholder="Service API"></div>
              <div class="form-actions"><button class="button primary" type="submit">Register Repository</button></div>
            </form>
          </div>
        </section>

        <section class="view" data-view="skills">
          <div class="panel-page">
            <div class="panel-intro"><div><span class="eyebrow">Capability platform</span><h2>Skills</h2><p>Enable trusted, permission-scoped capabilities for security, design, engineering, testing, documentation, and repository analysis.</p></div><div class="status-pill">Pinned sources only</div></div>
            <div id="skillsGrid" class="skills-grid"></div>
          </div>
        </section>

        <section class="view" data-view="mcp">
          <div class="panel-page">
            <div class="panel-intro"><div><span class="eyebrow">Protocol control plane</span><h2>MCP Control</h2><p>Register, start, stop, and inspect local MCP server processes. Environment values remain server-side and are never returned to the browser.</p></div></div>
            <div id="mcpList" class="mcp-list"></div>
            <form id="mcpForm" class="surface surface-body form-grid" style="margin-top:14px">
              <div class="field"><label>Name</label><input name="name" placeholder="filesystem-tools" required></div>
              <div class="field"><label>Command</label><input name="command" placeholder="npx" required></div>
              <div class="field wide"><label>Arguments</label><input name="args" placeholder="-y @modelcontextprotocol/server-filesystem /workspace"></div>
              <div class="field"><label><input name="auto_start" type="checkbox"> Start automatically</label></div>
              <div class="form-actions"><button class="button primary" type="submit">Register MCP Server</button></div>
            </form>
          </div>
        </section>

        <section class="view" data-view="settings">
          <div class="panel-page">
            <div class="panel-intro"><div><span class="eyebrow">Configuration</span><h2>Settings</h2><p>Workspace settings override global settings. All changes are persisted by the backend and broadcast as synchronization events.</p></div></div>
            <div class="settings-layout">
              <div id="settingsTable" class="surface surface-body"></div>
              <form id="settingForm" class="surface surface-body form-grid">
                <div class="field full"><label>Key</label><input name="key" placeholder="dashboard.graph.max_files" required></div>
                <div class="field full"><label>Value</label><input name="value" placeholder="260"></div>
                <div class="field full"><label>Scope</label><select name="scope"><option value="workspace">workspace</option><option value="global">global</option></select></div>
                <div class="form-actions"><button class="button primary" type="submit">Save Setting</button></div>
              </form>
            </div>
          </div>
        </section>

        <section class="view" data-view="github">
          <div class="panel-page">
            <div class="panel-intro"><div><span class="eyebrow">Delivery integration</span><h2>GitHub</h2><p>Inspect the local GitHub connection without exposing credentials to page JavaScript.</p></div></div>
            <div id="githubPanel"></div>
          </div>
        </section>
      </main>
    </div>
  </div>

  <div id="bootOverlay" class="boot-overlay">
    <div class="boot-card"><div class="boot-logo">CS</div><strong>Starting CodeSpace</strong><span id="bootMessage">Loading local runtime…</span></div>
  </div>
  <div id="toastRoot" aria-live="polite"></div>
  <script type="module" src="/assets/main.js"></script>
</body>
</html>"##
    )
}

pub fn asset(path: &str) -> Option<(&'static str, String)> {
    let name = path.strip_prefix("/assets/")?;
    let (content_type, body) = match name {
        "main.js" => (
            "text/javascript; charset=utf-8",
            include_str!("../dashboard/dist/main.js"),
        ),
        "dashboard.css" => (
            "text/css; charset=utf-8",
            include_str!("../dashboard/dist/dashboard.css"),
        ),
        _ => return None,
    };
    Some((content_type, body.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_has_single_runtime_and_core_views() {
        let html = render_dashboard("test-token");
        assert!(html.contains("Repository Map"));
        assert!(html.contains("MCP Control"));
        assert!(html.contains("data-view=\"skills\""));
        assert!(html.contains("meta name=\"cse-session\""));
        let script_tag = ["<", "script"].concat();
        assert_eq!(html.matches(&script_tag).count(), 1);
        assert!(!html.contains("fetch('/api/v1"));
    }

    #[test]
    fn only_compiled_assets_are_public() {
        assert!(asset("/assets/main.js").is_some());
        assert!(asset("/assets/dashboard.css").is_some());
        assert!(asset("/assets/api.js").is_none());
    }
}
