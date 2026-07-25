pub fn render_dashboard(token: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>CodeSpace 2.0</title>
<link rel="stylesheet" href="/assets/dashboard.css">
</head>
<body>
<header class="app-header">
  <div class="app-logo">CodeSpace<span>2.0</span></div>
  <div class="header-spacer"></div>
  <select id="workspaceSelect" class="header-select" aria-label="Workspace">
    <option value="">Loading workspaces...</option>
  </select>
  <button id="paletteTrigger" class="icon-button" title="Command palette (Ctrl+K)" aria-label="Command palette">&#8984;K</button>
  <button id="doctorBtn" class="secondary-button" title="Run doctor check">Doctor</button>
  <button id="updateIndexBtn" class="primary-button">Update Index</button>
  <div class="status-cluster">
    <span id="statusDot" class="status-dot is-offline"></span>
    <span id="statusText">Connecting...</span>
  </div>
</header>

<nav class="tab-bar" role="tablist">
  <div class="tab is-active" data-tab="graph" role="tab" tabindex="0">Graph</div>
  <div class="tab" data-tab="context" role="tab" tabindex="0">Context</div>
  <div class="tab" data-tab="impact" role="tab" tabindex="0">Impact</div>
  <div class="tab" data-tab="history" role="tab" tabindex="0">History</div>
  <div class="tab" data-tab="workspaces" role="tab" tabindex="0">Workspaces</div>
</nav>

<main class="app-main">
  <aside id="sidebar" class="sidebar">
    <div class="sidebar-header">
      <h3>Symbols <span class="sidebar-count" data-sidebar-count>0</span></h3>
    </div>
    <input class="sidebar-search" data-sidebar-filter type="text" placeholder="Filter symbols..." aria-label="Filter symbols">
    <div class="sidebar-chips" data-sidebar-chips></div>
    <ul class="sidebar-list" data-sidebar-list></ul>
  </aside>

  <section class="center-panel">
    <div class="canvas-area" data-panel="graph">
      <div id="graphEmptyState" class="graph-empty">Loading graph...</div>
      <svg id="graphSvg" style="display:none" xmlns="http://www.w3.org/2000/svg"></svg>
      <div class="edge-filters">
        <label><input type="checkbox" data-edge-filter="calls" checked> calls</label>
        <label><input type="checkbox" data-edge-filter="imports" checked> imports</label>
        <label><input type="checkbox" data-edge-filter="contains" checked> contains</label>
        <label><input type="checkbox" data-edge-filter="test-covers" checked> test-covers</label>
        <label><input type="checkbox" data-edge-filter="depends-on" checked> depends-on</label>
      </div>
    </div>

    <div class="panel" data-panel="context">
      <div class="panel-form">
        <div class="form-field grow">
          <label for="contextQuery">Query</label>
          <input id="contextQuery" data-context-query type="text" placeholder="Symbol name, file path, or question...">
        </div>
        <div class="form-field">
          <label for="contextTokens">Max tokens</label>
          <input id="contextTokens" data-context-tokens type="number" value="1200" min="100" max="32000" style="width:90px">
        </div>
        <div class="form-field">
          <label for="contextItems">Max items</label>
          <input id="contextItems" data-context-items type="number" value="8" min="1" max="50" style="width:70px">
        </div>
        <button class="primary-button" data-context-run>Build context</button>
      </div>
      <div data-context-results></div>
    </div>

    <div class="panel" data-panel="impact">
      <div class="panel-form">
        <div class="form-field">
          <label for="impactFrom">From</label>
          <input id="impactFrom" data-impact-from type="text" value="HEAD~1" style="width:120px">
        </div>
        <div class="form-field">
          <label for="impactTo">To</label>
          <input id="impactTo" data-impact-to type="text" value="HEAD" style="width:120px">
        </div>
        <div class="form-field">
          <label for="impactDepth">Depth</label>
          <input id="impactDepth" data-impact-depth type="number" value="3" min="1" max="10" style="width:60px">
        </div>
        <button class="primary-button" data-impact-run>Analyze impact</button>
      </div>
      <div data-impact-results></div>
    </div>

    <div class="panel" data-panel="history">
      <div class="panel-form">
        <div class="form-field grow">
          <label for="historyQuery">Search decisions</label>
          <input id="historyQuery" data-history-query type="text" placeholder="Filter by keyword...">
        </div>
        <button class="secondary-button" data-history-run>Search</button>
      </div>
      <div data-history-results></div>
      <details class="panel-form" style="margin-top:20px">
        <summary style="font-size:13px;font-weight:600;cursor:pointer;color:var(--accent)">Remember a decision</summary>
        <form data-remember-form style="display:flex;flex-wrap:wrap;gap:10px;margin-top:12px">
          <div class="form-field grow">
            <label>Summary</label>
            <input name="summary" type="text" placeholder="What was decided?" required>
          </div>
          <div class="form-field">
            <label>File</label>
            <input name="file" type="text" placeholder="src/main.rs">
          </div>
          <div class="form-field">
            <label>Symbol</label>
            <input name="symbol" type="text" placeholder="fn main">
          </div>
          <div class="form-field grow">
            <label>Rationale</label>
            <input name="rationale" type="text" placeholder="Why?">
          </div>
          <div class="form-field">
            <label>Tags (comma-separated)</label>
            <input name="tags" type="text" placeholder="architecture, refactor">
          </div>
          <input name="session" type="hidden" value="dashboard">
          <input name="agent" type="hidden" value="dashboard-user">
          <button class="primary-button" data-remember-submit type="submit">Remember</button>
        </form>
      </details>
    </div>

    <div class="panel" data-panel="workspaces">
      <div data-workspaces-list></div>
      <details class="panel-form" style="margin-top:20px">
        <summary style="font-size:13px;font-weight:600;cursor:pointer;color:var(--accent)">Register a new workspace</summary>
        <form data-workspace-form style="display:flex;flex-wrap:wrap;gap:10px;margin-top:12px">
          <div class="form-field grow">
            <label>Directory path</label>
            <input name="path" type="text" placeholder="/path/to/project" required>
          </div>
          <div class="form-field">
            <label>Name (optional)</label>
            <input name="name" type="text" placeholder="My Project">
          </div>
          <button class="primary-button" data-workspace-submit type="submit">Register</button>
        </form>
      </details>
    </div>
  </section>

  <aside id="inspector" class="inspector">
    <div class="inspector-empty">
      <p>Select a symbol from the graph or the list to inspect it.</p>
      <p class="muted">Tip: press / to search, or click a node to focus it.</p>
    </div>
  </aside>
</main>

<div id="commandPalette" class="palette-overlay">
  <div class="palette-dialog" role="dialog" aria-label="Command palette">
    <div class="palette-input-row">
      <input data-palette-input type="text" placeholder="Search commands and symbols..." aria-label="Search">
      <button class="palette-close" data-palette-close aria-label="Close">&times;</button>
    </div>
    <div class="palette-list" data-palette-list></div>
  </div>
</div>

<div id="toastRoot"></div>

<script>window.__CSE_TOKEN__ = "{token}";</script>
<script type="module" src="/assets/main.js"></script>
</body>
</html>"#
    )
}

