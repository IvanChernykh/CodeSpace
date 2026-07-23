pub fn render_dashboard() -> String {
    r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>CodeSpace 2.0</title>
<style>
:root{--bg:#0d1117;--bg2:#161b22;--bg3:#21262d;--fg:#e6edf3;--muted:#8b949e;--accent:#58a6ff;--accent2:#3fb950;--border:#30363d;--danger:#f85149;--warn:#d29922}
*{box-sizing:border-box;margin:0;padding:0}
body{font-family:system-ui,-apple-system,sans-serif;background:var(--bg);color:var(--fg);height:100vh;overflow:hidden;display:flex;flex-direction:column}
header{display:flex;align-items:center;gap:16px;padding:10px 20px;background:var(--bg2);border-bottom:1px solid var(--border);flex-shrink:0}
header .logo{font-size:18px;font-weight:700;color:var(--accent)}
header .logo span{color:var(--muted);font-weight:400;font-size:13px;margin-left:8px}
header .ws-select{background:var(--bg3);color:var(--fg);border:1px solid var(--border);border-radius:6px;padding:6px 12px;font-size:13px;cursor:pointer}
header .search{flex:1;max-width:500px;background:var(--bg);border:1px solid var(--border);border-radius:6px;padding:8px 14px;color:var(--fg);font-size:14px;outline:none}
header .search:focus{border-color:var(--accent)}
header .btn{background:var(--bg3);border:1px solid var(--border);border-radius:6px;padding:6px 14px;color:var(--fg);cursor:pointer;font-size:13px;white-space:nowrap}
header .btn:hover{border-color:var(--accent)}
header .btn.primary{background:var(--accent);color:#000;border-color:var(--accent)}
header .status-dot{width:8px;height:8px;border-radius:50%;background:var(--accent2);display:inline-block}
main{display:grid;grid-template-columns:280px 1fr 320px;height:calc(100vh - 52px);overflow:hidden}
aside.left{background:var(--bg2);border-right:1px solid var(--border);overflow-y:auto;padding:12px}
aside.right{background:var(--bg2);border-left:1px solid var(--border);overflow-y:auto;padding:12px}
section.center{display:flex;flex-direction:column;overflow:hidden}
.canvas-area{flex:1;position:relative;overflow:hidden;background:var(--bg)}
.canvas-area svg{width:100%;height:100%}
.bottom-panel{height:180px;background:var(--bg2);border-top:1px solid var(--border);overflow-y:auto;padding:10px 16px;flex-shrink:0}
.bottom-panel h4{color:var(--muted);font-size:12px;text-transform:uppercase;margin-bottom:8px}
.sym-list{list-style:none}
.sym-item{padding:8px 10px;border-radius:6px;cursor:pointer;display:flex;align-items:center;gap:8px;font-size:13px;margin-bottom:2px}
.sym-item:hover{background:var(--bg3)}
.sym-item.active{background:var(--bg3);border-left:3px solid var(--accent)}
.sym-kind{font-size:10px;padding:2px 6px;border-radius:4px;background:var(--bg3);color:var(--muted);text-transform:uppercase;white-space:nowrap}
.inspector h3{font-size:15px;margin-bottom:12px;color:var(--accent)}
.inspector .field{margin-bottom:10px}
.inspector .field-label{font-size:11px;color:var(--muted);text-transform:uppercase;margin-bottom:2px}
.inspector .field-value{font-size:13px;font-family:ui-monospace,monospace;word-break:break-all}
.inspector .edges-table{width:100%;border-collapse:collapse;font-size:12px}
.inspector .edges-table th{text-align:left;color:var(--muted);padding:4px 8px;border-bottom:1px solid var(--border)}
.inspector .edges-table td{padding:4px 8px;border-bottom:1px solid var(--border)}
.inspector pre{background:var(--bg);border:1px solid var(--border);border-radius:6px;padding:10px;font-size:12px;overflow-x:auto;max-height:200px}
.tab-bar{display:flex;gap:2px;padding:0 12px;background:var(--bg2);border-bottom:1px solid var(--border);flex-shrink:0}
.tab{padding:8px 16px;font-size:13px;color:var(--muted);cursor:pointer;border-bottom:2px solid transparent}
.tab:hover{color:var(--fg)}
.tab.active{color:var(--accent);border-bottom-color:var(--accent)}
.node{cursor:pointer;transition:opacity .2s}
.node:hover circle{stroke:var(--accent);stroke-width:2}
.node text{pointer-events:none;font-size:11px;fill:var(--fg)}
.edge-line{stroke:var(--border);stroke-width:1;fill:none}
.edge-line.calls{stroke:var(--accent);stroke-dasharray:4}
.edge-line.imports{stroke:var(--warn)}
.edge-line.contains{stroke:var(--muted)}
.legend{position:absolute;bottom:12px;left:12px;background:var(--bg2);border:1px solid var(--border);border-radius:8px;padding:10px;font-size:11px;color:var(--muted)}
.legend-item{display:flex;align-items:center;gap:6px;margin:4px 0}
.legend-color{width:16px;height:2px;border-radius:1px}
.empty-state{display:flex;align-items:center;justify-content:center;height:100%;color:var(--muted);font-size:14px}
.spinner{border:2px solid var(--border);border-top:2px solid var(--accent);border-radius:50%;width:20px;height:20px;animation:spin 1s linear infinite;display:inline-block;margin-right:8px}
@keyframes spin{to{transform:rotate(360deg)}}
.ctx-item{background:var(--bg);border:1px solid var(--border);border-radius:6px;padding:10px;margin-bottom:8px;font-size:12px}
.ctx-item .ctx-header{display:flex;justify-content:space-between;margin-bottom:6px}
.ctx-item .ctx-symbol{color:var(--accent);font-family:ui-monospace,monospace}
.ctx-item .ctx-score{color:var(--muted);font-size:11px}
.ctx-item pre{background:var(--bg2);padding:8px;border-radius:4px;overflow-x:auto;font-size:11px;margin-top:6px}
</style>
</head>
<body>
<header>
<div class="logo">CodeSpace <span>2.0</span></div>
<select class="ws-select" id="wsSelect"><option>Default workspace</option></select>
<input class="search" id="searchInput" placeholder="Search symbols, files, or ask a question..." />
<button class="btn" id="contextBtn">Context</button>
<button class="btn" id="impactBtn">Impact</button>
<button class="btn primary" id="updateBtn">Update Index</button>
<span class="status-dot" id="statusDot"></span>
</header>
<div class="tab-bar">
<div class="tab active" data-tab="graph">Graph</div>
<div class="tab" data-tab="context">Context</div>
<div class="tab" data-tab="impact">Impact</div>
<div class="tab" data-tab="history">History</div>
</div>
<main>
<aside class="left">
<div id="symListContainer">
<h4 style="color:var(--muted);font-size:12px;text-transform:uppercase;margin-bottom:8px">Symbols</h4>
<ul class="sym-list" id="symList"></ul>
</div>
</aside>
<section class="center">
<div class="canvas-area" id="canvasArea">
<div class="empty-state" id="emptyState"><span class="spinner"></span>Loading graph...</div>
<svg id="graphSvg" style="display:none"></svg>
<div class="legend" id="legend" style="display:none">
<div class="legend-item"><div class="legend-color" style="background:var(--accent)"></div>calls</div>
<div class="legend-item"><div class="legend-color" style="background:var(--warn)"></div>imports</div>
<div class="legend-item"><div class="legend-color" style="background:var(--muted)"></div>contains</div>
</div>
</div>
<div class="bottom-panel" id="bottomPanel">
<h4>Console</h4>
<div id="consoleOutput" style="font-size:12px;font-family:ui-monospace,monospace;color:var(--muted)"></div>
</div>
</section>
<aside class="right">
<div class="inspector" id="inspector">
<h3>Inspector</h3>
<p style="color:var(--muted);font-size:13px">Select a symbol to inspect its details and relationships.</p>
</div>
</aside>
</main>
<script>
const API='/api/v1';
let graphData=null, selectedSymbol=null, currentTab='graph';

async function api(path,opts){const r=await fetch(API+path,opts);return r.json();}
async function loadGraph(){
try{
const data=await api('/graph');
graphData=data;
document.getElementById('emptyState').style.display='none';
document.getElementById('graphSvg').style.display='block';
document.getElementById('legend').style.display='block';
renderGraph();
renderSymbolList();
log('Loaded '+data.symbols.length+' symbols, '+data.edges.length+' edges');
}catch(e){log('Error loading graph: '+e.message);}
}
function renderGraph(){
const svg=document.getElementById('graphSvg');
const w=svg.clientWidth,h=svg.clientHeight;
const symbols=graphData.symbols.slice(0,100);
const byId=new Map(symbols.map(s=>[s.id,s]));
const edges=graphData.edges.filter(e=>byId.has(e.from)&&byId.has(e.to));
const cx=w/2,cy=h/2;
const positions=new Map();
symbols.forEach((s,i)=>{
const angle=(i/symbols.length)*Math.PI*2;
const r=Math.min(w,h)*0.35;
positions.set(s.id,[cx+Math.cos(angle)*r,cy+Math.sin(angle)*r]);
});
let html='';
edges.forEach(e=>{
const[from,to]=positions.get(e.from),positions.get(e.to);
if(!from||!to)return;
html+=`<line class="edge-line ${e.kind}" x1="${from[0]}" y1="${from[1]}" x2="${to[0]}" y2="${to[1]}"/>`;
});
symbols.forEach(s=>{
const[x,y]=positions.get(s.id);
const colors={function:'#58a6ff',method:'#58a6ff',class:'#3fb950',struct:'#3fb950',enum:'#d29922',trait:'#d29922',interface:'#3fb950',module:'#8b949e',constant:'#f85149',variable:'#f85149',type_alias:'#d29922',test:'#3fb950'};
html+=`<g class="node" data-id="${s.id}"><circle cx="${x}" cy="${y}" r="6" fill="${colors[s.kind]||'#8b949e'}" stroke="var(--border)"/><text x="${x+10}" y="${y+4}">${esc(s.name)}</text></g>`;
});
svg.innerHTML=html;
svg.querySelectorAll('.node').forEach(n=>n.onclick=()=>selectSymbol(Number(n.dataset.id)));
}
function renderSymbolList(){
const list=document.getElementById('symList');
const symbols=graphData.symbols.slice(0,200);
list.innerHTML=symbols.map(s=>`<li class="sym-item" data-id="${s.id}"><span class="sym-kind">${s.kind}</span><span>${esc(s.qualified_name)}</span></li>`).join('');
list.querySelectorAll('.sym-item').forEach(item=>item.onclick=()=>selectSymbol(Number(item.dataset.id)));
}
function selectSymbol(id){
const s=graphData.symbols.find(x=>x.id===id);
if(!s)return;
selectedSymbol=s;
document.querySelectorAll('.sym-item').forEach(item=>{
item.classList.toggle('active',Number(item.dataset.id)===id);
});
const file=graphData.files.find(f=>f.id===s.file_id);
const edges=graphData.edges.filter(e=>e.from===id||e.to===id);
const inspector=document.getElementById('inspector');
let edgeRows=edges.map(e=>{
const outgoing=e.from===id;
const targetId=outgoing?e.to:e.from;
const target=graphData.symbols.find(x=>x.id===targetId)||graphData.files.find(f=>f.id===targetId);
const name=target?(target.qualified_name||target.path||target.name):String(targetId);
return `<tr><td>${outgoing?'out':'in'}</td><td>${e.kind}</td><td><code>${esc(name)}</code></td><td>${e.confidence_milli}</td></tr>`;
}).join('');
inspector.innerHTML=`<h3><code>${esc(s.qualified_name)}</code></h3>
<div class="field"><div class="field-label">Kind</div><div class="field-value">${s.kind}</div></div>
<div class="field"><div class="field-label">Location</div><div class="field-value">${esc(file?file.path:'?')}:${s.line_start}-${s.line_end}</div></div>
<div class="field"><div class="field-label">Signature</div><div class="field-value"><pre>${esc(s.signature)}</pre></div></div>
${s.doc?`<div class="field"><div class="field-label">Documentation</div><div class="field-value">${esc(s.doc)}</div></div>`:''}
<div class="field"><div class="field-label">Relationships (${edges.length})</div><table class="edges-table"><tr><th>Dir</th><th>Kind</th><th>Target</th><th>Conf</th></tr>${edgeRows}</table></div>`;
}
async function search(query){
if(!query.trim())return;
log('Searching: '+query);
const data=await api('/search?q='+encodeURIComponent(query)+'&limit=50');
if(graphData&&data.length){
graphData._searchResults=data;
log('Found '+data.length+' results');
}
}
async function buildContext(query){
if(!query.trim())return;
log('Building context for: '+query);
const data=await api('/context?q='+encodeURIComponent(query)+'&max_tokens=4000');
const panel=document.getElementById('bottomPanel');
panel.innerHTML='<h4>Context Bundle</h4><div>Estimated tokens: '+data.estimated_tokens+' | Items: '+data.items.length+'</div>';
data.items.forEach(item=>{
panel.innerHTML+=`<div class="ctx-item"><div class="ctx-header"><span class="ctx-symbol">${esc(item.symbol)}</span><span class="ctx-score">score: ${item.score_milli} | ${item.path}:${item.line_start}-${item.line_end}</span></div><pre>${esc(item.content)}</pre></div>`;
});
if(data.warnings&&data.warnings.length){
panel.innerHTML+='<div style="color:var(--warn);margin-top:8px">Warnings: '+data.warnings.join(', ')+'</div>';
}
}
function log(msg){
const out=document.getElementById('consoleOutput');
const time=new Date().toLocaleTimeString();
out.innerHTML+=`<div>[${time}] ${esc(msg)}</div>`;
out.parentElement.scrollTop=out.parentElement.scrollHeight;
}
function esc(s){return String(s||'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));}
document.getElementById('searchInput').addEventListener('keydown',e=>{
if(e.key==='Enter'){search(e.target.value);}
});
document.getElementById('contextBtn').onclick=()=>{
const q=document.getElementById('searchInput').value;
buildContext(q);
};
document.getElementById('updateBtn').onclick=async()=>{
log('Updating index...');
try{await api('/actions',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'update',input:{}})});log('Index updated');loadGraph();}catch(e){log('Update failed: '+e.message);}
};
document.querySelectorAll('.tab').forEach(tab=>{
tab.onclick=()=>{
document.querySelectorAll('.tab').forEach(t=>t.classList.remove('active'));
tab.classList.add('active');
currentTab=tab.dataset.tab;
log('Switched to '+currentTab+' tab');
};
});
loadGraph();
</script>
</body>
</html>"#.to_string()
}
