import {Client} from '@modelcontextprotocol/sdk/client/index.js';
import {StdioClientTransport} from '@modelcontextprotocol/sdk/client/stdio.js';
import path from 'node:path';
const root=process.argv[2];if(!root)throw Error('Pass the installed plugin directory');
const transport=new StdioClientTransport({command:process.execPath,args:[path.join(root,'server/launch.mjs')],stderr:'pipe'});
const client=new Client({name:'codespace-install-check',version:'3.0.0'});
try{
 await client.connect(transport);
 const tools=(await client.listTools()).tools;
 if(!tools.some(t=>t.name==='cse_open'))throw Error('UI entrypoint missing');
 if(!tools.some(t=>t.name==='cse_context'))throw Error('Context tool missing');
 const status=await client.callTool({name:'cse_status',arguments:{}});
 if(status.isError||status.structuredContent?.version!=='3.0.0')throw Error('Core identity mismatch');
 const view=await client.callTool({name:'cse_open',arguments:{}});
 const resource=await client.readResource({uri:view._meta.ui.resourceUri});
 if(!resource.contents[0].text.includes('<title>CodeSpace</title>'))throw Error('UI resource missing');
 console.log(`Installed CodeSpace 3.0.0 verified: ${tools.length} tools, selected repository available, UI resource readable.`);
}finally{await client.close();await transport.close();}
