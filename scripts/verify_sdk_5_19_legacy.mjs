// Independent ordinary JSONL consumer; no replay clock or internal World access.
import {mkdtempSync,readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {resolve,join} from 'node:path';
import {tmpdir} from 'node:os';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
import {KernelProcess,TwoMemberDatabase,TwoMemberHost,newRecord,readCatalog,runJourney,delay} from '../adapters/node/two-member-host.mjs';
import {businessKind} from '../adapters/node/asset-store.mjs';

const flag=(n,f)=>{const i=process.argv.indexOf(n);return i<0?f:process.argv[i+1]};
const binary=resolve(flag('--binary','target/debug/kernel-host')),out=resolve(flag('--out','artifacts/sdk-5-19/legacy-public'));mkdirSync(out,{recursive:true});
const manifest=resolve('examples/two-member-host/catalog.json'),catalog=readCatalog(binary,manifest),accounts=JSON.parse(readFileSync('examples/two-member-host/accounts-a.json','utf8'));
const template=newRecord({catalog,scenario:'pair-a',accounts,manifest}).originalCreate,cases=[];
const mutations=[
  ['zero-seats',r=>r.participants=[],'INVALID_MESSAGE'],['one-seat',r=>r.participants.pop(),'INVALID_MESSAGE'],['three-seats',r=>r.participants.push(r.participants[0]),'INVALID_MESSAGE'],
  ['duplicate-id',r=>r.participants[1].participant_id=r.participants[0].participant_id,'CONFLICT'],['duplicate-principal',r=>r.participants[1].authenticated_principal=r.participants[0].authenticated_principal,'CONFLICT'],['duplicate-connection',r=>r.participants[1].connection=r.participants[0].connection,'CONFLICT'],
  ['npc-namespace',r=>r.participants[0].participant_id='status-target-1','INVALID_MESSAGE'],['zero-generation',r=>r.participants[0].connection.generation=0,'INVALID_MESSAGE'],
  ['mixed-fields',r=>r.participant_id='legacy','INVALID_MESSAGE'],['unknown-field',r=>r.world_patch={hp:999},'INVALID_MESSAGE'],['wrong-version',r=>r.protocol_version=3,'VERSION_MISMATCH'],['wrong-hash',r=>r.config_hash='f'.repeat(64),'CONFIG_MISMATCH'],
];
for(const[name,change,error]of mutations){const kernel=new KernelProcess(binary,manifest);try{const request=structuredClone(template);change(request);await assert.rejects(kernel.rpc({type:'create',request}),e=>e.code===error);const before=await kernel.rpc({type:'drain_outputs'});assert.deepEqual(before.outputs,[]);await kernel.rpc({type:'create',request:template});const after=await kernel.rpc({type:'drain_outputs'});assert.equal(after.outputs.filter(o=>o.body.kind==='business_request_v4').length,2);cases.push({name,result:error,noPartialCreate:true});}finally{await kernel.dispose();}}
for(const[scenario,file,reverse]of[['pair-a','accounts-a.json',false],['pair-b','accounts-b.json',true]]){
  const database=new TwoMemberDatabase(join(out,`${scenario}.sqlite`));const account=JSON.parse(readFileSync(`examples/two-member-host/${file}`,'utf8'));
  const record=newRecord({catalog,scenario,accounts:account,manifest});database.create(record);const admissionOrder=[];const host=new TwoMemberHost({binary,manifest,database,record,ioPolicy:(pid,r)=>businessKind(r)==='commit_admission'?{delayMs:reverse&&pid===record.accounts[0].participant_id?100:0,afterCommit:()=>admissionOrder.push(pid)}:{}});
  try{await host.start();const journey=await runJourney(host,{reverse});assert.equal(admissionOrder[0],record.accounts[reverse?1:0].participant_id);cases.push({name:scenario,...journey,admissionOrder});}finally{await host.dispose();database.close();}
}
// A slow/unknown terminal write is deliberately outside the ordered gameplay queue.
{
  const database=new TwoMemberDatabase(join(out,'slow-io.sqlite')),record=newRecord({catalog,scenario:'pair-a',accounts,manifest});database.create(record);
  let held=false;const host=new TwoMemberHost({binary,manifest,database,record,ioPolicy:(pid,r)=>pid==='A'&&businessKind(r)==='commit_settlement'?(held=true,{delayMs:900,dropReceipt:true}):{}});
  try{await host.start();await host.joinAndReady();await host.input('A',{kind:'leave'});await host.until(()=>held);const before=host.member('B').snapshot.self.position;
    await host.input('B',{kind:'move',direction:[0,1]});await host.until(()=>host.member('B').snapshot.self.position[1]>before[1]);assert.equal(database.views(record.id)[0].settlements,0);await host.input('B',{kind:'stop'});
    await host.until(()=>database.views(record.id)[0].settlements===1&&host.pending()===0);assert.equal(host.phase,'active');await host.input('B',{kind:'leave'});await host.until(()=>host.phase==='closed'&&host.pending()===0);
    assert.ok(database.views(record.id).every(v=>v.admissions===1&&v.settlements===1));cases.push({name:'slow-and-lost-core-receipt',peerAdvanced:true,oneCommitPerMember:true,pending:0});
  }finally{await host.dispose();database.close();}
}
{
  const database=new TwoMemberDatabase(join(out,'refusal.sqlite')),record=newRecord({catalog,scenario:'pair-a',accounts,manifest});database.create(record);
  const host=new TwoMemberHost({binary,manifest,database,record,ioPolicy:(pid,r)=>pid==='B'&&businessKind(r)==='commit_admission'?{refuse:true}:{}});
  try{await host.start();await host.until(()=>host.phase==='closed'&&host.pending()===0);const views=database.views(record.id);assert.equal(views[0].admissions,1);assert.equal(views[0].compensations,1);assert.equal(views[1].admissions,0);assert.equal(views[1].compensations,0);cases.push({name:'required-seat-refusal',activation:false,compensations:1,pending:0});}finally{await host.dispose();database.close();}
}
const report={passed:true,crate:'0.21.0',clock:'ordinary',binarySha256:createHash('sha256').update(readFileSync(binary)).digest('hex'),cases};writeFileSync(join(out,'report.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({passed:true,cases:cases.length,report:join(out,'report.json')}));
