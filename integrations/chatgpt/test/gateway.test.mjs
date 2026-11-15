import test from 'node:test';
import assert from 'node:assert/strict';
import {Client} from '@modelcontextprotocol/sdk/client/index.js';
import {StreamableHTTPClientTransport} from '@modelcontextprotocol/sdk/client/streamableHttp.js';
import {createGateway,startHttp,RESOURCE} from '../src/server.mjs';
import {fixture,binary} from './fixture.mjs';

test('real stdio core + authenticated HTTP: discovery, cells, source, checkpoints and isolation',async()=>{
 const f=await fixture();let gateway,http,client;
 const token='e2e-private-credential-'.repeat(3);
 try{
  gateway=await createGateway({workspace:f.project,binary});
  http=await startHttp(gateway,{port:0,token});
  const url=`http://127.0.0.1:${http.http.address().port}/mcp`;
  assert.equal((await fetch(url,{method:'POST',headers:{'Content-Type':'application/json'},body:'{}'})).status,401);
  assert.equal((await fetch(url,{method:'POST',headers:{'Authorization':`Bearer ${token}`,'Content-Type':'application/json',Origin:'https://attacker.example'},body:'{}'})).status,403);
  assert.equal((await fetch(url,{method:'POST',headers:{'Authorization':`Bearer ${token}`,'Content-Type':'application/json'},body:'{' })).status,400);
  assert.equal((await fetch(url,{method:'GET',headers:{'Authorization':`Bearer ${token}`}})).status,405);
  client=new Client({name:'v3-e2e',version:'1'});
  await client.connect(new StreamableHTTPClientTransport(new URL(url),{requestInit:{headers:{Authorization:`Bearer ${token}`}}}));
  assert.equal(client.getServerVersion().version,'3.0.0');
  const tools=(await client.listTools()).tools;
  assert(tools.some(t=>t.name==='cse_open'&&t._meta.ui.resourceUri===RESOURCE));
  assert(!tools.some(t=>['cse_task_add','cse_run','cse_github_status'].includes(t.name)));
  assert(tools.every(t=>t.annotations.readOnlyHint));
  const opened=await client.callTool({name:'cse_open',arguments:{}});
  assert.equal(opened.structuredContent.read_only,true);
  assert(opened.structuredContent.status.files>=2);
  const pack=await client.callTool({name:'cse_context_pack',arguments:{query:'search titles',max_tokens:1600}});
  assert.equal(pack.isError,false);assert(pack.structuredContent.cells.length);assert(pack.structuredContent.items.length);
  assert(pack.structuredContent.estimated_tokens<=1600);
  const prompt=await client.callTool({name:'cse_context',arguments:{query:'search titles',max_tokens:1600}});
  assert(prompt.content[0].text.includes('Context Pack'));
  assert(Math.ceil([...prompt.content[0].text].length/4)<=1600);
  const source=await client.callTool({name:'cse_read',arguments:{file:'src/lib.rs',start_line:2,max_lines:1}});
  assert(source.content[0].text.includes('2 |'));assert(!source.content[0].text.includes('1 |'));
  assert((await client.callTool({name:'cse_read',arguments:{file:'../../outside.txt'}})).isError);
  assert((await client.callTool({name:'cse_read',arguments:{file:'src/lib.rs',max_lines:0}})).isError);
  assert((await client.callTool({name:'cse_task_remove',arguments:{id:'fake'}})).isError);
  assert((await client.callTool({name:'cse_bulk_read',arguments:{question:'search',paths:['src/lib.rs'],worker:'portal'}})).isError);
  const runs=await client.callTool({name:'cse_checkpoints',arguments:{operation:'list'}});
  assert.equal(runs.structuredContent.runs[0].id,'review');
  const resume=await client.callTool({name:'cse_checkpoints',arguments:{operation:'resume',id:'review'}});
  assert(resume.structuredContent.prompt_bytes<=18432);
  assert((await client.callTool({name:'cse_checkpoints',arguments:{operation:'patch',id:'review'}})).isError);
  const resource=await client.readResource({uri:RESOURCE});assert(resource.contents[0].text.includes('<title>CodeSpace</title>'));
  assert(!JSON.stringify(opened).includes(f.project));assert(!JSON.stringify(pack).includes(token));
 }finally{await client?.close();await http?.close();await gateway?.close();await f.close();}
});

test('explicit write mode retains old task lifecycle and checkpoint CAS',async()=>{
 const f=await fixture();let gateway;
 try{
  gateway=await createGateway({workspace:f.project,binary,readOnly:false});
  const add=await gateway.call('cse_task_add',{title:'Verify source',priority:'medium'});assert(!add.isError);
  const list=await gateway.call('cse_task_list',{});const task=list.structuredContent.tasks.find(t=>t.title==='Verify source');assert(task);
  assert(!(await gateway.call('cse_task_status',{id:task.id,status:'done'})).isError);
  assert(!(await gateway.call('cse_task_remove',{id:task.id})).isError);
  const patch=await gateway.call('cse_run',{operation:'patch',id:'review',expected_revision:0,patch:{facts:['Verified title matching'],phase:'verify'}});assert.equal(patch.structuredContent.revision,1);
  assert((await gateway.call('cse_run',{operation:'patch',id:'review',expected_revision:0,patch:{phase:'stale'}})).isError);
  assert.equal((await gateway.call('cse_run',{operation:'show',id:'review'})).structuredContent.state.phase,'verify');
 }finally{await gateway?.close();await f.close();}
});

test('HTTP refuses missing credentials and insecure remote origins',async()=>{
 await assert.rejects(startHttp({},{}),/TOKEN/);
 await assert.rejects(startHttp({},{token:'x'.repeat(40),publicUrl:'http://public.example'}),/HTTPS/);
 await assert.rejects(startHttp({},{token:'x'.repeat(40),host:'0.0.0.0'}),/HTTPS/);
});
