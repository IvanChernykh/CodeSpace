import {mkdir,writeFile,realpath} from 'node:fs/promises';
import {execFileSync} from 'node:child_process';
import path from 'node:path';
import {homedir} from 'node:os';
const args=process.argv.slice(2);const value=key=>args[args.indexOf(key)+1];
if(!args.includes('--workspace')||!path.isAbsolute(value('--workspace'))){console.error('--workspace must name one absolute repository path.');process.exit(1);}
const workspace=await realpath(value('--workspace'));const binary=args.includes('--binary')?value('--binary'):'cse';
if(execFileSync(binary,['--version'],{encoding:'utf8',windowsHide:true}).trim()!=='cse 3.0.0')throw Error('Install cse 3.0.0 first.');
execFileSync(binary,['doctor','--path',workspace],{stdio:'inherit',windowsHide:true});
const file=process.env.CSE_PLUGIN_CONFIG??path.join(process.env.PLUGIN_DATA??path.join(homedir(),'.config','codespace'),'plugin.json');
await mkdir(path.dirname(file),{recursive:true});
await writeFile(file,JSON.stringify({workspace,binary,allowWrites:args.includes('--allow-writes')},null,2)+'\n',{mode:0o600});
console.log('CodeSpace configured for one repository. Restart the plugin connection.');
