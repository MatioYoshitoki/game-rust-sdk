// Reuse the same thin server; only external profile and validation UI defaults differ.
import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../../',import.meta.url));
for(const [key,value]of[['--database',root+'.local/sdk-5-27-demo.sqlite'],['--port','4216'],['--page-prefix','member-skills']])if(!process.argv.includes(key))process.argv.push(key,value);
await import('./two-member-server.mjs');
