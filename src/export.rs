use crate::model::GraphIndex;
use crate::util::{html_escape, json_escape};

pub fn to_json(graph: &GraphIndex) -> String {
    let mut output = format!(
        "{{\"schema_version\":{},\"project_root\":\"{}\",\"created_unix_ms\":{},\"updated_unix_ms\":{},\"files\":[",
        graph.schema_version,
        json_escape(&graph.project_root),
        graph.created_unix_ms,
        graph.updated_unix_ms
    );
    for (index, file) in graph.files.values().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"id\":{},\"path\":\"{}\",\"language\":\"{}\",\"hash\":{},\"bytes\":{},\"modified_unix_ms\":{},\"line_count\":{}}}",
            file.id,
            json_escape(&file.path),
            json_escape(&file.language),
            file.hash,
            file.bytes,
            file.modified_unix_ms,
            file.line_count
        ));
    }
    output.push_str("],\"symbols\":[");
    for (index, symbol) in graph.symbols.values().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"id\":{},\"file_id\":{},\"name\":\"{}\",\"qualified_name\":\"{}\",\"kind\":\"{}\",\"line_start\":{},\"line_end\":{},\"signature\":\"{}\",\"doc\":\"{}\",\"complexity\":{}}}",
            symbol.id,
            symbol.file_id,
            json_escape(&symbol.name),
            json_escape(&symbol.qualified_name),
            symbol.kind.as_str(),
            symbol.line_start,
            symbol.line_end,
            json_escape(&symbol.signature),
            json_escape(&symbol.doc),
            symbol.complexity
        ));
    }
    output.push_str("],\"edges\":[");
    for (index, edge) in graph.edges.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"from\":{},\"to\":{},\"kind\":\"{}\",\"confidence_milli\":{}}}",
            edge.from,
            edge.to,
            edge.kind.as_str(),
            edge.confidence_milli
        ));
    }
    output.push_str("]}");
    output
}

pub fn to_graphviz(graph: &GraphIndex) -> String {
    let mut output = String::from("digraph codespace {\n  rankdir=LR;\n  node [shape=box, fontname=\"monospace\"];\n");
    for file in graph.files.values() {
        output.push_str(&format!(
            "  n{} [label=\"{}\", shape=folder];\n",
            file.id,
            dot_escape(&file.path)
        ));
    }
    for symbol in graph.symbols.values() {
        output.push_str(&format!(
            "  n{} [label=\"{}\\n{}\"];\n",
            symbol.id,
            dot_escape(&symbol.qualified_name),
            symbol.kind.as_str()
        ));
    }
    for edge in &graph.edges {
        output.push_str(&format!(
            "  n{} -> n{} [label=\"{}\", weight={}];\n",
            edge.from,
            edge.to,
            edge.kind.as_str(),
            edge.confidence_milli
        ));
    }
    output.push_str("}\n");
    output
}

pub fn to_html(graph: &GraphIndex) -> String {
    let json = to_json(graph).replace("</", "<\\/");
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>CodeSpace — {}</title>
<style>
body{{font-family:system-ui,sans-serif;margin:0;background:#101317;color:#edf2f7}}
header{{padding:20px 28px;border-bottom:1px solid #2d3748;position:sticky;top:0;background:#101317}}
main{{display:grid;grid-template-columns:320px 1fr;min-height:calc(100vh - 80px)}}
aside{{padding:20px;border-right:1px solid #2d3748;overflow:auto}}section{{padding:20px;overflow:auto}}
input,select{{width:100%;box-sizing:border-box;padding:10px;margin:6px 0 14px;background:#171b22;color:#fff;border:1px solid #3a4556;border-radius:6px}}
.card{{padding:12px;margin:8px 0;background:#171b22;border:1px solid #2d3748;border-radius:8px;cursor:pointer}}
.card:hover{{border-color:#718096}}.muted{{color:#a0aec0}}code{{font-family:ui-monospace,monospace}}
table{{border-collapse:collapse;width:100%}}td,th{{text-align:left;border-bottom:1px solid #2d3748;padding:8px}}
</style>
</head>
<body>
<header><strong>CodeSpace</strong> <span class="muted">{}</span></header>
<main><aside><input id="q" placeholder="Search symbols"><select id="kind"><option value="">All kinds</option></select><div id="list"></div></aside><section><div id="detail"><h2>Graph summary</h2><p>{} files, {} symbols, {} edges.</p><p class="muted">Select a symbol to inspect its incoming and outgoing relationships.</p></div></section></main>
<script>
const graph={json};
const byId=new Map([...graph.files,...graph.symbols].map(x=>[x.id,x]));
const kinds=[...new Set(graph.symbols.map(s=>s.kind))].sort();
const kind=document.getElementById('kind'); kinds.forEach(k=>kind.add(new Option(k,k)));
const q=document.getElementById('q'),list=document.getElementById('list'),detail=document.getElementById('detail');
function render(){{const needle=q.value.toLowerCase(),k=kind.value;const rows=graph.symbols.filter(s=>(!k||s.kind===k)&&(!needle||s.qualified_name.toLowerCase().includes(needle))).slice(0,300);list.innerHTML=rows.map(s=>`<div class="card" data-id="${{s.id}}"><code>${{escapeHtml(s.qualified_name)}}</code><div class="muted">${{s.kind}}</div></div>`).join('');document.querySelectorAll('.card').forEach(el=>el.onclick=()=>show(Number(el.dataset.id)));}}
function show(id){{const s=byId.get(id),file=byId.get(s.file_id);const edges=graph.edges.filter(e=>e.from===id||e.to===id);detail.innerHTML=`<h2><code>${{escapeHtml(s.qualified_name)}}</code></h2><p>${{s.kind}} · ${{escapeHtml(file?.path||'')}}:${{s.line_start}}-${{s.line_end}}</p><pre>${{escapeHtml(s.signature)}}</pre><h3>Relationships</h3><table><tr><th>Direction</th><th>Kind</th><th>Target</th></tr>${{edges.map(e=>{{const outgoing=e.from===id,t=byId.get(outgoing?e.to:e.from);return `<tr><td>${{outgoing?'out':'in'}}</td><td>${{e.kind}}</td><td><code>${{escapeHtml(t?.qualified_name||t?.path||String(outgoing?e.to:e.from))}}</code></td></tr>`}}).join('')}}</table>`;}}
function escapeHtml(v){{return String(v).replace(/[&<>"']/g,c=>({{'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}}[c]));}}
q.oninput=render;kind.onchange=render;render();
</script>
</body></html>"#,
        html_escape(&graph.project_root),
        html_escape(&graph.project_root),
        graph.files.len(),
        graph.symbols.len(),
        graph.edges.len()
    )
}

fn dot_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}
