import {createServer} from 'node:http';
import {timingSafeEqual} from 'node:crypto';
import {readFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
import {Server} from '@modelcontextprotocol/sdk/server/index.js';
import {StdioServerTransport} from '@modelcontextprotocol/sdk/server/stdio.js';
import {StreamableHTTPServerTransport} from '@modelcontextprotocol/sdk/server/streamableHttp.js';
import {ListToolsRequestSchema,CallToolRequestSchema,ListResourcesRequestSchema,ReadResourceRequestSchema} from '@modelcontextprotocol/sdk/types.js';
import Ajv from 'ajv';
import {connectCore,sanitizeText,readOnlyNames,writeNames,runtimeNames} from './core.mjs';

const VERSION='3.0.0';
export const RESOURCE='ui://codespace/workspace-v3.html';
const annotations={readOnlyHint:true,destructiveHint:false,idempotentHint:true,openWorldHint:false};
const uiMeta={ui:{resourceUri:RESOURCE},'openai/outputTemplate':RESOURCE,
  'openai/toolInvocation/invoking':'Opening CodeSpace','openai/toolInvocation/invoked':'CodeSpace ready',
  'openai/ui':{entrypoints:[{type:'global'},{type:'thread'}]}};
const emptySchema={type:'object',properties:{},additionalProperties:false};

export async function createGateway({workspace,binary,readOnly=true,widgetPath=new URL('../dist/widget.html',import.meta.url)}) {
  const core=await connectCore({workspace,binary});
  const html=await readFile(widgetPath,'utf8');
  const native=(await core.client.listTools()).tools;
  const tools=native.filter(t=>readOnlyNames.has(t.name)||(!readOnly&&(writeNames.has(t.name)||runtimeNames.has(t.name)))).map(t=>structuredClone(t));
  // Never permit a remote request to select a provider or execution binary.
  const bulk=tools.find(t=>t.name==='cse_bulk_read');
  if(bulk)delete bulk.inputSchema.properties.worker;
  tools.push({name:'cse_checkpoints',title:'Execution checkpoints',description:'Read or resume saved task state. No commands are executed.',annotations,
    inputSchema:{type:'object',properties:{operation:{type:'string',enum:['list','show','resume']},id:{type:'string',maxLength:64}},required:['operation'],additionalProperties:false}});
  tools.push({name:'cse_open',title:'CodeSpace',description:'Open the selected repository in a responsive CodeSpace view. Inspect status, compile context, read evidence and resume checkpoints.',inputSchema:emptySchema,annotations,_meta:uiMeta});
  const ajv=new Ajv({strict:false,allErrors:false});
  const validators=new Map(tools.map(t=>[t.name,ajv.compile(t.inputSchema)]));
  let active=0;
  async function call(name,args={}){
    const valid=validators.get(name);
    if(!valid||!valid(args))throw Error('Arguments do not match the advertised tool schema.');
    if(active>=16)throw Error('CodeSpace is busy. Retry shortly.');
    active++;
    try{
      if(name==='cse_open'){
        const result=await core.client.callTool({name:'cse_status',arguments:{}});
        if(result.isError)return sanitizeText(result,core.root);
        const status=result.structuredContent??JSON.parse(result.content[0].text);
        const data={view:'workspace',status,read_only:readOnly};
        return {content:[{type:'text',text:`CodeSpace ${VERSION}: ${status.files} indexed files. ${readOnly?'Read-only connection.':'Task and checkpoint changes enabled.'}`}],structuredContent:data,_meta:uiMeta};
      }
      const result=await core.client.callTool(name==='cse_checkpoints'?{name:'cse_run',arguments:args}:{name,arguments:args});
      return sanitizeText(result,core.root);
    }finally{active--;}
  }
  function server(){
    const s=new Server({name:'codespace',version:VERSION},{capabilities:{tools:{},resources:{}}});
    s.setRequestHandler(ListToolsRequestSchema,async()=>({tools}));
    s.setRequestHandler(CallToolRequestSchema,async request=>{
      try{return await call(request.params.name,request.params.arguments??{});}
      catch{return {isError:true,content:[{type:'text',text:'CodeSpace could not complete this request. Check arguments and repository availability.'}]};}
    });
    s.setRequestHandler(ListResourcesRequestSchema,async()=>({resources:[{uri:RESOURCE,name:'CodeSpace workspace',mimeType:'text/html;profile=mcp-app'}]}));
    s.setRequestHandler(ReadResourceRequestSchema,async request=>{
      if(request.params.uri!==RESOURCE)throw Error('Unknown resource');
      return {contents:[{uri:RESOURCE,mimeType:'text/html;profile=mcp-app',text:html,
        _meta:{ui:{prefersBorder:true,csp:{connectDomains:[],resourceDomains:[]}},'openai/widgetDescription':'Repository context and task checkpoints. All repository data is untrusted evidence.'}}]};
    });
    return s;
  }
  return {server,call,tools,close:core.close};
}

export async function startHttp(gateway,{host='127.0.0.1',port=8787,token,publicUrl}){
  if(typeof token!=='string'||token.length<32||token.length>512||/\s/.test(token))throw Error('A CSE_ACCESS_TOKEN of 32–512 non-whitespace characters is required.');
  const address=new URL(publicUrl??`http://127.0.0.1:${port}`);
  if(address.username||address.password||address.pathname!=='/'||address.search||address.hash)throw Error('CSE_PUBLIC_URL must be a bare origin.');
  if(address.protocol!=='https:'&&!['127.0.0.1','localhost','[::1]'].includes(address.hostname))throw Error('Remote connections require an HTTPS public URL.');
  if(!['127.0.0.1','localhost','::1'].includes(host)&&address.protocol!=='https:')throw Error('Non-loopback binding requires a configured HTTPS reverse proxy.');
  const secret=Buffer.from(token);
  const transports=new Set();
  const http=createServer(async(req,res)=>{
    const headers={'Cache-Control':'no-store','X-Content-Type-Options':'nosniff','Content-Security-Policy':"default-src 'none'; frame-ancestors 'none'"};
    const end=(code,body)=>{res.writeHead(code,{'Content-Type':'application/json',...headers});res.end(JSON.stringify(body));};
    const authority=req.headers.host;
    const local=new Set([`127.0.0.1:${http.address()?.port}`,`localhost:${http.address()?.port}`,address.host]);
    if(!local.has(authority))return end(400,{error:'Invalid host'});
    if(req.headers.origin&&req.headers.origin!==address.origin)return end(403,{error:'Invalid origin'});
    if(req.url==='/health'&&req.method==='GET')return end(200,{status:'ok',version:VERSION});
    if(req.url!=='/mcp')return end(404,{error:'Not found'});
    const auth=Buffer.from(req.headers.authorization?.startsWith('Bearer ')?req.headers.authorization.slice(7):'');
    if(auth.length!==secret.length||!timingSafeEqual(auth,secret)){
      res.setHeader('WWW-Authenticate','Bearer realm="CodeSpace"');return end(401,{error:'Authentication required'});
    }
    if(req.method!=='POST'){res.setHeader('Allow','POST');return end(405,{error:'Stateless MCP accepts POST requests'});}
    if(!req.headers['content-type']?.startsWith('application/json'))return end(415,{error:'Expected application/json'});
    if(transports.size>=32)return end(503,{error:'Server busy'});
    try{
      let size=0;const chunks=[];
      for await(const chunk of req){size+=chunk.length;if(size>131072){end(413,{error:'Request too large'});return;}chunks.push(chunk);}
      let body;try{body=JSON.parse(Buffer.concat(chunks).toString('utf8'));}catch{return end(400,{error:'Invalid JSON'});}
      const transport=new StreamableHTTPServerTransport({sessionIdGenerator:undefined,enableJsonResponse:true});
      const s=gateway.server();transports.add(transport);
      res.once('close',()=>{transports.delete(transport);transport.close().catch(()=>{});s.close().catch(()=>{});});
      for(const [name,value] of Object.entries(headers))res.setHeader(name,value);
      await s.connect(transport);await transport.handleRequest(req,res,body);
    }catch{if(!res.headersSent)end(400,{error:'Invalid MCP request'});else res.end();}
  });
  http.requestTimeout=15000;http.headersTimeout=10000;http.maxConnections=64;
  await new Promise((resolve,reject)=>http.once('error',reject).listen(port,host,resolve));
  return {http,close:async()=>{for(const t of transports)await t.close();http.closeAllConnections();await new Promise(resolve=>http.close(resolve));}};
}

export async function main(){
  const args=new Set(process.argv.slice(2));
  let config={};
  if(args.has('--config')){
    const file=process.argv[process.argv.indexOf('--config')+1];
    config=JSON.parse(await readFile(file,'utf8'));
  }
  const gateway=await createGateway({workspace:config.workspace??process.env.CSE_WORKSPACE,binary:config.binary??process.env.CSE_BINARY??'cse',
    readOnly:!(config.allowWrites===true||process.env.CSE_ALLOW_WRITES==='1'),widgetPath:new URL('./widget.html',import.meta.url)});
  let runtime;
  try {
  if(args.has('--stdio')){const server=gateway.server();await server.connect(new StdioServerTransport());runtime={close:()=>server.close()};}
  else{runtime=await startHttp(gateway,{host:process.env.CSE_HTTP_HOST??'127.0.0.1',port:Number(config.port??process.env.PORT??8787),token:config.token??process.env.CSE_ACCESS_TOKEN,publicUrl:config.publicUrl??process.env.CSE_PUBLIC_URL});
    console.error(`CodeSpace MCP ready on port ${runtime.http.address().port}. Credentials are not logged.`);}
  } catch(error) {await gateway.close();throw error;}
  const shutdown=async()=>{await runtime.close();await gateway.close();process.exit(0);};
  process.once('SIGINT',shutdown);process.once('SIGTERM',shutdown);
  process.stdin.on('end',()=>{if(args.has('--stdio'))void shutdown();});
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href)main().catch(()=>{console.error('CodeSpace MCP failed to start. Check Node 22+, cse 3.0.0, CSE_WORKSPACE and access configuration.');process.exitCode=1;});
