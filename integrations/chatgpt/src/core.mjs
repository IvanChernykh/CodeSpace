import {spawn} from 'node:child_process';
import {realpath,stat} from 'node:fs/promises';
import path from 'node:path';
import {Client} from '@modelcontextprotocol/sdk/client/index.js';
import {StdioClientTransport} from '@modelcontextprotocol/sdk/client/stdio.js';

const SAFE_ENV=['PATH','Path','SystemRoot','WINDIR','TEMP','TMP','HOME','USERPROFILE','LOCALAPPDATA'];
export async function connectCore({workspace,binary='cse'}) {
  if(!workspace || !path.isAbsolute(workspace)) throw Error('CSE_WORKSPACE must name one absolute repository directory. Run plugin setup first.');
  const root=await realpath(workspace);
  if(!(await stat(path.join(root,'.codespace','index.csf'))).isFile()) throw Error('Index missing. Run cse init --path <repository>.');
  const env=Object.fromEntries(SAFE_ENV.filter(k=>process.env[k]).map(k=>[k,process.env[k]]));
  const transport=new StdioClientTransport({command:binary,args:['serve','--mcp','--path',root],env,stderr:'pipe',cwd:root});
  // The child only serves the selected root. Provider credentials and HTTP secrets
  // are intentionally absent from its environment.
  transport.stderr?.on('data',()=>{});
  const client=new Client({name:'codespace-chatgpt',version:'3.0.0'});
  try {
    await client.connect(transport);
    if(client.getServerVersion()?.version!=='3.0.0') throw Error('CodeSpace 3.0.0 is required; upgrade cse first.');
    return {client,root,close:()=>client.close()};
  } catch(error){await transport.close().catch(()=>{}); throw error;}
}

export function sanitizeText(value,root){
  const clean=s=>s.split(root).join('[workspace]').split(root.replaceAll('\\','/')).join('[workspace]');
  if(typeof value==='string')return clean(value);
  if(Array.isArray(value))return value.map(v=>sanitizeText(v,root));
  if(value&&typeof value==='object')return Object.fromEntries(Object.entries(value).map(([k,v])=>[k,sanitizeText(v,root)]));
  return value;
}

export const readOnlyNames=new Set(['cse_search','cse_context','cse_context_pack','cse_impact','cse_history','cse_read','cse_bulk_read','cse_task_list','cse_status']);
export const writeNames=new Set(['cse_task_add','cse_task_status','cse_task_remove']);
export const runtimeNames=new Set(['cse_run']);
