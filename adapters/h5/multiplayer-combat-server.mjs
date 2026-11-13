// Reuse the same thin server; only external profile and validation UI defaults differ.
import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../../',import.meta.url));
for(const [key,value]of[['--database',root+'.local/sdk-5-23-demo.sqlite'],['--port','4204'],['--page-prefix','multiplayer-combat']])if(!process.argv.includes(key))process.argv.push(key,value);
await import('./two-member-server.mjs');
