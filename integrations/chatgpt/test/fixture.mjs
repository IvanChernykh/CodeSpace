import {mkdtemp,writeFile,mkdir,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {execFileSync} from 'node:child_process';
export const binary=process.env.CSE_BINARY??path.resolve('../../target/v3-verify/debug/cse'+(process.platform==='win32'?'.exe':''));
export async function fixture(){
  const root=await mkdtemp(path.join(tmpdir(),'cse-v3-e2e-'));
  const project=path.join(root,'repo');await mkdir(project);await mkdir(path.join(project,'src'));
  const call=(...args)=>execFileSync(binary,[...args,'--path',project],{encoding:'utf8',timeout:30000,windowsHide:true,env:{...process.env,HOME:root,USERPROFILE:root}});
  await writeFile(path.join(project,'src/lib.rs'),'pub fn search_titles(query: &str) -> bool {\n    !query.trim().is_empty()\n}\n');
  await writeFile(path.join(project,'CODESPACE.md'),'<!-- cse:cell\nid: catalogue.order\ntitle: Preserve catalogue order\nstatus: accepted\ntags: search,titles\npaths: src/lib.rs\nsymbols: search_titles\n-->\nSearch titles in their original shelf order.\n<!-- /cse:cell -->\n');
  call('init');call('task','add','--title','Verify title matching','--priority','high');
  call('run','start','review','--objective','Review title matching');
  return {root,project,call,close:()=>rm(root,{recursive:true,force:true})};
}
