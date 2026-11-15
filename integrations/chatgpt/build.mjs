import {build} from 'esbuild';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
await mkdir('dist',{recursive:true});
await build({entryPoints:['src/widget.mjs'],bundle:true,format:'esm',platform:'browser',outfile:'dist/widget.js',minify:true});
const template=await readFile('src/widget.html','utf8');
const script=(await readFile('dist/widget.js','utf8')).replace(/<\/script/gi,'<\\/script');
await writeFile('dist/widget.html',template.replace('<!-- WIDGET_SCRIPT -->',()=>`<script type="module">${script}</script>`));
await build({entryPoints:['src/server.mjs'],bundle:true,format:'esm',platform:'node',packages:'bundle',outfile:'dist/server.mjs',banner:{js:'import { createRequire as __createRequire } from "node:module"; const require = __createRequire(import.meta.url);'}});
