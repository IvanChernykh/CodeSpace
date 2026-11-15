import {AppBridge,PostMessageTransport} from '@modelcontextprotocol/ext-apps/app-bridge';
const frame=document.querySelector('iframe');
const bridge=new AppBridge(null,{name:'E2E MCP Apps Host',version:'1'},{serverTools:{},updateModelContext:{}});
bridge.oncalltool=async params=>{
 const response=await fetch('/call',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(params)});
 return response.json();
};
bridge.onupdatemodelcontext=async params=>{window.attachedContext=params;return {};};
bridge.oninitialized=async()=>{
 await bridge.sendToolInput({arguments:{}});
 await bridge.sendToolResult(await bridge.oncalltool({name:'cse_open',arguments:{}}));
};
const connecting=bridge.connect(new PostMessageTransport(frame.contentWindow,frame.contentWindow));
frame.src='/widget';
await connecting;
