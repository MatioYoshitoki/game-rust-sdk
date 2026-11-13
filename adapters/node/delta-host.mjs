// Protocol-6 ordinary consumer. Inputs are intentions; all results and amounts come from Rust.
import {readFileSync,realpathSync} from 'node:fs';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import assert from 'node:assert/strict';
import {TwoMemberDatabase,TwoMemberHost,newRecord,readCatalog,delay,requireHostOption} from './two-member-host.mjs';
import {businessKind} from './asset-store.mjs';
export {TwoMemberDatabase,TwoMemberHost,newRecord,readCatalog,delay};

export async function runMemberJourney(host,{reverse=false}={}){
  await host.joinAndReady({reverse});const order=reverse?[...host.members].reverse():host.members,c=host.record.config;
  const projectile=[c.skill,...c.additional_skills].find(s=>s.mechanism==='projectile'),self=[c.skill,...c.additional_skills].find(s=>s.mechanism==='self_status');
  await host.until(()=>host.members.every(m=>m.snapshot?.phase==='active'&&m.snapshot.self.ready));
  const activeAt=host.members[0].snapshot.active_at_ms;assert.ok(activeAt!==null&&host.members.every(m=>m.snapshot.active_at_ms===activeAt));
  for(const m of order)await host.input(m.account.participant_id,{kind:'cast_self',skill_id:self.id});
  await host.until(()=>host.members.every(m=>m.snapshot.self.pending_action===null&&m.snapshot.self.statuses.length>0));
  let casts=0;const deadline=Date.now()+22000;
  while(host.members[0].snapshot.monsters.some(n=>n.hp>0)){
    assert.ok(Date.now()<deadline,'FIGHT_DEADLINE');
    for(const m of order){const s=m.snapshot,alive=s.monsters.filter(n=>n.hp>0),target=alive.sort((a,b)=>Math.hypot(...a.position.map((v,i)=>v-s.self.position[i]))-Math.hypot(...b.position.map((v,i)=>v-s.self.position[i])))[0];
      if(!target||s.self.pending_action||s.logical_ms<Math.max(s.self.recovery_until_ms,s.self.player_skills.find(v=>v.skill_id===projectile.id).ready_at_ms))continue;
      try{await host.input(m.account.participant_id,{kind:'cast',skill_id:projectile.id,direction:target.position.map((v,i)=>v-s.self.position[i])});casts++;}catch(e){if(!['INVALID_STATE','INVALID_TARGET','NO_RESOURCE'].includes(e.code))throw e;}
    }await host.pump();await delay(35);
  }
  await host.until(()=>host.members.every(m=>m.snapshot.monsters.every(n=>!n.hp)));
  const resource=host.members[0].snapshot.resources.find(r=>r.item_id);assert.ok(resource,'SHARED_DROP_REQUIRED');
  for(const m of order)await host.input(m.account.participant_id,{kind:'navigate',destination:resource.position});
  await host.until(()=>host.members.every(m=>Math.hypot(...m.snapshot.self.position.map((v,i)=>v-resource.position[i]))<1));
  for(const m of order)await host.input(m.account.participant_id,{kind:'stop'});
  const winner=order[0].account.participant_id;await host.input(winner,{kind:'pickup',resource_id:resource.id,expected_revision:order[0].snapshot.self.inventory_revision});
  const refused=[];for(const m of order.slice(1)){await assert.rejects(host.input(m.account.participant_id,{kind:'pickup',resource_id:resource.id,expected_revision:m.snapshot.self.inventory_revision}),e=>e.code==='INVALID_TARGET');refused.push(m.account.participant_id);}
  for(const m of order)await host.input(m.account.participant_id,{kind:'navigate',destination:c.extraction_point});
  await host.until(()=>host.members.every(m=>Math.hypot(...m.snapshot.self.position.map((v,i)=>v-c.extraction_point[i]))<1)&&order[0].snapshot.extraction_open);
  for(const m of order)await host.input(m.account.participant_id,{kind:'stop'});
  await host.input(winner,{kind:'extract_start'});await host.until(()=>host.database.views(host.record.id).find(v=>v.participant_id===winner).settlements===1);
  if(order.length>1){assert.equal(host.phase,'active');const survivor=order.at(-1),before=survivor.snapshot.self.position;await host.input(survivor.account.participant_id,{kind:'move',direction:[0,1]});await host.until(()=>survivor.snapshot.self.position[1]>before[1]);await host.input(survivor.account.participant_id,{kind:'stop'});}
  for(const m of order.slice(1)){await host.input(m.account.participant_id,{kind:'leave'});await host.pump();}
  await host.until(()=>host.phase==='closed'&&host.pending()===0&&host.publicationsComplete());const accounts=host.database.views(host.record.id);assert.ok(accounts.every(a=>a.admissions===1&&a.settlements===1&&a.compensations===0));assert.equal(host.errors.length,0);
  return {clock:'ordinary',worlds:1,capacity:host.members.length,activeAt,casts,winner,resource:resource.id,refused,phase:host.phase,pending:host.pending(),accounts};
}
async function main(){
  const flag=(n,f)=>{const i=process.argv.indexOf(n);return i<0?f:process.argv[i+1]},binary=resolve(flag('--binary','target/debug/kernel-host'));const resume=process.argv.includes('--resume');
  let manifest=resume?undefined:resolve(requireHostOption(flag('--configs',process.env.MATCH_CONFIG_MANIFEST),'--configs'));
  const accounts=resume?undefined:JSON.parse(readFileSync(requireHostOption(flag('--accounts'),'--accounts'),'utf8'));
  const database=new TwoMemberDatabase(flag('--database','.local/delta-node.sqlite'));let record,recovery;
  if(resume){record=database.room(flag('--resume'));const path=flag('--frozen-checkpoint');recovery=database.recovery(record.id,path?JSON.parse(readFileSync(path,'utf8')):null);manifest=database.frozenManifest(record.id);}
  else{
    const catalog=readCatalog(binary,manifest);
    record=newRecord({catalog,scenario:flag('--scenario',catalog.defaultScenario),accounts,manifest});
    database.create(record);
  }
  const reverse=process.argv.includes('--reverse'),admissionOrder=[],host=new TwoMemberHost({binary,manifest,database,record,autoAck:true,ioPolicy:(pid,r)=>businessKind(r)==='commit_admission'?{delayMs:reverse?(record.accounts.length-1-record.accounts.findIndex(a=>a.participant_id===pid))*60:0,afterCommit:()=>admissionOrder.push(pid)}:{}});
  try{await host.start(recovery);let result;if(recovery){await host.until(()=>host.pending()===0);result={phase:host.phase,pending:host.pending(),accounts:database.views(record.id),generation:recovery.generation,replication:host.replicationStats};}else result={...await runMemberJourney(host,{reverse}),admissionOrder,replication:host.replicationStats};console.log(JSON.stringify({roomId:record.id,...result}));}
  finally{await host.dispose();database.close();}
}
if(process.argv[1]&&import.meta.url===pathToFileURL(realpathSync(process.argv[1])).href)main().catch(e=>{console.error(e.stack);process.exitCode=1;});
