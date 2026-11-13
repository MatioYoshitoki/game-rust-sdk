// Explicit destructive-process acceptance fixture; the ordinary host never kills itself.
import {readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {TwoMemberDatabase,TwoMemberHost,readCatalog,newRecord} from './two-member-host.mjs';
import {businessKind} from './asset-store.mjs';
const flag=n=>process.argv[process.argv.indexOf(n)+1],binary=resolve(flag('--binary')),manifest=resolve(flag('--configs')),database=new TwoMemberDatabase(flag('--database'));
const accounts=JSON.parse(readFileSync(flag('--accounts'),'utf8')),boundary=flag('--boundary'),record=newRecord({catalog:readCatalog(binary,manifest),scenario:'eight',accounts,manifest});database.create(record);let host;
function crash(){writeFileSync(flag('--checkpoint'),JSON.stringify(database.room(record.id).checkpoint));writeFileSync(1,JSON.stringify({boundary,roomId:record.id,kernelPid:host.kernel.child.pid,hash:record.originalCreate.config_hash,accounts:database.views(record.id)})+'\n');process.kill(process.pid,'SIGKILL');}
host=new TwoMemberHost({binary,manifest,database,record,ioPolicy:(pid,r)=>{
 const kind=businessKind(r);
 if(boundary==='partial_admissions'&&pid==='P02'&&kind==='commit_admission')return {afterCommit:crash};
 if(boundary==='last_terminal_pending'&&pid==='P07'&&kind==='commit_settlement')return {beforeCommit:crash};
 if(boundary==='committed_lost_receipt'&&pid==='P03'&&kind==='commit_settlement')return {afterCommit:crash};return {};
}});
await host.start();if(boundary==='partial_admissions')await host.until(()=>false,10000);else {await host.joinAndReady();if(boundary==='last_terminal_pending'){for(const m of host.members){const pid=m.account.participant_id;await host.input(pid,{kind:'leave'});await host.until(()=>database.views(record.id).find(a=>a.participant_id===pid).settlements===1);}}else await host.input('P03',{kind:'leave'});await host.until(()=>false,10000);}
