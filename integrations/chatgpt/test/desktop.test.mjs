import test from 'node:test';
import assert from 'node:assert/strict';
import {spawn,execFileSync} from 'node:child_process';
import {writeFile,mkdir} from 'node:fs/promises';
import path from 'node:path';
import {chromium} from '@playwright/test';
import {fixture,binary} from './fixture.mjs';

async function nativeServer(f,mode){
 const child=spawn(binary,['serve',mode,'--path',f.project,'--port','0'],{env:{...process.env,HOME:f.root,USERPROFILE:f.root,GH_CONFIG_DIR:path.join(f.root,'gh'),GH_TOKEN:'',GITHUB_TOKEN:''},windowsHide:true,stdio:['ignore','ignore','pipe']});
 let buffer='';
 const url=await new Promise((resolve,reject)=>{
  const timeout=setTimeout(()=>reject(Error('Native server startup timeout')),10000);
  child.once('exit',()=>{clearTimeout(timeout);reject(Error('Native server exited'));});
  child.stderr.on('data',chunk=>{buffer+=chunk;const found=buffer.match(/listening on (http:\/\/127\.0\.0\.1:\d+)/);if(found){clearTimeout(timeout);resolve(found[1]);}});
 });
 return {child,url};
}

test('existing desktop flows end-to-end: all views, CRUD, skills, settings, impact and new Studio',async()=>{
 const f=await fixture();let native,browser;
 try{
  const git=(...args)=>execFileSync('git',args,{cwd:f.project,windowsHide:true,stdio:'pipe'});
  git('init','-q');git('config','user.name','Demo');git('config','user.email','demo@example.com');git('add','.');git('commit','-qm','baseline');
  await writeFile(path.join(f.project,'src/lib.rs'),'pub fn search_titles(query: &str) -> bool {\n    query.trim().len() > 1\n}\n');
  git('add','src/lib.rs');git('commit','-qm','match nonempty titles');f.call('update');f.call('remember','--summary','Preserve shelf order','--file','src/lib.rs','--tags','catalogue');
  native=await nativeServer(f,'--dashboard');
  browser=await chromium.launch(process.env.CSE_BROWSER_CHANNEL?{channel:process.env.CSE_BROWSER_CHANNEL}:{});
  const page=await browser.newPage({viewport:{width:1440,height:1000}});page.setDefaultTimeout(10000);
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.goto(native.url);
  await page.locator('[data-overview-file]').first().waitFor();
  const nav=page.locator('[data-nav]');assert.equal(await nav.count(),11);
  const open=async name=>{await page.locator(`[data-nav="${name}"]`).click();await page.locator(`.is-active[data-view="${name}"]`).waitFor();};
  await open('graph');await page.locator('.file-node').first().waitFor();await page.locator('#graphFilter').fill('lib.rs');
  await page.locator('.file-node').filter({hasText:'lib.rs'}).dispatchEvent('click');await page.locator('#readSelectedFileBtn').click();await page.locator('#selectedFileSource').filter({hasText:'search_titles'}).waitFor();
  await open('context');await page.locator('#contextQuery').fill('search titles');await page.locator('#contextForm button[type=submit]').click();await page.locator('.context-cell').first().waitFor();
  await page.locator('#bulkQuestion').fill('search titles');await page.locator('#bulkPaths').fill('src/lib.rs');await page.locator('#bulkReadForm button[type=submit]').click();await page.locator('#bulkReadResults .code-result').waitFor();
  await page.locator('[data-run-id="review"]').click();await page.locator('[name=latest_observation]').fill('Verified source');await page.locator('[data-patch-run] button[type=submit]').click();await page.locator('#runDetail').filter({hasText:'revision 1'}).waitFor();
  await open('impact');await page.locator('#impactForm button[type=submit]').click();await page.locator('.risk-card').waitFor();assert((await page.locator('#impactResults').innerText()).includes('src/lib.rs'));
  await open('history');await page.locator('.decision-card').filter({hasText:'Preserve shelf order'}).waitFor();
  await page.locator('#rememberForm [name=summary]').fill('Verify before release');await page.locator('#rememberForm [name=rationale]').fill('Demo acceptance');await page.locator('#rememberForm button[type=submit]').click();await page.locator('.decision-card').filter({hasText:'Verify before release'}).waitFor();
  await open('tasks');await page.locator('#taskForm [name=title]').fill('Browser acceptance');await page.locator('#taskForm button[type=submit]').click();
  const task=page.locator('.task-card').filter({hasText:'Browser acceptance'});await task.waitFor();await task.getByRole('button',{name:'Start',exact:true}).click();await page.locator('[data-task-column=in_progress] .task-card').filter({hasText:'Browser acceptance'}).waitFor();
  await page.locator('[data-task-column=in_progress] .task-card').filter({hasText:'Browser acceptance'}).getByRole('button',{name:'Complete'}).click();await page.locator('[data-task-column=done] .task-card').filter({hasText:'Browser acceptance'}).waitFor();
  await page.locator('[data-task-column=done] .task-card').filter({hasText:'Browser acceptance'}).getByRole('button',{name:'Remove task'}).click();await task.waitFor({state:'detached'});
  await open('skills');const skill=page.locator('[data-skill-id]').first();await skill.waitFor({state:'attached'});const initial=await skill.isChecked();await Promise.all([page.waitForResponse(r=>r.url().includes('/skills/toggle')&&r.status()===200),page.locator('.switch').first().click()]);await page.reload();await open('skills');await page.locator('[data-skill-id]').first().waitFor({state:'attached'});assert.equal(await page.locator('[data-skill-id]').first().isChecked(),!initial);
  await open('settings');await page.locator('#settingForm [name=key]').fill('preview.lines');await page.locator('#settingForm [name=value]').fill('80');await page.locator('#settingForm button[type=submit]').click();await page.locator('#settingsTable').filter({hasText:'preview.lines'}).waitFor();
  await open('workspaces');await page.locator('#workspaceForm [name=path]').fill(f.project);await page.locator('#workspaceForm [name=name]').fill('Demo repository');await page.locator('#workspaceForm button[type=submit]').click();await page.locator('#workspaceList .workspace-card').filter({hasText:'Demo repository'}).waitFor();
  await open('mcp');await page.locator('#mcpList').filter({hasText:'No external MCP servers'}).waitFor();
  const childSource=path.join(f.root,'fixture-mcp.mjs');
  await writeFile(childSource,`import readline from 'node:readline';for await(const line of readline.createInterface({input:process.stdin})){const r=JSON.parse(line);if(r.method==='initialize')console.log(JSON.stringify({jsonrpc:'2.0',id:r.id,result:{protocolVersion:'2025-11-25',capabilities:{tools:{}},serverInfo:{name:'fixture',version:'1'}}}));if(r.method==='tools/list')console.log(JSON.stringify({jsonrpc:'2.0',id:r.id,result:{tools:[]}}));}`);
  await page.locator('#mcpForm [name=name]').fill('Demo protocol');await page.locator('#mcpForm [name=command]').fill(process.execPath);await page.locator('#mcpForm [name=args]').fill('"'+childSource.replaceAll('\\','/')+'"');await page.locator('#mcpForm button[type=submit]').click();
  const mcp=page.locator('.mcp-card').filter({hasText:'Demo protocol'});await mcp.waitFor();await mcp.getByRole('button',{name:'Start',exact:true}).click();await mcp.getByRole('button',{name:'Stop',exact:true}).waitFor();await mcp.getByRole('button',{name:'Stop',exact:true}).click();await mcp.getByRole('button',{name:'Start',exact:true}).waitFor();await mcp.getByRole('button',{name:'Remove MCP server'}).click();await mcp.waitFor({state:'detached'});
  await open('github');await page.locator('#githubPanel').filter({hasText:'Link GitHub'}).waitFor();
  await open('context');await page.locator('#contextQuery').fill('search titles');await page.locator('#contextForm button[type=submit]').click();await page.locator('.context-cell').first().waitFor();
  await page.setViewportSize({width:390,height:844});await page.waitForTimeout(100);
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
  assert.equal(await page.locator('.view.is-active').evaluate(el=>el.scrollWidth>el.clientWidth),false);
  await mkdir('../../artifacts/v3-e2e',{recursive:true});await page.locator('.toast').last().waitFor({state:'detached',timeout:10000}).catch(()=>{});await page.screenshot({path:'../../artifacts/v3-e2e/desktop-mobile-context.png'});
  assert.deepEqual(errors,[]);
 }finally{await browser?.close();native?.child.kill();await f.close();}
});

test('legacy loopback REST still serves search and bounded context',async()=>{
 const f=await fixture();let native;
 try{
  native=await nativeServer(f,'--rest');
  assert.equal((await fetch(native.url+'/v1/health')).status,200);
  const search=await(await fetch(native.url+'/v1/search?q=search_titles')).json();assert(search.length);
  const context=await(await fetch(native.url+'/v1/context?q=search+titles&max_tokens=1200')).json();assert(context.pack_id.startsWith('ctx-'));assert(context.estimated_tokens<=1200);
 }finally{native?.child.kill();await f.close();}
});
