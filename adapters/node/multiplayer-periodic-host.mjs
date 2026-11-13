// Ordinary consumer: input intentions and actual Rust publication/periodic facts only.
import {readFileSync,realpathSync} from 'node:fs';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import assert from 'node:assert/strict';
import {TwoMemberDatabase,TwoMemberHost,newRecord,readCatalog,delay,requireHostOption} from './two-member-host.mjs';
export {TwoMemberDatabase,TwoMemberHost,newRecord,readCatalog};
import {probeSkillSlots} from './skill-slot-probe.mjs';
const memberActive=m=>m.snapshot?.self.phase==='active'&&m.snapshot.self.ready;
export async function runPeriodicJourney(host,{reverse=false,shieldValidation=false,unitValidation=false,areaValidation=false,flightValidation=false,slotsValidation=false}={}) {
 await host.joinAndReady({reverse});await host.until(()=>host.members.every(memberActive));
 const order=reverse?[...host.members].reverse():host.members,c=host.record.config,projectile=[c.skill,...c.additional_skills].find(s=>['projectile','unit_projectile','ground_circle','ground_field','line_field','piercing_projectile'].includes(s.mechanism)),self=[c.skill,...c.additional_skills].find(s=>s.mechanism==='self_status');
 const activeAt=host.members[0].snapshot.active_at_ms;
 for(const m of order)await host.input(m.account.participant_id,{kind:'cast_self',skill_id:self.id});
 await host.until(()=>host.members.every(m=>m.snapshot.self.pending_action===null&&m.snapshot.self.statuses.some(s=>s.periodic?.kind==='max_hp_heal')));
 async function fire(m){const s=m.snapshot,target=s.monsters.filter(n=>n.hp>0).sort((a,b)=>Math.hypot(...a.position.map((v,i)=>v-s.self.position[i]))-Math.hypot(...b.position.map((v,i)=>v-s.self.position[i])))[0];if(!target||!memberActive(m)||s.self.pending_action||s.logical_ms<Math.max(s.self.recovery_until_ms,s.self.player_skills.find(s=>s.skill_id===projectile.id).ready_at_ms))return false;try{await host.input(m.account.participant_id,['ground_circle','ground_field'].includes(projectile.mechanism)?{kind:'cast_ground',skill_id:projectile.id,position:target.position}:projectile.mechanism==='unit_projectile'?{kind:'cast_unit',skill_id:projectile.id,target:{id:target.entity.id,generation:target.entity.generation}}:{kind:'cast',skill_id:projectile.id,direction:target.position.map((v,i)=>v-s.self.position[i])});return true;}catch(e){if(!['INVALID_STATE','INVALID_TARGET','NO_RESOURCE','OUT_OF_RANGE','CAPACITY'].includes(e.code))throw e;return false;}}
 await host.until(()=>host.members.every(m=>m.snapshot.logical_ms>=m.snapshot.self.recovery_until_ms));
 if(slotsValidation)await probeSkillSlots(host,{skip:[projectile.id,self.id]});
 for(const m of order)assert.ok(await fire(m));
 await host.until(()=>host.member('P00').snapshot.monsters.some(n=>n.statuses?.some(s=>s.source.id==='P00'))&&host.members.some(m=>m.periodic.incomingDotTicks>0)&&host.members.some(m=>m.periodic.selfHotTicks>0)&&host.members.some(m=>m.periodic.npcHotTicks>0));
 if(areaValidation&&projectile.mechanism==='ground_circle'){await host.until(()=>host.member('P00').snapshot.logical_ms>=host.member('P00').snapshot.self.player_skills.find(s=>s.skill_id===projectile.id).ready_at_ms);assert.ok(await fire(host.member('P00')));await host.until(()=>host.member('P00').snapshot.areas.some(a=>a.source.id==='P00'));}
 if(flightValidation&&!host.member('P00').snapshot.projectiles.some(p=>p.source.id==='P00'&&p.piercing)){await host.until(()=>host.member('P00').snapshot.logical_ms>=host.member('P00').snapshot.self.player_skills.find(s=>s.skill_id===projectile.id).ready_at_ms);await host.input('P00',{kind:'cast',skill_id:projectile.id,direction:[1,0]});await host.until(()=>host.member('P00').snapshot.projectiles.some(p=>p.source.id==='P00'&&p.piercing));}
 const retired=host.member('P00');await host.input('P00',{kind:'leave'});await host.pump();const retiredTicks=retired.periodic.outgoingDotTicks;
 const survivors=order.filter(m=>m.account.participant_id!=='P00'),watch=survivors[0],reconnecting=host.member('P01');
 let flightReconnect=null,oldFlight=null;
 if(flightValidation){oldFlight=reconnecting.snapshot.projectiles.find(p=>p.source.id==='P01'&&p.piercing);if(!oldFlight){await host.until(()=>reconnecting.snapshot.logical_ms>=Math.max(reconnecting.snapshot.self.recovery_until_ms,reconnecting.snapshot.self.player_skills.find(s=>s.skill_id===projectile.id).ready_at_ms));await host.input('P01',{kind:'cast',skill_id:projectile.id,direction:[1,0]});await host.until(()=>reconnecting.snapshot.projectiles.some(p=>p.source.id==='P01'&&p.piercing));oldFlight=reconnecting.snapshot.projectiles.find(p=>p.source.id==='P01'&&p.piercing);}}
 let areaReconnect=null,oldArea=null;
 if(areaValidation){oldArea=reconnecting.snapshot.areas.find(a=>a.source.id==='P01');if(!oldArea){await host.until(()=>reconnecting.snapshot.logical_ms>=reconnecting.snapshot.self.player_skills.find(s=>s.skill_id===projectile.id).ready_at_ms);await host.input('P01',projectile.mechanism==='line_field'?{kind:'cast',skill_id:projectile.id,direction:[1,0]}:{kind:'cast_ground',skill_id:projectile.id,position:reconnecting.snapshot.self.position});await host.until(()=>reconnecting.snapshot.areas.some(a=>a.source.id==='P01'));oldArea=reconnecting.snapshot.areas.find(a=>a.source.id==='P01');}}
 const priorStatus=predicate=>[...host.events].reverse().find(row=>row.recipient_id==='P01'&&row.event.kind==='private'&&['status_applied','status_refreshed'].includes(row.event.event.kind)&&predicate(row.event.event.status))?.event.event.status;
 const oldShield=shieldValidation?reconnecting.snapshot.self.statuses.find(s=>s.shield):null;if(shieldValidation)assert.ok(oldShield);
 const oldStatus=reconnecting.snapshot.self.statuses.find(s=>s.periodic?.kind==='max_hp_heal')||((areaValidation||flightValidation)?reconnecting.periodic.selfHotIdentity||priorStatus(s=>s.source.id==='P01'&&s.target.id==='P01'&&s.periodic?.kind==='max_hp_heal'):null);assert.ok(oldStatus);
 await host.reconnect('P01');await host.pump();await host.input('P01',{kind:'foreground'});await host.until(()=>reconnecting.snapshot.self.foreground&&!reconnecting.snapshot.self.ready);await host.input('P01',{kind:'combat_ready',publication_id:reconnecting.readyPublication});await host.until(()=>memberActive(reconnecting));
 const restored=reconnecting.snapshot.self.statuses.find(s=>s.instance_id===oldStatus.instance_id);if(restored)assert.ok(restored.applied_at_ms===oldStatus.applied_at_ms&&restored.expires_at_ms===oldStatus.expires_at_ms);else assert.ok(reconnecting.snapshot.logical_ms>=oldStatus.expires_at_ms);
 if(oldArea){const kept=reconnecting.snapshot.areas.find(a=>a.id===oldArea.id),ended=host.members.flatMap(m=>Object.entries(m.areaStats.endedReasons)).find(([id])=>id===oldArea.id)?.[1];if(kept){assert.deepEqual(kept.center,oldArea.center);assert.deepEqual(kept.line,oldArea.line);assert.equal(kept.created_at_ms,oldArea.created_at_ms);assert.equal(kept.expires_at_ms,oldArea.expires_at_ms);assert.ok(kept.completed_ticks>=oldArea.completed_ticks);if(kept.contact_policy)assert.ok(kept.contact_policy.used_hits>=oldArea.contact_policy.used_hits);}else assert.ok(['resolved','expired','hit_budget_exhausted'].includes(ended));areaReconnect={id:oldArea.id,expiresAt:oldArea.expires_at_ms,preserved:!!kept,endedReason:ended??null};}
 if(oldFlight){const kept=reconnecting.snapshot.projectiles.find(p=>p.id===oldFlight.id),ended=host.members.flatMap(m=>Object.entries(m.flightStats.endedReasons)).find(([id])=>id===oldFlight.id)?.[1];if(kept){const a=oldFlight.piercing,b=kept.piercing;for(const k of ['skill_id','direction','speed','created_at_ms','expires_at_ms','max_hits','repeat_interval_ms'])assert.deepEqual(b[k],a[k]);assert.ok(b.remaining_range<=a.remaining_range&&b.used_hits>=a.used_hits);for(const c of a.contacts){const next=b.contacts.find(n=>n.target.id===c.target.id&&n.target.generation===c.target.generation);assert.ok(next&&next.hit_at_ms>=c.hit_at_ms);}}else assert.ok(['wall','range_exhausted','hit_budget_exhausted','expired'].includes(ended));flightReconnect={id:oldFlight.id,expiresAt:oldFlight.piercing.expires_at_ms,preserved:!!kept,endedReason:ended??null};}
 const shieldAfter=oldShield?reconnecting.snapshot.self.statuses.find(s=>s.instance_id===oldShield.instance_id):null;
 if(oldShield){if(shieldAfter){assert.equal(shieldAfter.applied_at_ms,oldShield.applied_at_ms);assert.equal(shieldAfter.expires_at_ms,oldShield.expires_at_ms);assert.ok(shieldAfter.shield.remaining<=oldShield.shield.remaining);}else assert.ok(reconnecting.snapshot.logical_ms>=oldShield.expires_at_ms||host.events.some(r=>r.recipient_id==='P01'&&r.event.kind==='private'&&r.event.event.kind==='status_removed'&&r.event.event.instance_ids.includes(oldShield.instance_id)&&['depleted','source_invalid','expired'].includes(r.event.event.reason)));}
 const shieldReconnect=oldShield?{instanceId:oldShield.instance_id,expiresAt:oldShield.expires_at_ms,before:oldShield.shield.remaining,after:shieldAfter?.shield.remaining??0,expiredNormally:!shieldAfter}:null;
 const reconnectCadence={instanceId:oldStatus.instance_id,expiresAt:oldStatus.expires_at_ms,afterLogical:reconnecting.snapshot.logical_ms,expiredNormally:!restored};
 const cache=host.member('P02'),resyncs=host.replicationStats.resyncs;cache.replica.forgetBaseline();await host.until(()=>host.replicationStats.resyncs>resyncs&&cache.replica.snapshot!==null);
 let casts=0;const end=Date.now()+22000;
 while(watch.snapshot.monsters.some(n=>n.hp>0)){assert.ok(Date.now()<end,'FIGHT_DEADLINE');for(const m of survivors)casts+=await fire(m);await host.pump();await delay(35);}
 await host.until(()=>survivors.every(m=>m.snapshot.monsters.every(n=>!n.hp)));
 assert.equal(retired.periodic.outgoingDotTicks,retiredTicks,'EXITED_SOURCE_TICKED');
 const resource=watch.snapshot.resources.find(r=>r.item_id);assert.ok(resource);
 for(const m of survivors)await host.input(m.account.participant_id,{kind:'navigate',destination:resource.position});
 await host.until(()=>survivors.every(m=>Math.hypot(...m.snapshot.self.position.map((v,i)=>v-resource.position[i]))<1));
 for(const m of survivors)await host.input(m.account.participant_id,{kind:'stop'});
 const winner=watch.account.participant_id;await host.input(winner,{kind:'pickup',resource_id:resource.id,expected_revision:watch.snapshot.self.inventory_revision});
 const refused=[];for(const m of survivors.slice(1)){await assert.rejects(host.input(m.account.participant_id,{kind:'pickup',resource_id:resource.id,expected_revision:m.snapshot.self.inventory_revision}),e=>e.code==='INVALID_TARGET');refused.push(m.account.participant_id);}
 for(const m of survivors)await host.input(m.account.participant_id,{kind:'navigate',destination:c.extraction_point});
 await host.until(()=>survivors.every(m=>Math.hypot(...m.snapshot.self.position.map((v,i)=>v-c.extraction_point[i]))<1)&&watch.snapshot.extraction_open);
 for(const m of survivors)await host.input(m.account.participant_id,{kind:'stop'});
 await host.input(winner,{kind:'extract_start'});await host.until(()=>host.database.views(host.record.id).find(a=>a.participant_id===winner).settlements===1);
 for(const m of survivors.slice(1))await host.input(m.account.participant_id,{kind:'leave'});
 await host.until(()=>host.phase==='closed'&&host.pending()===0&&host.publicationsComplete());
 const accounts=host.database.views(host.record.id);assert.ok(accounts.every(a=>a.admissions===a.settlements&&a.settlements===1&&a.compensations===0));assert.equal(host.errors.length,0);
 const metrics=host.members.map(m=>({participant_id:m.account.participant_id,...m.periodic}));
 assert.ok(metrics.some(m=>m.outgoingDotTicks>0)&&metrics.some(m=>m.incomingDotTicks>0)&&metrics.some(m=>m.selfHotTicks>0)&&metrics.some(m=>m.npcHotTicks>0)&&metrics.some(m=>m.hpRestored>0));
 const shields=host.members.map(m=>({participant_id:m.account.participant_id,...m.shields}));
 if(shieldValidation){assert.ok(shields.every(m=>m.ownGrants>0));assert.ok(shields.some(m=>m.ownAbsorptions>0)&&shields.some(m=>m.npcAbsorptions>0)&&shields.some(m=>m.dotAbsorbed>0));}
 const combat=host.members.map(m=>({participant_id:m.account.participant_id,...m.combat}));if(unitValidation)assert.ok(combat.every(m=>m.unitCommits>0&&m.unitProjectiles>0&&m.targets.length>0));
 const areaStats=host.members.map(m=>({participant_id:m.account.participant_id,...m.areaStats}));if(areaValidation)assert.ok(areaStats.every(m=>m.ownStarted>0&&m.ownTicks>0&&m.ownContacts>0));
 const flightStats=host.members.map(m=>({participant_id:m.account.participant_id,...m.flightStats}));if(flightValidation)assert.ok(flightStats.every(m=>m.ownStarted>0&&m.ownContacts>0));
 const skillStats=host.members.map(m=>({participant_id:m.account.participant_id,...m.skillStats}));if(slotsValidation)assert.ok(skillStats.every(m=>[c.skill,...c.additional_skills].every(s=>m.commits[s.id]>0)));
 return {skillStats,slotsValidated:slotsValidation,flightStats,flightReconnect,flightValidated:flightValidation,areaStats,areaReconnect,areaValidated:areaValidation,combat,unitValidated:unitValidation,shields,shieldReconnect,clock:'ordinary',worlds:1,capacity:host.members.length,activeAt,casts,winner,resource:resource.id,refused,earlyExit:'P00',retiredSourceStopped:true,reconnectPreserved:true,reconnectCadence,cacheResynced:true,phase:host.phase,pending:host.pending(),accounts,periodic:metrics,replication:host.replicationStats};
}
async function main(){const flag=(n,f)=>{const i=process.argv.indexOf(n);return i<0?f:process.argv[i+1]},binary=resolve(flag('--binary','target/debug/kernel-host'));const resume=process.argv.includes('--resume');
  let manifest=resume?undefined:resolve(requireHostOption(flag('--configs',process.env.MATCH_CONFIG_MANIFEST),'--configs'));
  const accounts=resume?undefined:JSON.parse(readFileSync(requireHostOption(flag('--accounts'),'--accounts'),'utf8'));const database=new TwoMemberDatabase(flag('--database','.local/periodic-members-node.sqlite'));let record,recovery;
 if(resume){record=database.room(flag('--resume'));const path=flag('--frozen-checkpoint');recovery=database.recovery(record.id,path?JSON.parse(readFileSync(path,'utf8')):null);manifest=database.frozenManifest(record.id);}
 else{
    const catalog=readCatalog(binary,manifest);
    record=newRecord({catalog,scenario:flag('--scenario',catalog.defaultScenario),accounts,manifest});
    database.create(record);
  }
 const host=new TwoMemberHost({binary,manifest,database,record,autoAck:true});try{await host.start(recovery);let result;if(recovery){await host.until(()=>host.pending()===0);result={phase:host.phase,pending:host.pending(),accounts:database.views(record.id),generation:recovery.generation,replication:host.replicationStats,periodic:host.members.map(m=>m.periodic)};}else result=await runPeriodicJourney(host,{reverse:process.argv.includes('--reverse')});console.log(JSON.stringify({roomId:record.id,...result}));}finally{await host.dispose();database.close();}}
if(process.argv[1]&&import.meta.url===pathToFileURL(realpathSync(process.argv[1])).href)main().catch(e=>{console.error(e.stack);process.exitCode=1;});