/// Serve a compiled static asset by URL path (e.g. `/assets/main.js`).
/// Returns `(content_type, body)` or `None` if not found.
pub fn asset(path: &str) -> Option<(&'static str, String)> {
    let name = path.strip_prefix("/assets/")?;
    let (content_type, body) = match name {
        "main.js" => ("text/javascript", include_str!("../dashboard/dist/main.js")),
        "api.js" => ("text/javascript", include_str!("../dashboard/dist/api.js")),
        "dom.js" => ("text/javascript", include_str!("../dashboard/dist/dom.js")),
        "toast.js" => ("text/javascript", include_str!("../dashboard/dist/toast.js")),
        "state.js" => ("text/javascript", include_str!("../dashboard/dist/state.js")),
        "sse.js" => ("text/javascript", include_str!("../dashboard/dist/sse.js")),
        "graph-view.js" => ("text/javascript", include_str!("../dashboard/dist/graph-view.js")),
        "sidebar.js" => ("text/javascript", include_str!("../dashboard/dist/sidebar.js")),
        "inspector.js" => ("text/javascript", include_str!("../dashboard/dist/inspector.js")),
        "command-palette.js" => ("text/javascript", include_str!("../dashboard/dist/command-palette.js")),
        "types.js" => ("text/javascript", include_str!("../dashboard/dist/types.js")),
        "panels/context-panel.js" => ("text/javascript", include_str!("../dashboard/dist/panels/context-panel.js")),
        "panels/impact-panel.js" => ("text/javascript", include_str!("../dashboard/dist/panels/impact-panel.js")),
        "panels/history-panel.js" => ("text/javascript", include_str!("../dashboard/dist/panels/history-panel.js")),
        "panels/workspaces-panel.js" => ("text/javascript", include_str!("../dashboard/dist/panels/workspaces-panel.js")),
        "dashboard.css" => ("text/css", include_str!("../dashboard/dist/dashboard.css")),
        _ => return None,
    };
    Some((content_type, body.to_string()))
}
