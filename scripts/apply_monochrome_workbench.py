#!/usr/bin/env python3
"""Replace the dashboard overview with a dense monochrome IDE workbench."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DASHBOARD_RS = ROOT / "src/dashboard.rs"
MAIN_TS = ROOT / "dashboard/src/main.ts"
DASHBOARD_CSS = ROOT / "dashboard/dist/dashboard.css"
RENDERER = ROOT / "scripts/render_dashboard.mjs"
CI = ROOT / ".github/workflows/ci.yml"


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


dashboard = DASHBOARD_RS.read_text(encoding="utf-8")
dashboard = replace_once(
    dashboard,
    '<meta name="theme-color" content="#07090f">',
    '<meta name="theme-color" content="#0a0a0a">',
    "theme color",
)
dashboard = replace_once(
    dashboard,
    '<div class="brand-copy"><strong>CodeSpace</strong><span>IDE Assistant</span></div>',
    '<div class="brand-copy"><strong>CodeSpace</strong><span>Local workbench</span></div>',
    "brand subtitle",
)
dashboard = replace_once(
    dashboard,
    '<button class="nav-item is-active" data-nav="overview"><span class="nav-icon">⌂</span><span>Command Center</span></button>',
    '<button class="nav-item is-active" data-nav="overview"><span class="nav-icon">⌂</span><span>Workspace</span></button>',
    "overview navigation label",
)
dashboard = replace_once(
    dashboard,
    '<div class="page-heading"><h1 id="pageTitle">Command Center</h1><p id="pageSubtitle">Runtime, index, and repository health</p></div>',
    '<div class="page-heading"><h1 id="pageTitle">Workspace</h1><p id="pageSubtitle">Repository, agent, and runtime</p></div>',
    "page heading",
)

start = dashboard.index('        <section class="view is-active" data-view="overview">')
end = dashboard.index('\n\n        <section class="view graph-view"', start)
new_overview = '''        <section class="view is-active" data-view="overview">
          <div class="workbench-overview">
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
                    <button class="button primary small" type="button" data-quick-tab="assistant">Open agent</button>
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
                  <button type="button" data-quick-tab="assistant"><span>AI</span><strong id="aiRuntimeStatus">Local Ollama</strong></button>
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
              <button class="agent-composer" type="button" data-quick-tab="assistant">
                <span>Ask CodeSpace about this repository</span><kbd>⌘ ↵</kbd>
              </button>
            </aside>

            <section class="workbench-bottom" aria-label="Runtime output">
              <div class="bottom-panel-tabs">
                <button class="is-active" type="button">System</button>
                <button type="button">Languages</button>
                <button type="button">Interfaces</button>
                <span class="bottom-panel-spacer"></span>
                <code>localhost · v<span id="runtimeVersionMirror">2.0.0</span></code>
              </div>
              <div class="bottom-panel-content">
                <div class="system-output health-list">
                  <div class="health-row" data-tone="warning"><span>semantic-index</span><strong id="indexHealthText">Checking…</strong></div>
                  <div class="health-row"><span>ai-runtime</span><strong id="aiHealthText">Local Ollama</strong></div>
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
        </section>'''
dashboard = dashboard[:start] + new_overview + dashboard[end:]
DASHBOARD_RS.write_text(dashboard, encoding="utf-8")

main_ts = MAIN_TS.read_text(encoding="utf-8")
main_ts = replace_once(
    main_ts,
    'overview: ["Command Center", "Runtime, index, and repository health"],',
    'overview: ["Workspace", "Repository, agent, and runtime"],',
    "TypeScript overview label",
)
MAIN_TS.write_text(main_ts, encoding="utf-8")

renderer = RENDERER.read_text(encoding="utf-8")
renderer = renderer.replace('document.body?.innerText.includes("Command Center")', 'document.body?.innerText.includes("Workspace")')
RENDERER.write_text(renderer, encoding="utf-8")

ci = CI.read_text(encoding="utf-8")
ci = ci.replace("grep -q 'Command Center' artifacts/dashboard-overview.html", "grep -q 'Workspace' artifacts/dashboard-overview.html")
CI.write_text(ci, encoding="utf-8")

css = DASHBOARD_CSS.read_text(encoding="utf-8")
marker = "/* monochrome-workbench-v1 */"
if marker in css:
    raise SystemExit("monochrome workbench CSS is already present")
css += r'''

/* monochrome-workbench-v1 */
:root {
  color-scheme: dark;
  --bg: #080808;
  --bg-elevated: #0b0b0b;
  --surface: #0e0e0e;
  --surface-2: #121212;
  --surface-3: #181818;
  --surface-hover: #1b1b1b;
  --line: #242424;
  --line-strong: #333333;
  --text: #f2f2f2;
  --text-2: #d1d1d1;
  --muted: #929292;
  --muted-2: #626262;
  --cyan: #f2f2f2;
  --cyan-soft: rgba(255,255,255,.07);
  --violet: #d6d6d6;
  --violet-soft: rgba(255,255,255,.055);
  --green: #ededed;
  --green-soft: rgba(255,255,255,.07);
  --amber: #bdbdbd;
  --amber-soft: rgba(255,255,255,.055);
  --red: #ffffff;
  --red-soft: rgba(255,255,255,.08);
  --shadow-sm: none;
  --shadow-lg: 0 18px 50px rgba(0,0,0,.42);
  --radius: 4px;
  --radius-sm: 3px;
  --sidebar: 204px;
  --topbar: 48px;
  --font-sans: "SF Pro Text", "Segoe UI", ui-sans-serif, system-ui, -apple-system, sans-serif;
  --font-mono: "SFMono-Regular", "Cascadia Code", Consolas, "Liberation Mono", monospace;
}

body { background: var(--bg); font-size: 12px; letter-spacing: 0; }
button:focus-visible, input:focus-visible, textarea:focus-visible, select:focus-visible { outline: 1px solid #fff; outline-offset: 1px; }
::selection { background: #f2f2f2; color: #090909; }

.sidebar-nav {
  padding: 8px 7px 7px;
  background: #090909;
  border-right-color: #202020;
  backdrop-filter: none;
}
.sidebar-nav::after { display: none; }
.brand { gap: 8px; min-height: 38px; padding: 2px 6px 10px; border-bottom: 1px solid #1f1f1f; }
.brand-mark {
  width: 24px; height: 24px; border-color: #3a3a3a; border-radius: 3px;
  background: #f0f0f0; color: #090909; box-shadow: none; font-size: 10px;
}
.brand-copy strong { font-size: 12px; font-weight: 600; }
.brand-copy span { margin-top: 1px; color: #666; font-size: 8px; letter-spacing: .08em; }
.nav-scroll { padding: 8px 0 0; }
.nav-section { margin-bottom: 12px; }
.nav-label { padding: 0 8px 4px; color: #555; font-size: 8px; letter-spacing: .08em; }
.nav-item {
  min-height: 29px; gap: 7px; margin: 0; padding: 5px 7px; border: 0;
  border-radius: 3px; color: #878787; font-size: 11px; transition: none;
}
.nav-item:hover { background: #141414; color: #d0d0d0; }
.nav-item.is-active { border: 0; background: #191919; color: #fff; box-shadow: inset 1px 0 #fff; }
.nav-icon { width: 16px; color: #656565; font-size: 11px; }
.nav-item.is-active .nav-icon { color: #fff; }
.sidebar-footer { padding: 8px 7px 0; border-top-color: #202020; }
.runtime-row, .local-only { font-size: 9px; }
.local-only::before { width: 5px; height: 5px; background: #ddd; box-shadow: none; }

.topbar { gap: 12px; padding: 0 12px; background: #0b0b0b; border-bottom-color: #242424; backdrop-filter: none; }
.page-heading { min-width: 145px; }
.page-heading h1 { font-size: 12px; font-weight: 600; letter-spacing: 0; }
.page-heading p { display: none; }
.global-search { max-width: 500px; }
.global-search::before { left: 9px; color: #5f5f5f; font-size: 12px; }
.global-search input {
  height: 29px; padding: 0 62px 0 28px; border-color: #2b2b2b; border-radius: 4px;
  background: #111; font-size: 11px; transition: none;
}
.global-search input:focus { border-color: #555; background: #111; box-shadow: none; }
.keycap { right: 6px; border-color: #292929; border-radius: 3px; background: #0c0c0c; color: #666; font-size: 8px; }
.topbar-actions { gap: 5px; }
.workspace-select { height: 29px; max-width: 170px; border-color: #2b2b2b; border-radius: 4px; background-color: #111; font-size: 10px; }
.connection-chip { height: 27px; padding: 0 8px; border-color: #2b2b2b; border-radius: 4px; background: #111; font-size: 9px; }
.connection-chip::before, .connection-chip[data-state="online"]::before, .connection-chip[data-state="degraded"]::before, .connection-chip[data-state="offline"]::before { width: 5px; height: 5px; background: #ddd; box-shadow: none; }
.connection-detail { top: 33px; border-radius: 4px; background: #111; }

.content, .view { background: #080808; }
.view { padding: 14px; }
.view[data-view="overview"] { padding: 0; overflow: hidden; }
.surface,
[class$="-card"],
.integration-hero,
.empty-state,
.loading-card,
.task-column,
.chat-composer,
.prompt-card,
.mcp-card,
.skill-card {
  border-color: #292929 !important;
  border-radius: 4px !important;
  background: #101010 !important;
  box-shadow: none !important;
}
.surface-header { padding: 11px 12px; border-bottom-color: #252525; }
.surface-body { padding: 12px; }
.eyebrow { color: #9a9a9a; font-size: 8px; letter-spacing: .08em; }
.button { min-height: 29px; padding: 0 10px; border-color: #303030; border-radius: 4px; background: #151515; color: #cfcfcf; font-size: 10px; font-weight: 500; transition: none; }
.button:hover:not(:disabled) { transform: none; border-color: #555; background: #1a1a1a; color: #fff; }
.button.primary { border-color: #f0f0f0; background: #f0f0f0; color: #090909; box-shadow: none; }
.button.primary:hover:not(:disabled) { background: #fff; color: #000; }
.button.small { min-height: 25px; padding: 0 8px; border-radius: 3px; font-size: 9px; }

.workbench-overview {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 330px;
  grid-template-rows: minmax(0, 1fr) 224px;
  width: 100%; height: 100%; min-width: 0; min-height: 0;
  background: #080808;
}
.workbench-editor { min-width: 0; min-height: 0; display: grid; grid-template-rows: 35px minmax(0,1fr); border-right: 1px solid #242424; }
.editor-tabs { display: flex; align-items: stretch; min-width: 0; border-bottom: 1px solid #242424; background: #0b0b0b; }
.editor-tab { display: flex; align-items: center; gap: 6px; min-width: 104px; padding: 0 12px; border: 0; border-right: 1px solid #242424; background: #0b0b0b; color: #777; font: 10px var(--font-mono); }
.editor-tab:hover { background: #111; color: #bbb; }
.editor-tab.is-active { position: relative; background: #101010; color: #eee; }
.editor-tab.is-active::after { content: ""; position: absolute; left: 0; right: 0; bottom: -1px; height: 1px; background: #101010; }
.file-glyph { color: #777; }
.editor-tab-spacer { flex: 1; }
.editor-toolbar-action { padding: 0 11px; border: 0; border-left: 1px solid #242424; background: transparent; color: #777; font-size: 9px; }
.editor-toolbar-action:hover { background: #151515; color: #ddd; }

.repository-document { min-width: 0; min-height: 0; overflow: auto; padding: 22px 26px 26px; background: #101010; }
.repository-document-header { display: grid; grid-template-columns: 34px minmax(0,1fr) auto; align-items: center; gap: 11px; padding-bottom: 18px; border-bottom: 1px solid #282828; }
.repository-document .repo-avatar { width: 34px; height: 34px; border: 1px solid #353535; border-radius: 3px; background: #171717; color: #ddd; font-size: 11px; }
.repository-document-copy { min-width: 0; }
.document-kicker { display: block; color: #666; font-size: 8px; letter-spacing: .08em; text-transform: uppercase; }
.repository-document-copy h2 { margin: 3px 0 0; font-size: 16px; font-weight: 600; letter-spacing: -.015em; }
.repository-document-copy code { display: block; overflow: hidden; margin-top: 3px; color: #777; font-size: 9px; text-overflow: ellipsis; white-space: nowrap; }
.repository-document-actions { display: flex; gap: 5px; }
.repository-statline { display: grid; grid-template-columns: repeat(4, minmax(0,1fr)); border-bottom: 1px solid #282828; }
.repository-statline > div { min-width: 0; padding: 13px 15px 12px 0; border-right: 1px solid #282828; }
.repository-statline > div:not(:first-child) { padding-left: 15px; }
.repository-statline > div:last-child { border-right: 0; }
.repository-statline span { display: block; color: #6e6e6e; font: 8px var(--font-mono); text-transform: uppercase; }
.repository-statline strong { display: inline-block; margin-top: 5px; color: #e8e8e8; font: 600 15px var(--font-mono); }
.repository-statline small { margin-left: 7px; color: #5e5e5e; font-size: 8px; }
.document-section { padding-top: 18px; }
.document-section-heading { display: flex; align-items: flex-end; justify-content: space-between; gap: 12px; margin-bottom: 10px; }
.document-section-heading h3 { margin: 4px 0 0; font-size: 12px; font-weight: 600; }
.section-path { color: #606060; font: 8px var(--font-mono); }

.workbench-overview .topology-map {
  display: grid; grid-template-columns: repeat(2, minmax(0,1fr)); align-content: start; gap: 0;
  min-height: 0; padding: 0; border: 1px solid #292929; border-radius: 3px;
  background: #0d0d0d; background-image: none;
}
.workbench-overview .topology-map::before, .workbench-overview .topology-map::after { display: none; }
.workbench-overview .topology-node,
.workbench-overview .topology-node-2,
.workbench-overview .topology-node-3,
.workbench-overview .topology-node-6,
.workbench-overview .topology-node-7 {
  transform: none; min-height: 52px; padding: 10px 12px 9px 26px; border: 0; border-right: 1px solid #242424; border-bottom: 1px solid #242424; border-radius: 0;
  background: #0e0e0e; box-shadow: none; transition: none;
}
.workbench-overview .topology-node:nth-child(even) { border-right: 0; }
.workbench-overview .topology-node:hover,
.workbench-overview .topology-node-2:hover,
.workbench-overview .topology-node-3:hover,
.workbench-overview .topology-node-6:hover,
.workbench-overview .topology-node-7:hover { transform: none; border-color: #242424; background: #161616; box-shadow: none; }
.workbench-overview .topology-node strong { color: #ddd; font-size: 9px; font-weight: 500; }
.workbench-overview .topology-node small { margin-top: 4px; color: #666; font-size: 8px; }
.workbench-overview .topology-node i { left: 0; top: 0; bottom: 0; width: 2px; height: auto; background: #777; box-shadow: none; }
.workbench-overview .topology-node-language { top: 20px; right: auto; left: 12px; width: 5px; height: 5px; background: #888 !important; }

.workbench-agent { min-width: 0; min-height: 0; display: grid; grid-template-rows: 43px minmax(0,1fr) auto auto; background: #0c0c0c; }
.agent-panel-header { display: flex; align-items: center; justify-content: space-between; padding: 0 12px; border-bottom: 1px solid #242424; }
.agent-panel-header > div { display: flex; align-items: baseline; gap: 7px; }
.agent-panel-header strong { font-size: 11px; font-weight: 600; }
.agent-panel-header span { color: #666; font-size: 8px; }
.agent-mode { padding: 2px 5px; border: 1px solid #303030; border-radius: 3px; color: #8a8a8a !important; font: 7px var(--font-mono); }
.agent-transcript { min-height: 0; overflow: auto; padding: 13px 12px; }
.agent-message { padding: 0 0 13px; border-bottom: 1px solid #202020; }
.agent-message + .agent-message { margin-top: 13px; }
.agent-author { color: #bdbdbd; font: 8px var(--font-mono); text-transform: uppercase; }
.agent-message p { margin: 7px 0 0; color: #a2a2a2; font-size: 10px; line-height: 1.55; }
.agent-message-muted p { color: #777; }
.agent-runtime-list { display: grid; margin: 13px 0; border: 1px solid #252525; }
.agent-runtime-list button { display: grid; grid-template-columns: 58px minmax(0,1fr); gap: 8px; width: 100%; padding: 8px 9px; border: 0; border-bottom: 1px solid #222; background: #0e0e0e; text-align: left; }
.agent-runtime-list button:last-child { border-bottom: 0; }
.agent-runtime-list button:hover { background: #151515; }
.agent-runtime-list span { color: #646464; font: 8px var(--font-mono); text-transform: uppercase; }
.agent-runtime-list strong { overflow: hidden; color: #c9c9c9; font-size: 9px; font-weight: 500; text-overflow: ellipsis; white-space: nowrap; }
.agent-shortcuts { display: grid; grid-template-columns: repeat(3,1fr); border-top: 1px solid #242424; }
.agent-shortcuts button { display: grid; place-items: center; gap: 4px; min-height: 52px; padding: 6px; border: 0; border-right: 1px solid #242424; background: #0d0d0d; color: #777; font-size: 8px; }
.agent-shortcuts button:last-child { border-right: 0; }
.agent-shortcuts button:hover { background: #151515; color: #ddd; }
.agent-shortcuts span { color: #aaa; font: 10px var(--font-mono); }
.agent-composer { display: flex; align-items: center; justify-content: space-between; gap: 8px; min-height: 46px; margin: 9px; padding: 0 10px; border: 1px solid #343434; border-radius: 4px; background: #141414; color: #8b8b8b; font-size: 9px; text-align: left; }
.agent-composer:hover { border-color: #555; color: #d6d6d6; }
.agent-composer kbd { color: #666; font: 8px var(--font-mono); }

.workbench-bottom { grid-column: 1 / -1; min-width: 0; min-height: 0; display: grid; grid-template-rows: 33px minmax(0,1fr); border-top: 1px solid #242424; background: #0b0b0b; }
.bottom-panel-tabs { display: flex; align-items: stretch; border-bottom: 1px solid #242424; }
.bottom-panel-tabs button { padding: 0 12px; border: 0; border-right: 1px solid #222; background: transparent; color: #666; font-size: 8px; text-transform: uppercase; }
.bottom-panel-tabs button.is-active { color: #ddd; box-shadow: inset 0 -1px #ddd; }
.bottom-panel-spacer { flex: 1; }
.bottom-panel-tabs code { align-self: center; padding-right: 12px; color: #555; font-size: 8px; }
.bottom-panel-content { display: grid; grid-template-columns: minmax(0,1.35fr) minmax(220px,.7fr) minmax(220px,.7fr); min-height: 0; }
.system-output, .language-output, .interface-output { min-width: 0; min-height: 0; overflow: auto; padding: 10px 12px; border-right: 1px solid #242424; }
.interface-output { border-right: 0; }
.output-heading { margin-bottom: 5px; color: #555; font: 8px var(--font-mono); text-transform: uppercase; }
.workbench-bottom .health-list { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: 0 18px; }
.workbench-bottom .health-row { min-height: 29px; padding: 5px 0; border-bottom-color: #202020; }
.workbench-bottom .health-row span { color: #6f6f6f; font: 8px var(--font-mono); }
.workbench-bottom .health-row strong,
.workbench-bottom .health-row[data-tone="success"] strong,
.workbench-bottom .health-row[data-tone="warning"] strong { color: #c8c8c8; font-size: 8px; font-weight: 500; }
.workbench-bottom .language-row { padding: 5px 0; border-bottom-color: #202020; font-size: 9px; }
.workbench-bottom .language-dot { width: 5px; height: 5px; background: #888 !important; }
.workbench-bottom .delivery-stack { display: grid; grid-template-columns: repeat(3,minmax(0,1fr)); gap: 5px; }
.workbench-bottom .delivery-item { padding: 7px; border-color: #242424; border-radius: 3px; background: #0e0e0e; }
.workbench-bottom .delivery-item span { color: #555; font-size: 7px; }
.workbench-bottom .delivery-item strong { margin-top: 3px; color: #bbb; font-size: 8px; font-weight: 500; }

.graph-toolbar, .graph-inspector, .chat-shell, .assistant-context { background: #0d0d0d; }
.graph-canvas { background-color: #0b0b0b; background-image: linear-gradient(#151515 1px, transparent 1px), linear-gradient(90deg, #151515 1px, transparent 1px); background-size: 24px 24px; }
.graph-canvas::after { display: none; }
.file-edge { stroke: #3d3d3d; }
.file-node > rect:first-child { fill: #121212; stroke: #333; filter: none; }
.file-node:hover > rect:first-child { fill: #181818; stroke: #777; }
.file-node.is-selected > rect:first-child { fill: #1a1a1a; stroke: #eee; }
.language-indicator { fill: #888 !important; }
.meta-chip { border-radius: 3px; background: #111; }
.skill-card:hover { transform: none; }
.skill-icon, .integration-logo, .empty-icon, .boot-logo, .toast-icon { border-radius: 3px !important; background: #171717 !important; color: #ddd !important; box-shadow: none !important; }
.switch input:checked + span { border-color: #777; background: #222; }
.switch input:checked + span::after { background: #eee; }
.mcp-status[data-status="running"], .mcp-status[data-status="starting"], .mcp-status[data-status="error"] { background: #ddd; box-shadow: none; }
.spinner { border-top-color: #ddd; }
.boot-overlay { background: #080808; backdrop-filter: none; }
.toast { border-radius: 4px; background: #141414; box-shadow: 0 12px 40px rgba(0,0,0,.45); }
.lang-rust, .lang-typescript, .lang-ts, .lang-javascript, .lang-js, .lang-python, .lang-html, .lang-css, .lang-markdown { background: #888 !important; }

@media (max-width: 1120px) {
  :root { --sidebar: 62px; }
  .workbench-overview { grid-template-columns: minmax(0,1fr) 290px; }
  .brand-copy, .nav-label, .nav-item span:last-child, .sidebar-footer { display: none; }
  .brand, .nav-item { justify-content: center; }
}
@media (max-width: 860px) {
  .workbench-overview { grid-template-columns: 1fr; grid-template-rows: minmax(0,1fr) 210px; }
  .workbench-agent { display: none; }
  .workbench-editor { border-right: 0; }
  .bottom-panel-content { grid-template-columns: 1fr 1fr; }
  .interface-output { display: none; }
}
@media (max-width: 620px) {
  :root { --sidebar: 50px; --topbar: 44px; }
  .repository-document { padding: 15px; }
  .repository-document-header { grid-template-columns: 30px minmax(0,1fr); }
  .repository-document-actions { display: none; }
  .repository-statline { grid-template-columns: repeat(2,1fr); }
  .repository-statline > div:nth-child(2) { border-right: 0; }
  .workbench-overview .topology-map { grid-template-columns: 1fr; }
  .workbench-overview .topology-node { border-right: 0; }
  .bottom-panel-content { grid-template-columns: 1fr; }
  .language-output { display: none; }
}
'''
DASHBOARD_CSS.write_text(css, encoding="utf-8")
print("monochrome workbench applied")
