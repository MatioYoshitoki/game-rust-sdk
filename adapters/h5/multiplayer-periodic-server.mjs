// Reuse the same thin server; only external profile and validation UI defaults differ.
import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../../',import.meta.url));
for(const [key,value]of[['--database',root+'.local/sdk-5-21-demo.sqlite'],['--port','4198'],['--page-prefix','multiplayer-periodic']])if(!process.argv.includes(key))process.argv.push(key,value);
await import('./two-member-server.mjs');
