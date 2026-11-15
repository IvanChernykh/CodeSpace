import {readFile} from 'node:fs/promises';
import path from 'node:path';
import {homedir} from 'node:os';
const fallback=path.join(homedir(),'.config','codespace','plugin.json');
let location=process.env.CSE_PLUGIN_CONFIG??fallback;
if(!process.env.CSE_PLUGIN_CONFIG&&process.env.PLUGIN_DATA){
  const owned=path.join(process.env.PLUGIN_DATA,'plugin.json');
  try{await readFile(owned);location=owned;}catch{/* Terminal setup uses the user-wide fallback. */}
}
try{
  const config=JSON.parse(await readFile(location,'utf8'));
  if(!path.isAbsolute(config.workspace))throw Error('invalid workspace');
  process.env.CSE_WORKSPACE=config.workspace;
  if(config.binary)process.env.CSE_BINARY=config.binary;
  process.env.CSE_ALLOW_WRITES=config.allowWrites===true?'1':'0';
  process.argv.push('--stdio');
  const {main}=await import('./server.mjs');await main();
}catch{console.error('CodeSpace needs setup. Run node <plugin>/scripts/setup.mjs --workspace <absolute-repository-path> --binary <cse-executable>.');process.exitCode=1;}
