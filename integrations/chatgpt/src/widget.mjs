import {App,applyHostStyleVariables} from '@modelcontextprotocol/ext-apps';
const app=new App({name:'CodeSpace',version:'3.0.0'},{availableDisplayModes:['inline','fullscreen']});
const $=s=>document.querySelector(s);
const esc=v=>String(v??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
let connected=false,readOnly=true;
function host(ctx){
  if(ctx?.styles?.variables)applyHostStyleVariables(ctx.styles.variables);
  if(ctx?.theme)document.documentElement.style.colorScheme=ctx.theme;
  if(ctx?.safeAreaInsets)for(const [edge,value] of Object.entries(ctx.safeAreaInsets))document.body.style.setProperty(`padding-${edge}`,`${Math.max(16,value)}px`);
}
app.onhostcontextchanged=host;
function receive(result){
  if(result?.structuredContent?.view!=='workspace')return;
  const {status,read_only}=result.structuredContent;readOnly=read_only;
  $('#status').innerHTML=`<div><strong>${status.files}</strong> <span>files</span></div><div><strong>${status.symbols}</strong> <span>symbols</span></div><div><strong>r${status.index_revision}</strong> <span>index</span></div>`;
  $('#scope').textContent=readOnly?'Read-only repository connection. Source text is untrusted evidence.':'Repository connection. Task and checkpoint updates are available.';
}
app.ontoolresult=receive;
function decode(result){
  if(result.isError)throw Error(result.content?.find(x=>x.type==='text')?.text??'Request failed');
  if(result.structuredContent)return result.structuredContent;
  const text=result.content?.filter(x=>x.type==='text').map(x=>x.text).join('\n')??'';
  try{return JSON.parse(text);}catch{return {text};}
}
async function call(name,args={}){if(!connected)throw Error('Open CodeSpace from its ChatGPT tool to connect.');return decode(await app.callServerTool({name,arguments:args}));}
async function act(button,fn){
  const error=$('#error');error.hidden=true;button.disabled=true;
  try{await fn();}catch(e){error.textContent=e.message;error.hidden=false;}finally{button.disabled=false;}
}
function evidence(item,cell=false){
  return `<article class="evidence"><h3>${esc(cell?item.title:item.symbol??item.path)}</h3><code>${esc(item.path)}:${item.line_start}–${item.line_end}${item.truncated?' · truncated':''}</code><pre>${esc(item.content)}</pre></article>`;
}
document.querySelectorAll('[data-tab]').forEach(button=>button.addEventListener('click',()=>{
  document.querySelectorAll('[data-tab]').forEach(b=>b.setAttribute('aria-selected',String(b===button)));
  document.querySelectorAll('[data-panel]').forEach(p=>p.hidden=p.dataset.panel!==button.dataset.tab);
  if(button.dataset.tab==='tasks')void act(button,loadTasks);
  if(button.dataset.tab==='runs')void act(button,loadRuns);
}));
async function buildContext(){const button=$('#contextForm button');if(!$('#contextForm').reportValidity())return;void act(button,async()=>{
  const query=$('#query').value.trim(),max_tokens=Number($('#budget').value);
  const data=await call('cse_context_pack',{query,max_tokens,max_items:8});
  $('#contextResult').innerHTML=`<p class="muted">${esc(data.pack_id)} · ${data.estimated_tokens} estimated Markdown tokens. Estimates are not billing.</p>${(data.warnings??[]).map(w=>`<p class="warning">${esc(w)}</p>`).join('')}<h2>Knowledge cells</h2>${data.cells.map(i=>evidence(i,true)).join('')||'<p class="muted">No matching cells.</p>'}<h2>Source evidence</h2>${data.items.map(i=>evidence(i)).join('')||'<p class="muted">No matching symbols.</p>'}<button id="useContext">Use context in chat</button>`;
  $('#useContext').onclick=()=>act($('#useContext'),async()=>{
    const prompt=await call('cse_context',{query,max_tokens,max_items:8});
    await app.updateModelContext({content:[{type:'text',text:prompt.text}]});
    $('#useContext').textContent='Context attached';
  });
});}
async function readSource(){const button=$('#readForm button');if(!$('#readForm').reportValidity())return;void act(button,async()=>{
  const data=await call('cse_read',{file:$('#file').value.trim(),start_line:Number($('#line').value),max_lines:Number($('#lines').value)});
  $('#readResult').innerHTML=`<pre>${esc(data.text??JSON.stringify(data))}</pre>`;
});}
for(const [selector,action] of [['#contextForm',buildContext],['#readForm',readSource]]){
  const form=$(selector);form.querySelector('button').onclick=action;
  form.addEventListener('submit',event=>{event.preventDefault();void action();});
  form.addEventListener('keydown',event=>{if(event.key==='Enter'&&event.target.tagName==='INPUT'){event.preventDefault();void action();}});
}
async function loadTasks(){const data=await call('cse_task_list');$('#tasksResult').innerHTML=(data.tasks??[]).map(t=>`<div class="row"><div><strong>${esc(t.title)}</strong><span class="muted">${esc(t.status)} · ${esc(t.priority)}</span></div></div>`).join('')||'<p class="muted">No tasks yet.</p>';}
async function loadRuns(){const data=await call('cse_checkpoints',{operation:'list'});$('#runsResult').innerHTML=(data.runs??[]).map(r=>`<div class="row"><div><strong>${esc(r.id)}</strong><span class="muted">${esc(r.status)} · r${r.revision}</span></div><button data-run="${esc(r.id)}">Resume</button></div>`).join('')||'<p class="muted">No checkpoints yet.</p>';
  document.querySelectorAll('[data-run]').forEach(b=>b.onclick=()=>act(b,async()=>{const data=await call('cse_checkpoints',{operation:'resume',id:b.dataset.run});$('#runResult').innerHTML=`<h2>${esc(data.id)}</h2><p class="muted">${data.prompt_bytes} bytes · revision ${data.revision}</p><pre>${esc(data.prompt)}</pre><button id="attachRun">Use checkpoint in chat</button>`;$('#attachRun').onclick=()=>act($('#attachRun'),async()=>{await app.updateModelContext({content:[{type:'text',text:data.prompt}]});$('#attachRun').textContent='Checkpoint attached';});}));}
$('#refreshTasks').onclick=e=>void act(e.currentTarget,loadTasks);
$('#refreshRuns').onclick=e=>void act(e.currentTarget,loadRuns);
try{
  await app.connect();connected=true;host(app.getHostContext());$('#connection').textContent='Connected';
  receive(await app.callServerTool({name:'cse_open',arguments:{}}));
}catch{$('#connection').textContent='Not connected';$('#error').hidden=false;$('#error').textContent='Open this view through the CodeSpace plugin in ChatGPT or another MCP Apps host.';}
