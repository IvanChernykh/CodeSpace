import test from 'node:test';
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFile,mkdir} from 'node:fs/promises';
import {build} from 'esbuild';
import {chromium} from '@playwright/test';
import {createGateway} from '../src/server.mjs';
import {fixture,binary} from './fixture.mjs';
test('MCP Apps bridge end-to-end at phone, tablet and desktop widths',async()=>{
 const f=await fixture();let gateway,web,browser;
 try{
  gateway=await createGateway({workspace:f.project,binary});
  const widget=await readFile('dist/widget.html','utf8');
  const hostJs=(await build({entryPoints:['test/host.mjs'],bundle:true,platform:'browser',format:'esm',write:false})).outputFiles[0].text;
  web=createServer(async(req,res)=>{
   if(req.url==='/call'){
    let data='';for await(const chunk of req)data+=chunk;
    const {name,arguments:args}=JSON.parse(data);let result;
    try{result=await gateway.call(name,args);}catch{result={isError:true,content:[{type:'text',text:'Invalid request'}]};}
    res.setHeader('Content-Type','application/json');res.end(JSON.stringify(result));
   }else if(req.url==='/widget'){res.setHeader('Content-Type','text/html');res.end(widget);}
   else if(req.url==='/host.js'){res.setHeader('Content-Type','text/javascript');res.end(hostJs);}
   else{res.setHeader('Content-Type','text/html');res.end('<!doctype html><style>body{margin:0}iframe{width:100%;height:100vh;border:0}</style><iframe title="CodeSpace" sandbox="allow-scripts allow-same-origin"></iframe><script type="module" src="/host.js"></script>');}
  });
  await new Promise(resolve=>web.listen(0,'127.0.0.1',resolve));
  browser=await chromium.launch(process.env.CSE_BROWSER_CHANNEL?{channel:process.env.CSE_BROWSER_CHANNEL}:{});
  await mkdir('../../artifacts/v3-e2e',{recursive:true});
  for(const [width,height]of [[390,844],[768,1024],[1440,1000]]){
   const page=await browser.newPage({viewport:{width,height},deviceScaleFactor:1});
   const errors=[];const messages=[];page.on('pageerror',e=>errors.push(e.message));page.on('console',m=>messages.push(m.text()));
   await page.goto(`http://127.0.0.1:${web.address().port}`);
   const frame=page.frameLocator('iframe');
   try { await frame.locator('#connection').filter({hasText:'Connected'}).waitFor({timeout:12000}); }
   catch { throw Error(JSON.stringify({errors,messages,text:await frame.locator('body').innerText().catch(()=>'' )})); }
   await frame.locator('#query').fill('search titles');await frame.getByRole('button',{name:'Build context',exact:true}).click();
   try {await frame.locator('.evidence').first().waitFor({timeout:10000});}
   catch {throw Error(JSON.stringify({errors,messages,text:await frame.locator('body').innerText()}));}
   assert(await frame.locator('.evidence').count()>=2);
   assert.equal(await frame.locator('body').evaluate(el=>el.scrollWidth>innerWidth),false);
   await page.screenshot({path:`../../artifacts/v3-e2e/chatgpt-${width}.png`});
   await frame.locator('#useContext').click();await frame.locator('#useContext').filter({hasText:'Context attached'}).waitFor();
   assert(await page.evaluate(()=>window.attachedContext.content[0].text.includes('Context Pack')));
   await frame.getByRole('button',{name:'Read source',exact:true}).click();await frame.locator('#file').fill('src/lib.rs');
   await frame.locator('#line').fill('2');await frame.locator('#lines').fill('1');await frame.getByRole('button',{name:'Read',exact:true}).click();
   await frame.locator('#readResult').filter({hasText:'2 |'}).waitFor();
   await frame.getByRole('button',{name:'Tasks',exact:true}).click();await frame.locator('#tasksResult').filter({hasText:'Verify title matching'}).waitFor();
   await frame.getByRole('button',{name:'Checkpoints',exact:true}).click();await frame.getByRole('button',{name:'Resume',exact:true}).click();
   await frame.locator('#runResult').filter({hasText:'Review title matching'}).waitFor();
   await frame.getByRole('button',{name:'Use checkpoint in chat',exact:true}).click();
   await frame.locator('#attachRun').filter({hasText:'Checkpoint attached'}).waitFor();
   assert(await page.evaluate(()=>window.attachedContext.content[0].text.includes('Trust boundary')));
   assert.deepEqual(errors,[]);await page.close();
  }
 }finally{await browser?.close();web?.closeAllConnections();if(web)await new Promise(resolve=>web.close(resolve));await gateway?.close();await f.close();}
});
