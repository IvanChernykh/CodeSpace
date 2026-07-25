pub fn render_dashboard(token: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>CodeSpace Smart AI</title>
<link rel="stylesheet" href="/assets/dashboard.css">
</head>
<body>
<header class="app-header">
  <div class="app-logo">CodeSpace<span>Smart AI</span></div>
  <div class="header-spacer"></div>
  <select id="workspaceSelect" class="header-select" aria-label="Workspace">
    <option value="">Loading workspaces...</option>
  </select>
  <button id="paletteTrigger" class="icon-button" title="Command palette (Ctrl+K)" aria-label="Command palette">Cmd K</button>
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
  <div class="tab" data-tab="ai" role="tab" tabindex="0">AI Chat</div>
  <div class="tab" data-tab="tasks" role="tab" tabindex="0">Tasks</div>
  <div class="tab" data-tab="github" role="tab" tabindex="0">GitHub</div>
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
        <summary style="font-size:13px;font-weight:600;cursor:pointer;color:var(--fg)">Remember a decision</summary>
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

    <div class="panel" data-panel="ai" style="display:none;flex-direction:column;padding:0">
      <div class="ai-chat-panel">
        <div class="ai-chat-messages" id="aiMessages">
          <div class="ai-msg is-system">CodeSpace Smart AI ready. Ask anything about your codebase.</div>
        </div>
        <div class="ai-chat-input-row">
          <textarea class="ai-chat-input" id="aiInput" placeholder="Ask about code, architecture, bugs..." rows="1"></textarea>
          <button class="ai-chat-send" id="aiSend">Send</button>
        </div>
      </div>
    </div>

    <div class="panel" data-panel="tasks" style="display:none">
      <div class="panel-form">
        <div class="form-field grow">
          <label>Title</label>
          <input id="taskTitle" type="text" placeholder="Task title...">
        </div>
        <div class="form-field">
          <label>Priority</label>
          <select id="taskPriority">
            <option value="low">Low</option>
            <option value="medium" selected>Medium</option>
            <option value="high">High</option>
            <option value="critical">Critical</option>
          </select>
        </div>
        <div class="form-field">
          <label>Tags (comma)</label>
          <input id="taskTags" type="text" placeholder="bug,ui" style="width:100px">
        </div>
        <button class="primary-button" id="taskAddBtn">Add Task</button>
      </div>
      <div class="task-list" id="taskList"></div>
    </div>

    <div class="panel" data-panel="github" style="display:none">
      <div id="githubContent">
        <div class="gh-not-linked">Loading GitHub status...</div>
      </div>
    </div>

    <div class="panel" data-panel="workspaces">
      <div data-workspaces-list></div>
      <details class="panel-form" style="margin-top:20px">
        <summary style="font-size:13px;font-weight:600;cursor:pointer;color:var(--fg)">Register a new workspace</summary>
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
<script>
// AI Chat
(function(){{
  const msgs = document.getElementById('aiMessages');
  const input = document.getElementById('aiInput');
  const sendBtn = document.getElementById('aiSend');
  const token = window.__CSE_TOKEN__ || '';
  async function sendChat() {{
    const text = input.value.trim();
    if (!text) return;
    input.value = '';
    sendBtn.disabled = true;
    const userDiv = document.createElement('div');
    userDiv.className = 'ai-msg is-user';
    userDiv.textContent = text;
    msgs.appendChild(userDiv);
    const thinkingDiv = document.createElement('div');
    thinkingDiv.className = 'ai-msg is-assistant';
    thinkingDiv.textContent = 'Thinking...';
    msgs.appendChild(thinkingDiv);
    msgs.scrollTop = msgs.scrollHeight;
    try {{
      const r = await fetch('/api/v1/ai/chat', {{
        method: 'POST',
        headers: {{ 'Content-Type': 'application/json', 'Authorization': 'Bearer ' + token }},
        body: JSON.stringify({{ query: text }})
      }});
      if (!r.ok) throw new Error('HTTP ' + r.status);
      const data = await r.json();
      thinkingDiv.textContent = data.response || data.error || 'No response';
    }} catch(e) {{
      thinkingDiv.textContent = 'Error: ' + e.message;
    }}
    msgs.scrollTop = msgs.scrollHeight;
    sendBtn.disabled = false;
  }}
  sendBtn.addEventListener('click', sendChat);
  input.addEventListener('keydown', (e) => {{ if (e.key === 'Enter' && !e.shiftKey) {{ e.preventDefault(); sendChat(); }} }});
}})();

// Tasks
(function(){{
  const list = document.getElementById('taskList');
  const titleInput = document.getElementById('taskTitle');
  const prioritySelect = document.getElementById('taskPriority');
  const tagsInput = document.getElementById('taskTags');
  const addBtn = document.getElementById('taskAddBtn');
  const token = window.__CSE_TOKEN__ || '';
  async function loadTasks() {{
    try {{
      const r = await fetch('/api/v1/tasks', {{ headers: {{ 'Authorization': 'Bearer ' + token }} }});
      if (!r.ok) return;
      const data = await r.json();
      list.innerHTML = '';
      for (const t of (data.tasks || [])) {{
        const card = document.createElement('div');
        card.className = 'task-card' + (t.status === 'done' ? ' is-done' : '');
        const dot = document.createElement('div');
        dot.className = 'task-priority-dot is-' + (t.priority || 'medium');
        const body = document.createElement('div');
        body.className = 'task-body';
        body.innerHTML = '<div class="task-title"></div><div class="task-desc"></div><div class="task-meta"><span class="task-status-badge">' + (t.status||'todo') + '</span></div>';
        body.querySelector('.task-title').textContent = t.title;
        body.querySelector('.task-desc').textContent = t.description || '';
        if (t.tags && t.tags.length) {{
          const tagsDiv = document.createElement('div');
          tagsDiv.className = 'task-tags';
          for (const tag of t.tags) {{
            const span = document.createElement('span');
            span.className = 'task-tag';
            span.textContent = tag;
            tagsDiv.appendChild(span);
          }}
          body.querySelector('.task-meta').appendChild(tagsDiv);
        }}
        card.appendChild(dot);
        card.appendChild(body);
        list.appendChild(card);
      }}
    }} catch(e) {{}}
  }}
  addBtn.addEventListener('click', async () => {{
    const title = titleInput.value.trim();
    if (!title) return;
    addBtn.disabled = true;
    try {{
      await fetch('/api/v1/tasks', {{
        method: 'POST',
        headers: {{ 'Content-Type': 'application/json', 'Authorization': 'Bearer ' + token }},
        body: JSON.stringify({{ title, priority: prioritySelect.value, tags: tagsInput.value }})
      }});
      titleInput.value = '';
      tagsInput.value = '';
      loadTasks();
    }} catch(e) {{}}
    addBtn.disabled = false;
  }});
  loadTasks();
}})();

// GitHub
(function(){{
  const content = document.getElementById('githubContent');
  const token = window.__CSE_TOKEN__ || '';
  async function loadGitHub() {{
    try {{
      const r = await fetch('/api/v1/github/status', {{ headers: {{ 'Authorization': 'Bearer ' + token }} }});
      if (!r.ok) throw new Error('HTTP ' + r.status);
      const data = await r.json();
      if (!data.token_set) {{
        content.innerHTML = '<div class="gh-not-linked">GitHub not linked.<br>Run: <code>cse github link --token &lt;token&gt; --username &lt;user&gt;</code></div>';
        return;
      }}
      content.innerHTML = '<div class="gh-status-card"><div class="gh-label">Username</div><div class="gh-value">' + (data.username||'') + '</div></div>';
    }} catch(e) {{
      content.innerHTML = '<div class="gh-not-linked">Error loading GitHub status</div>';
    }}
  }}
  loadGitHub();
}})();
</script>
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
