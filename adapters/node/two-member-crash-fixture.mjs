// Explicit crash fixture, separate from the production/reference host path.
import {readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {TwoMemberDatabase,TwoMemberHost,readCatalog,newRecord} from './two-member-host.mjs';
import {businessKind} from './asset-store.mjs';
const flag=n=>process.argv[process.argv.indexOf(n)+1];
const binary=resolve(flag('--binary')),manifest=resolve(flag('--configs')),database=new TwoMemberDatabase(flag('--database'));
const accounts=JSON.parse(readFileSync(flag('--accounts'),'utf8')),boundary=flag('--boundary');
const record=newRecord({catalog:readCatalog(binary,manifest),scenario:'pair-a',accounts,manifest});database.create(record);
let host;
function crash(point){const checkpoint=database.room(record.id).checkpoint;writeFileSync(flag('--checkpoint'),JSON.stringify(checkpoint));
  writeFileSync(1,JSON.stringify({crash:point,roomId:record.id,kernelPid:host.kernel.child.pid,hash:record.originalCreate.config_hash,members:database.views(record.id)})+'\n');process.kill(process.pid,'SIGKILL');}
host=new TwoMemberHost({binary,manifest,database,record,ioPolicy:(pid,r)=>{
  const kind=businessKind(r);
  if(boundary==='single_admission'&&pid==='A'&&kind==='commit_admission')return {afterCommit:()=>crash(boundary)};
  if(boundary==='both_terminal_one_pending'&&pid==='B'&&kind==='commit_settlement')return {beforeCommit:()=>crash(boundary)};
  if(boundary==='committed_lost_receipt'&&pid==='A'&&kind==='commit_settlement')return {afterCommit:()=>crash(boundary)};
  return {};
}});
await host.start();
if(boundary==='single_admission')await host.until(()=>false,10000);
else{await host.joinAndReady();await host.input('A',{kind:'leave'});if(boundary==='both_terminal_one_pending'){await host.until(()=>database.views(record.id)[0].settlements===1);await host.input('B',{kind:'leave'});}await host.until(()=>false,10000);}
