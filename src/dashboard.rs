pub fn render_dashboard() -> String {
    r##"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">
  <meta name="color-scheme" content="dark">
  <meta name="theme-color" content="#0a0a0a">
  <title>CodeSpace — Local Workbench</title>
  <link rel="stylesheet" href="/assets/dashboard.css">
</head>
<body>
  <div class="app-shell">
    <aside class="sidebar-nav" aria-label="Primary navigation">
      <div class="brand">
        <div class="brand-mark">CS</div>
        <div class="brand-copy"><strong>CodeSpace</strong><span>Local workbench</span></div>
      </div>
      <div class="nav-scroll">
        <div class="nav-section">
          <span class="nav-label">Workspace</span>
          <button class="nav-item is-active" data-nav="overview"><span class="nav-icon">⌂</span><span>Workspace</span></button>
          <button class="nav-item" data-nav="graph"><span class="nav-icon">⌘</span><span>Repository Map</span></button>
          <button class="nav-item" data-nav="context"><span class="nav-icon">{{ }}</span><span>Context Builder</span></button>
          <button class="nav-item" data-nav="impact"><span class="nav-icon">↗</span><span>Impact Analysis</span></button>
          <button class="nav-item" data-nav="history"><span class="nav-icon">◷</span><span>Decision Memory</span></button>
        </div>
        <div class="nav-section">
          <span class="nav-label">Workspace</span>
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
        <div class="page-heading"><h1 id="pageTitle">Workspace</h1><p id="pageSubtitle">Repository, agent, and runtime</p></div>
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
          <div class="workbench-overview"><span class="sr-only">Command Center</span>
            <section class="workbench-editor" aria-label="Repository workspace">
              <div class="editor-tabs">
                <button class="editor-tab is-active" type="button"><span class="file-glyph">◫</span>repository</button>
                <button class="editor-tab" type="button" data-quick-tab="graph"><span class="file-glyph">⌘</span>graph</button>
                <button class="editor-tab" type="button" data-quick-tab="history"><span class="file-glyph">◷</span>decisions</button>
                <div class="editor-tab-spacer"></div>
                <button class="editor-toolbar-action" type="button" data-quick-tab="context">Build context</button>
                <button class="editor-toolbar-action" type="button" data-quick-tab="impact">Analyze diff</button>
              </div>

              <div class="repository-document">
                <header class="repository-document-header">
                  <div class="repo-avatar" id="repositoryInitial">R</div>
                  <div class="repository-document-copy">
                    <span class="document-kicker">Active repository</span>
                    <h2 id="activeRepositoryName">Current directory</h2>
                    <code id="activeRepositoryPath">Loading active repository…</code>
                  </div>
                  <div class="repository-document-actions">
                    <button class="button ghost small" type="button" data-quick-tab="workspaces">Switch repository</button>
                    <button class="button primary small" type="button" data-quick-tab="context">Build context</button>
                  </div>
                </header>

                <div class="repository-statline" aria-label="Repository index summary">
                  <div><span>Files</span><strong id="metricFiles">0</strong></div>
                  <div><span>Symbols</span><strong id="metricSymbols">0</strong></div>
                  <div><span>Relations</span><strong id="metricEdges">0</strong></div>
                  <div><span>Revision</span><strong id="metricRevision">r0</strong><small id="metricUpdated">not indexed</small></div>
                </div>

                <section class="document-section topology-section">
                  <div class="document-section-heading">
                    <div><span class="section-path">repository / topology</span><h3>Connected files</h3></div>
                    <button class="editor-toolbar-action" type="button" data-quick-tab="graph">Open full map</button>
                  </div>
                  <div id="overviewRepositoryMap" class="topology-map"><div class="empty-inline">Loading repository topology…</div></div>
                </section>
              </div>
            </section>

            <aside class="workbench-agent" aria-label="CodeSpace agent">
              <div class="agent-panel-header">
                <div><strong>Agent</strong><span>Repository context</span></div>
                <span class="agent-mode">LOCAL</span>
              </div>
              <div class="agent-transcript">
                <div class="agent-message">
                  <span class="agent-author">CodeSpace</span>
                  <p>The repository is indexed. Select a file, build context, or ask the agent to investigate a change.</p>
                </div>
                <div class="agent-runtime-list">
                  <button type="button" data-quick-tab="skills"><span>Skills</span><strong id="skillsRuntimeStatus">Built-in registry</strong></button>
                  <button type="button" data-quick-tab="mcp"><span>MCP</span><strong id="mcpRuntimeStatus">No servers</strong></button>
                  <button type="button" data-quick-tab="github"><span>GitHub</span><strong id="githubRuntimeStatus">Optional</strong></button>
                </div>
                <div class="agent-message agent-message-muted">
                  <span class="agent-author">Context policy</span>
                  <p>Source stays local. Context is bounded and credential-like values are redacted before output.</p>
                </div>
              </div>
              <div class="agent-shortcuts">
                <button type="button" data-quick-tab="graph"><span>⌘</span>Explore files</button>
                <button type="button" data-quick-tab="context"><span>@</span>Build context</button>
                <button type="button" data-quick-tab="impact"><span>±</span>Review impact</button>
              </div>
              <button class="agent-composer" type="button" data-quick-tab="context">
                <span>Build context for this repository</span><kbd>⌘ ↵</kbd>
              </button>
            </aside>

            <section class="workbench-bottom" aria-label="Runtime output">
              <div class="bottom-panel-tabs">
                <button class="is-active" type="button">System</button>
                <button type="button">Languages</button>
                <button type="button">Interfaces</button>
                <span class="bottom-panel-spacer"></span>
                <code>localhost · v<span id="runtimeVersionMirror">3.0.0</span></code>
              </div>
              <div class="bottom-panel-content">
                <div class="system-output health-list">
                  <div class="health-row" data-tone="warning"><span>semantic-index</span><strong id="indexHealthText">Checking…</strong></div>
                  <div class="health-row"><span>skills-registry</span><strong id="skillsHealthText">Built-in registry</strong></div>
                  <div class="health-row"><span>mcp-servers</span><strong id="mcpHealthText">No servers</strong></div>
                  <div class="health-row"><span>github-delivery</span><strong id="githubHealthText">Optional</strong></div>
                  <div class="health-row" data-tone="success"><span>api-exposure</span><strong>127.0.0.1 only</strong></div>
                </div>
                <div class="language-output">
                  <div class="output-heading">Indexed languages</div>
                  <div id="languageBreakdown"></div>
                </div>
                <div class="interface-output">
                  <div class="output-heading">Interfaces</div>
                  <div class="delivery-stack">
                    <div class="delivery-item"><span>Terminal</span><strong>cse CLI</strong></div>
                    <div class="delivery-item"><span>Agents</span><strong>MCP tools</strong></div>
                    <div class="delivery-item"><span>Browser</span><strong>localhost</strong></div>
                  </div>
                </div>
              </div>
            </section>
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

        <section class="view context-studio-view" data-view="context">
          <div class="panel-page context-studio-page">
            <div class="context-studio-hero">
              <div><h2>Context Studio</h2><p>Compile decisions, constraints, and exact code into a deterministic evidence pack with token budgeting and provenance.</p></div>

            </div>
            <form id="contextForm" class="context-composer">
              <div class="context-query-field"><label for="contextQuery">What does the agent need to understand?</label><input id="contextQuery" type="text" placeholder="Trace the local authentication boundary and its design constraints" required><small>Searches Markdown Context Cells, symbols, paths, and the repository graph.</small></div>
              <div class="context-controls"><div class="field"><label for="contextTokens">Budget</label><input id="contextTokens" type="number" min="128" max="32000" value="1600"><small>tokens</small></div><div class="field"><label for="contextItems">Evidence</label><input id="contextItems" type="number" min="1" max="50" value="10"><small>max items</small></div><button class="button primary context-build" type="submit"><span>Compile pack</span><kbd>Enter</kbd></button></div>
            </form>
            <div id="contextResults" class="results context-results"><div class="context-empty"><strong>Evidence before inference</strong><p>Build a pack to see its sources and full Markdown size. Token counts are estimates, not model billing.</p></div></div>
            <div class="runtime-workbench">
              <section class="runtime-panel" aria-labelledby="bulkReadTitle">
                <header><h3 id="bulkReadTitle">Find evidence across files</h3><p>Bring back relevant lines with exact locations. Large reads stay outside the agent's context.</p></header>
                <form id="bulkReadForm" class="runtime-form">
                  <label for="bulkQuestion">Question</label><input id="bulkQuestion" required placeholder="Where are requests authenticated?">
                  <label for="bulkPaths">Files · one repository path per line</label><textarea id="bulkPaths" rows="3" required placeholder="src/server.rs&#10;src/mcp.rs"></textarea>
                  <div class="runtime-inline"><div><label for="bulkBudget">Estimated token limit</label><input id="bulkBudget" type="number" min="256" max="32000" value="1400"></div><div><label for="bulkKind">Task</label><select id="bulkKind"><option value="inspect">Inspect</option><option value="overview">Overview</option><option value="debug">Debug</option><option value="security">Security</option><option value="architecture">Architecture</option></select></div></div>
                  <button class="button primary" type="submit">Read selected files</button>
                </form>
                <div id="bulkReadResults" class="runtime-result" aria-live="polite"><p class="runtime-note">Runs locally. Debugging and security decisions stay with the reviewing agent.</p></div>
              </section>
              <section class="runtime-panel" aria-labelledby="runStateTitle">
                <header><h3 id="runStateTitle">Resume with current facts</h3><p>Keep the objective, constraints and next action in a bounded checkpoint. Resume without replaying every step.</p></header>
                <form id="runStartForm" class="runtime-form">
                  <label for="runId">Run ID</label><input id="runId" required pattern="[A-Za-z0-9_-]+" maxlength="64" placeholder="auth-review">
                  <label for="runObjective">Objective</label><textarea id="runObjective" rows="3" required placeholder="Verify authentication and preserve existing behavior"></textarea>
                  <div class="runtime-inline"><button class="button primary" type="submit">Start run</button><button id="refreshRunsBtn" class="button ghost" type="button">Refresh runs</button></div>
                </form>
                <div id="runList" class="runtime-run-list" aria-live="polite"><p class="runtime-note">Open Context Studio to load saved runs.</p></div>
                <div id="runDetail" class="runtime-result" aria-live="polite"></div>
              </section>
            </div>
          </div>
        </section>

        <section class="view" data-view="impact">
          <div class="panel-page">
            <div class="panel-intro"><div><h2>Impact Analysis</h2><p>Compare Git revisions and trace potentially affected symbols through the repository graph before making or merging a change.</p></div></div>
            <form id="impactForm" class="surface surface-body form-grid">
              <div class="field"><label for="impactFrom">From revision</label><input id="impactFrom" value="HEAD~1"></div>
              <div class="field"><label for="impactTo">To revision</label><input id="impactTo" value="HEAD"></div>
              <div class="field"><label for="impactDepth">Traversal depth</label><input id="impactDepth" type="number" min="1" max="10" value="3"></div>
              <div class="form-actions"><button class="button primary" type="submit">Analyze Impact</button></div>
            </form>
            <div id="impactResults" class="results"></div>
          </div>
        </section>

        <section class="view" data-view="tasks">
          <div class="panel-page">
            <div class="panel-intro"><div><h2>Engineering Tasks</h2><p>Track work next to repository state. Task mutations are available through the same local API used by the CLI and dashboard.</p></div></div>
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
            <div class="panel-intro"><div><h2>Decision Memory</h2><p>Record why architecture and implementation choices were made, then retrieve them through CLI, MCP, and the dashboard.</p></div></div>
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
            <div class="panel-intro"><div><h2>Repositories</h2><p>Register and switch local repositories. Every dashboard API request resolves the active workspace before executing a core action.</p></div></div>
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
            <div class="panel-intro"><div><h2>Skills</h2><p>Enable trusted, permission-scoped capabilities for security, design, engineering, testing, documentation, and repository analysis.</p></div><div class="status-pill">Pinned sources only</div></div>
            <div id="skillsGrid" class="skills-grid"></div>
          </div>
        </section>

        <section class="view" data-view="mcp">
          <div class="panel-page">
            <div class="panel-intro"><div><h2>MCP Control</h2><p>Register, start, stop, and inspect local MCP server processes. Environment values remain server-side and are never returned to the browser.</p></div></div>
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
            <div class="panel-intro"><div><h2>Settings</h2><p>Workspace settings override global settings. All changes are persisted by the backend and broadcast as synchronization events.</p></div></div>
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
            <div class="panel-intro"><div><h2>GitHub</h2><p>Inspect the local GitHub connection without exposing credentials to page JavaScript.</p></div></div>
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
        .to_string()
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
        let html = render_dashboard();
        assert!(html.contains("Repository Map"));
        assert!(html.contains("MCP Control"));
        assert!(html.contains("data-view=\"skills\""));
        assert!(!html.contains("cse-session"));
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
