import assert from 'node:assert/strict';
import {randomUUID} from 'node:crypto';
import {readFileSync,writeFileSync} from 'node:fs';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';

export async function verifyAllyCommit({binary,hostPath,configs,output,scenario}) {
 const {TwoMemberDatabase,TwoMemberHost,newRecord,readCatalog}=await import(pathToFileURL(hostPath).href);
 const manifest=join(configs,'catalog.json'),catalog=readCatalog(binary,manifest);
 const accounts=JSON.parse(readFileSync(join(configs,scenario==='four'?'accounts-a.json':'accounts-b.json'),'utf8'));
 const record=newRecord({catalog,scenario,accounts,manifest});
 const database=new TwoMemberDatabase(join(output,`ally-${scenario}-${randomUUID()}.sqlite`));database.create(record);
 const facts=[];const host=new TwoMemberHost({binary,manifest,database,record,autoAck:true,onEvent(recipient,event){facts.push({recipient,event:event.kind==='private'?event.event:event});}});
 const actor=host.members[2],holder=host.members[3],source=actor.account.participant_id,target=holder.account.participant_id;
 const own=()=>actor.snapshot.self,other=()=>holder.snapshot.self;
 try {
  await host.start();await host.joinAndReady();
  await host.input(source,{kind:'navigate',destination:[890,800]});await host.until(()=>Math.hypot(own().position[0]-890,own().position[1]-800)<5);await host.input(source,{kind:'stop'});
  await host.input(target,{kind:'navigate',destination:[930,800]});await host.until(()=>other().statuses.some(s=>s.definition_id==='compound-lock'));
  const original=other().statuses.find(s=>s.definition_id==='compound-lock');assert.equal(original.source.id,'control-source');assert.deepEqual(original.flags,scenario==='four'?['silence','root']:['stun']);
  const cost=record.config.additional_skills.find(s=>s.id==='ally-renew').cost,spirit=own().spirit;
  await host.input(source,{kind:'cast_unit',skill_id:'ally-renew',target:{id:target,generation:other().participant.entity.generation}});
  await host.until(()=>own().statuses.some(s=>s.instance_id===original.instance_id));
  const moved=own().statuses.find(s=>s.instance_id===original.instance_id);assert.deepEqual(moved,{...original,target:own().participant.entity});assert.equal(own().spirit,spirit-cost);
  assert.ok(!other().statuses.some(s=>s.instance_id===original.instance_id));
  const transferred=facts.filter(r=>r.event.kind==='status_transferred'&&r.event.status.instance_id===original.instance_id);assert.equal(transferred.length,2);assert.deepEqual(new Set(transferred.map(r=>r.recipient)),new Set([source,target]));assert.deepEqual(transferred[0].event,transferred[1].event);
  const commit=facts.findIndex(r=>r.recipient===source&&r.event.kind==='action_committed'&&r.event.skill_id==='ally-renew');const primary=facts.findIndex(r=>r.recipient===source&&r.event.kind==='status_applied'&&r.event.status.definition_id==='ally-renewal');const transfer=facts.indexOf(transferred.find(r=>r.recipient===source));assert.ok(commit>=0&&commit<primary&&primary<transfer);
  assert.ok(facts.filter(r=>r.event.kind==='combat_trigger_fired'&&r.event.trigger_id==='ally-carry-control').every(r=>r.recipient===source));
  for(const m of host.members.filter(m=>![source,target].includes(m.account.participant_id)))assert.ok(!m.snapshot.self.statuses.some(s=>s.instance_id===original.instance_id));
  const paid=own().spirit;await assert.rejects(host.input(source,{kind:'cast_self',skill_id:'focus-power'}));assert.equal(own().spirit,paid);
  await host.input(target,{kind:'leave'});await host.until(()=>own().statuses.some(s=>s.instance_id===original.instance_id));
  await host.reconnect(source);await host.until(()=>!own().ready);await host.input(source,{kind:'foreground'});await host.input(source,{kind:'combat_ready',publication_id:actor.readyPublication??actor.publication});await host.until(()=>own().ready);assert.deepEqual(own().statuses.find(s=>s.instance_id===original.instance_id),moved);
  await host.until(()=>!own().statuses.some(s=>s.instance_id===original.instance_id));
  const expired=facts.filter(r=>r.event.kind==='status_removed'&&r.event.reason==='expired'&&r.event.instance_ids.includes(original.instance_id));assert.equal(expired.length,1);assert.equal(expired[0].recipient,source);
  await host.input(source,{kind:'cast_self',skill_id:'focus-power'});await host.until(()=>own().statuses.some(s=>s.definition_id==='power-focus'));
  writeFileSync(join(output,scenario+'.ally-commit-facts.json'),JSON.stringify({before:original,after:moved,events:transferred},null,2)+'\n');
  for(const m of host.members.filter(m=>m!==holder))await host.input(m.account.participant_id,{kind:'leave'});
  await host.until(()=>host.phase==='closed'&&host.pending()===0&&host.publicationsComplete());
  const ledger=database.views(record.id);assert.ok(ledger.every(r=>r.admissions===1&&r.settlements===1));assert.equal(host.errors.length,0);
  return {scenario,members:host.members.length,source,target,originalSource:'control-source',flags:original.flags,preserved:true,privateRecipients:2,blockedWithoutCost:true,oldHolderLeft:true,reconnected:true,expired:true,resumed:true,settled:ledger.length};
 } finally {await host.dispose();database.close();}
}

export async function verifyLifecycleTransfer({binary,hostPath,configs,output,scenario}) {
 const {TwoMemberDatabase,TwoMemberHost,newRecord,readCatalog}=await import(pathToFileURL(hostPath).href);
 const manifest=join(configs,'catalog.json'),catalog=readCatalog(binary,manifest),accounts=JSON.parse(readFileSync(join(configs,scenario==='four'?'accounts-a.json':'accounts-b.json'),'utf8'));
 const record=newRecord({catalog,scenario,accounts,manifest}),database=new TwoMemberDatabase(join(output,`life-${scenario}-${randomUUID()}.sqlite`));database.create(record);
 const facts=[];const host=new TwoMemberHost({binary,manifest,database,record,autoAck:true,onEvent(recipient,event){facts.push({recipient,event:event.kind==='private'?event.event:event});}});
 const origin=host.members[0],old=host.members[1],taker=host.members[3],source=origin.account.participant_id,previous=old.account.participant_id,target=taker.account.participant_id;
 const own=()=>taker.snapshot.self,held=()=>old.snapshot.self;
 async function cast(pid,id,other){const m=host.members.find(m=>m.account.participant_id===pid);await host.until(()=>m.snapshot.logical_ms>=m.snapshot.self.player_skills.find(s=>s.skill_id===id).ready_at_ms&&m.snapshot.self.pending_action===null&&m.snapshot.logical_ms>=m.snapshot.self.recovery_until_ms);const start=facts.length;await host.input(pid,other?{kind:'cast_unit',skill_id:id,target:{id:other,generation:1}}:{kind:'cast_self',skill_id:id});await host.until(()=>facts.slice(start).some(r=>r.recipient===pid&&r.event.kind==='action_committed'&&r.event.skill_id===id));}
 try {
  await host.start();await host.joinAndReady();await cast(source,'ally-renew',previous);await host.until(()=>held().statuses.some(s=>s.definition_id==='ally-renewal'));
  const id=held().statuses.find(s=>s.definition_id==='ally-renewal').instance_id;
  await host.until(()=>held().statuses.find(s=>s.instance_id===id)?.periodic.ticks_done===1);
  const before=held().statuses.find(s=>s.instance_id===id),oldShield=held().statuses.find(s=>s.definition_id==='overflow-shield'&&s.source.id===source);assert.ok(oldShield);
  await cast(target,'training-tempo');const tempo=own().statuses.find(s=>s.definition_id==='training-tempo');assert.ok(tempo.applied_at_ms>=before.applied_at_ms);
  const paid=own().spirit,cost=record.config.additional_skills.find(s=>s.id==='ally-renew').cost;
  await cast(target,'ally-renew',previous);await host.until(()=>own().statuses.some(s=>s.instance_id===id));
  const moved=own().statuses.find(s=>s.instance_id===id);assert.deepEqual(moved,{...before,target:own().participant.entity});assert.equal(own().spirit,paid-cost);assert.ok(held().statuses.some(s=>s.instance_id===oldShield.instance_id));assert.ok(!own().statuses.some(s=>s.instance_id===oldShield.instance_id));
  const transferred=facts.filter(r=>r.event.kind==='status_transferred'&&r.event.status.instance_id===id);assert.equal(transferred.length,3);assert.deepEqual(new Set(transferred.map(r=>r.recipient)),new Set([source,previous,target]));
  await host.input(previous,{kind:'leave'});await host.reconnect(target);await host.until(()=>!own().ready);await host.input(target,{kind:'foreground'});await host.input(target,{kind:'combat_ready',publication_id:taker.readyPublication??taker.publication});await host.until(()=>own().ready);
  const resumed=own().statuses.find(s=>s.instance_id===id);assert.deepEqual([resumed.source,resumed.applied_at_ms,resumed.expires_at_ms,resumed.window,resumed.cleanse],[moved.source,moved.applied_at_ms,moved.expires_at_ms,moved.window,moved.cleanse]);assert.ok(resumed.periodic.ticks_done>=1);
  await host.until(()=>own().statuses.find(s=>s.instance_id===id)?.periodic.ticks_done===3);
  assert.ok(!own().statuses.some(s=>s.instance_id===tempo.instance_id));
  await cast(target,'training-tempo');const late=own().statuses.find(s=>s.definition_id==='training-tempo');
  await host.until(()=>facts.some(r=>r.recipient===target&&r.event.kind==='window_record_ended'&&r.event.instance_id===id));
  const cleans=facts.filter(r=>r.recipient===target&&r.event.kind==='status_cleanse_resolved'&&r.event.instance_id===id);assert.deepEqual(cleans.map(r=>r.event.removed_count),[1,0,1]);assert.deepEqual(cleans.map(r=>r.event.phase),['tick','tick','expired']);assert.deepEqual(cleans.slice(0,2).map(r=>r.event.scheduled_at_ms-before.applied_at_ms),[2000,3000]);
  const ticks=facts.filter(r=>r.recipient===target&&r.event.kind==='periodic_heal_tick'&&r.event.instance_id===id);assert.deepEqual(ticks.map(r=>r.event.tick_index),[2,3]);assert.ok(ticks.every(r=>r.event.source.id===source&&r.event.target.id===target&&r.event.scheduled_at_ms===before.applied_at_ms+r.event.tick_index*1000));
  const ends=facts.filter(r=>r.event.kind==='window_record_ended'&&r.event.instance_id===id);assert.equal(ends.length,2);assert.deepEqual(new Set(ends.map(r=>r.recipient)),new Set([source,target]));assert.deepEqual(ends[0].event,ends[1].event);const end=ends[0].event;assert.equal(end.spirit_restored,2);assert.ok(facts.indexOf(cleans.at(-1))<facts.indexOf(ends.find(r=>r.recipient===target)));assert.ok(!own().statuses.some(s=>s.instance_id===late.instance_id));
  const hpMax=moved.window.maximum_sampling==='snapshot'?moved.window.max_hp_at_apply:own().max_hp;assert.equal(end.raw_hp,Math.floor(hpMax/10));
  const subsequent=facts.filter(r=>['periodic_heal_tick','periodic_spirit_tick','status_cleanse_resolved'].includes(r.event.kind)&&r.event.instance_id===id&&r.event.target.id===target);assert.ok(subsequent.every(r=>[source,target].includes(r.recipient)));
  writeFileSync(join(output,scenario+'.lifecycle-transfer-facts.json'),JSON.stringify({before,after:moved,events:[...transferred,...cleans,...ends]},null,2)+'\n');
  for(const m of host.members.filter(m=>m!==old))await host.input(m.account.participant_id,{kind:'leave'});await host.until(()=>host.phase==='closed'&&host.pending()===0&&host.publicationsComplete());const ledgers=database.views(record.id);assert.ok(ledgers.every(r=>r.admissions===1&&r.settlements===1));assert.equal(host.errors.length,0);
  return {scenario,members:host.members.length,source,previous,target,ticksBefore:1,ticksAfter:[2,3],cleanseRemoved:[1,0,1],windowSpirit:2,transferRecipients:3,subsequentRecipients:2,oldShieldIsolated:true,oldHolderLeft:true,reconnected:true,expired:true,settled:ledgers.length};
 } finally {await host.dispose();database.close();}
}

export async function verifyControlPolicy({binary,hostPath,configs,output,scenario}) {
 const {TwoMemberDatabase,TwoMemberHost,newRecord,readCatalog}=await import(pathToFileURL(hostPath).href);const manifest=join(configs,'catalog.json'),catalog=readCatalog(binary,manifest),record=newRecord({catalog,scenario,accounts:JSON.parse(readFileSync(join(configs,scenario==='four'?'accounts-a.json':'accounts-b.json'),'utf8')),manifest});const database=new TwoMemberDatabase(join(output,`policy-${scenario}-${randomUUID()}.sqlite`));database.create(record);const facts=[];const host=new TwoMemberHost({binary,manifest,database,record,autoAck:true,onEvent(recipient,event){facts.push({recipient,event:event.kind==='private'?event.event:event});}});const actor=host.members[0],source=actor.account.participant_id;const own=()=>actor.snapshot.self,npc=()=>actor.snapshot.monsters.find(n=>n.entity.id==='replacement-target');
 async function cast(target){await host.until(()=>own().pending_action===null&&actor.snapshot.logical_ms>=own().recovery_until_ms&&actor.snapshot.logical_ms>=own().player_skills.find(s=>s.skill_id==='focus-target').ready_at_ms);await host.input(source,{kind:'cast_unit',skill_id:'focus-target',target:{id:target,generation:1}});}
 try {
  await host.start();await host.joinAndReady();await host.input(source,{kind:'navigate',destination:[500,300]});await host.until(()=>Math.hypot(own().position[0]-500,own().position[1]-300)<5);await host.input(source,{kind:'stop'});await cast('capped-target');await host.until(()=>actor.snapshot.monsters.find(n=>n.entity.id==='capped-target').statuses.some(s=>s.definition_id==='compound-lock'));const capped=actor.snapshot.monsters.find(n=>n.entity.id==='capped-target').statuses.find(s=>s.definition_id==='compound-lock');assert.equal(capped.expires_at_ms-capped.applied_at_ms,1800);
  await cast('replacement-target');await host.until(()=>npc().statuses.some(s=>s.definition_id==='control-softened'));const first=npc().statuses.find(s=>s.definition_id==='control-softened');const duration=scenario==='four'?3600:4500;assert.equal(first.expires_at_ms-first.applied_at_ms,duration);assert.deepEqual(first.modifiers,[{attribute:'movement_slow',modifier_bps:-5000},{attribute:'basic_interval_delay',modifier_bps:1500}]);assert.ok(!npc().statuses.some(s=>s.definition_id==='compound-lock'));
  await cast('replacement-target');await host.until(()=>npc().statuses.some(s=>s.instance_id===first.instance_id&&s.applied_at_ms>first.applied_at_ms));const refreshed=npc().statuses.find(s=>s.instance_id===first.instance_id);assert.deepEqual(refreshed,{...first,applied_at_ms:refreshed.applied_at_ms,expires_at_ms:refreshed.expires_at_ms});assert.equal(refreshed.expires_at_ms-refreshed.applied_at_ms,duration);
  await host.reconnect(source);await host.until(()=>!own().ready);await host.input(source,{kind:'foreground'});await host.input(source,{kind:'combat_ready',publication_id:actor.readyPublication??actor.publication});await host.until(()=>own().ready);assert.deepEqual(npc().statuses.find(s=>s.instance_id===first.instance_id),refreshed);
  await host.input(source,{kind:'navigate',destination:[588,400]});await host.until(()=>Math.hypot(own().position[0]-588,own().position[1]-400)<5);await host.input(source,{kind:'stop'});
  await host.until(()=>!npc().statuses.some(s=>s.instance_id===first.instance_id));assert.ok(facts.some(r=>r.recipient===source&&r.event.kind==='status_removed'&&r.event.reason==='expired'&&r.event.instance_ids.includes(first.instance_id)));assert.ok(facts.filter(r=>r.event.status?.instance_id===first.instance_id||r.event.instance_ids?.includes(first.instance_id)).every(r=>r.recipient===source));
  const attacks=database.db.prepare('SELECT body FROM duo_outputs WHERE room_id=?').all(record.id).map(r=>JSON.parse(r.body)).filter(r=>r.logical_ms<refreshed.expires_at_ms&&r.body.kind==='event_v4'&&r.body.event.kind==='damage'&&r.body.event.source.id==='replacement-target'&&r.body.event.target.id===source).map(r=>r.logical_ms);assert.ok(attacks.length>=2,JSON.stringify(attacks));const interval=attacks[1]-attacks[0];assert.equal(interval,scenario==='four'?690:930);
  writeFileSync(join(output,scenario+'.control-policy-facts.json'),JSON.stringify({capped,first,refreshed,attackTimes:attacks},null,2)+'\n');
  for(const member of host.members)await host.input(member.account.participant_id,{kind:'leave'});await host.until(()=>host.phase==='closed'&&host.pending()===0&&host.publicationsComplete());const ledgers=database.views(record.id);assert.ok(ledgers.every(r=>r.admissions===1&&r.settlements===1));assert.equal(host.errors.length,0);
  return {scenario,members:host.members.length,cap:1800,duration,movementSlow:-5000,intervalDelay:1500,interval,refreshed:true,reconnected:true,expired:true,privateRecipients:1,settled:ledgers.length};
 }finally{await host.dispose();database.close();}
}

export async function verifyHitStatus({binary,hostPath,configs,output,scenario}) {
 const seats=scenario==='marks-four'?4:8;
 const {TwoMemberDatabase,TwoMemberHost,newRecord,readCatalog}=await import(pathToFileURL(hostPath).href);
 const manifest=join(configs,'catalog.json'),catalog=readCatalog(binary,manifest),record=newRecord({catalog,scenario,accounts:JSON.parse(readFileSync(join(configs,seats===4?'accounts-a.json':'accounts-b.json'),'utf8')),manifest});
 const database=new TwoMemberDatabase(join(output,`hit-${scenario}-${randomUUID()}.sqlite`));database.create(record);const facts=[];
 const host=new TwoMemberHost({binary,manifest,database,record,autoAck:true,onEvent(recipient,event){facts.push({recipient,event:event.kind==='private'?event.event:event});}});
 const actor=host.members[0],source=actor.account.participant_id,target=seats===4?'capped-target':'replacement-target',own=()=>actor.snapshot.self,npc=()=>actor.snapshot.monsters.find(n=>n.entity.id===target);
 async function cast(){await host.until(()=>own().pending_action===null&&actor.snapshot.logical_ms>=own().recovery_until_ms&&actor.snapshot.logical_ms>=own().player_skills.find(s=>s.skill_id==='mark-bolt').ready_at_ms);await host.input(source,{kind:'cast_unit',skill_id:'mark-bolt',target:{id:target,generation:1}});}
 try{
  await host.start();await host.joinAndReady();await host.input(source,{kind:'navigate',destination:[500,300]});await host.until(()=>Math.hypot(own().position[0]-500,own().position[1]-300)<5);await host.input(source,{kind:'stop'});
  const marks=[];
  for(const layers of [1,2]){await cast();await host.until(()=>npc().statuses.some(s=>s.definition_id==='threshold-mark'&&s.stacks===layers));marks.push(structuredClone(npc().statuses.find(s=>s.definition_id==='threshold-mark')));}
  assert.equal(marks[0].instance_id,marks[1].instance_id);assert.equal(marks[1].modifier_bps,seats===4?-800:-1000);
  await host.reconnect(source);await host.until(()=>!own().ready);await host.input(source,{kind:'foreground'});await host.input(source,{kind:'combat_ready',publication_id:actor.readyPublication??actor.publication});await host.until(()=>own().ready);assert.deepEqual(npc().statuses.find(s=>s.instance_id===marks[1].instance_id),marks[1]);
  await cast();const effect=seats===4?'compound-lock':'control-softened';await host.until(()=>npc().statuses.some(s=>s.definition_id===effect));const converted=structuredClone(npc().statuses.find(s=>s.definition_id===effect));assert.ok(!npc().statuses.some(s=>s.definition_id==='threshold-mark'));assert.equal(converted.expires_at_ms-converted.applied_at_ms,seats===4?1200:1800);assert.equal(converted.source.id,source);
  const consumed=facts.filter(r=>r.event.kind==='status_consumed'&&r.event.instance_id===marks[0].instance_id);assert.equal(consumed.length,1);assert.equal(consumed[0].event.consumed_stacks,3);
  const rows=facts.filter(r=>r.event.status?.instance_id===converted.instance_id||r.event.instance_ids?.includes(converted.instance_id));assert.ok(rows.every(r=>r.recipient===source));
  await host.until(()=>!npc().statuses.some(s=>s.instance_id===converted.instance_id));
  await cast();await host.until(()=>npc().statuses.some(s=>s.definition_id==='threshold-mark'));await host.input(source,{kind:'leave'});await host.until(()=>!npc().statuses.some(s=>s.definition_id==='threshold-mark'));
  for(const member of host.members.slice(1))await host.input(member.account.participant_id,{kind:'leave'});await host.until(()=>host.phase==='closed'&&host.pending()===0&&host.publicationsComplete());const ledgers=database.views(record.id);assert.ok(ledgers.every(r=>r.admissions===1&&r.settlements===1));assert.equal(host.errors.length,0);
  writeFileSync(join(output,scenario+'.hit-status-facts.json'),JSON.stringify({marks,converted,consumed},null,2)+'\n');
  return {scenario,members:seats,layers:[1,2,3],consumed:3,effect,duration:seats===4?1200:1800,source,privateRecipients:1,reconnected:true,expired:true,sourceLeftWithMark:true,settled:seats};
 }finally{await host.dispose();database.close();}
}

export async function verifyContactModifier({binary,hostPath,configs,output,scenario}) {
 const seats=scenario==='strikes-four'?4:8;
 const {TwoMemberDatabase,TwoMemberHost,newRecord,readCatalog}=await import(pathToFileURL(hostPath).href);
 const manifest=join(configs,'catalog.json'),catalog=readCatalog(binary,manifest),record=newRecord({catalog,scenario,accounts:JSON.parse(readFileSync(join(configs,seats===4?'accounts-a.json':'accounts-b.json'),'utf8')),manifest});
 const database=new TwoMemberDatabase(join(output,`contact-${scenario}-${randomUUID()}.sqlite`));database.create(record);const facts=[];
 const host=new TwoMemberHost({binary,manifest,database,record,autoAck:true,onEvent(recipient,event){facts.push({recipient,event:event.kind==='private'?event.event:event});}});
 const actor=host.members[0],source=actor.account.participant_id,target='capped-target',own=()=>actor.snapshot.self,npc=()=>actor.snapshot.monsters.find(n=>n.entity.id===target);
 async function cast(skill){await host.until(()=>own().pending_action===null&&actor.snapshot.logical_ms>=own().recovery_until_ms&&actor.snapshot.logical_ms>=own().player_skills.find(s=>s.skill_id===skill).ready_at_ms);const start=facts.length;await host.input(source,{kind:'cast_unit',skill_id:skill,target:{id:target,generation:1}});await host.until(()=>facts.slice(start).some(r=>r.event.kind==='damage'&&r.event.source.id===source&&r.event.target.id===target));const slice=facts.slice(start);const hits=slice.filter(r=>r.event.kind==='damage'&&r.event.source.id===source&&r.event.target.id===target);assert.equal(hits.length,1);return {amount:hits[0].event.amount,events:slice};}
 try{
  await host.start();await host.joinAndReady();await host.input(source,{kind:'navigate',destination:[500,300]});await host.until(()=>Math.hypot(own().position[0]-500,own().position[1]-300)<5);await host.input(source,{kind:'stop'});
  const damage=[(await cast('break-bolt')).amount],consumed=[],marks=[];
  for(const layers of [1,2,3]){
   for(let n=0;n<layers;n++)await cast('mark-bolt');await host.until(()=>npc().statuses.some(s=>s.definition_id==='threshold-mark'&&s.stacks===layers));const before=structuredClone(npc().statuses.find(s=>s.definition_id==='threshold-mark'));marks.push(before);
   if(layers===2){await host.reconnect(source);await host.until(()=>!own().ready);await host.input(source,{kind:'foreground'});await host.input(source,{kind:'combat_ready',publication_id:actor.readyPublication??actor.publication});await host.until(()=>own().ready);assert.deepEqual(npc().statuses.find(s=>s.instance_id===before.instance_id),before);}
   const result=await cast('break-bolt');damage.push(result.amount);const used=result.events.filter(r=>r.event.kind==='status_consumed'&&r.event.instance_id===before.instance_id);assert.equal(used.length,1);assert.equal(used[0].recipient,source);consumed.push(used[0]);const remaining=layers-Math.min(layers,seats===4?3:2);
   if(remaining){await host.until(()=>npc().statuses.some(s=>s.instance_id===before.instance_id&&s.stacks===remaining));const after=npc().statuses.find(s=>s.instance_id===before.instance_id);assert.deepEqual(after,{...before,stacks:remaining});const next=await cast('break-bolt');damage.push(next.amount);const last=next.events.filter(r=>r.event.kind==='status_consumed'&&r.event.instance_id===before.instance_id);assert.equal(last.length,1);consumed.push(last[0]);}
   await host.until(()=>!npc().statuses.some(s=>s.definition_id==='threshold-mark'));
  }
  damage.push((await cast('break-bolt')).amount);assert.deepEqual(damage,seats===4?[50,64,78,94,50]:[50,78,111,111,78,50]);assert.deepEqual(consumed.map(r=>r.event.consumed_stacks),seats===4?[1,2,3]:[1,2,2,1]);
  assert.ok(consumed.every(r=>r.recipient===source));for(const member of host.members)await host.input(member.account.participant_id,{kind:'leave'});await host.until(()=>host.phase==='closed'&&host.pending()===0&&host.publicationsComplete());const ledgers=database.views(record.id);assert.ok(ledgers.every(r=>r.admissions===1&&r.settlements===1));assert.equal(host.errors.length,0);
  writeFileSync(join(output,scenario+'.contact-modifier-facts.json'),JSON.stringify({marks,consumed},null,2)+'\n');
  return {scenario,members:seats,damage,consumed:consumed.map(r=>r.event.consumed_stacks),partial:seats===8,reconnected:true,privateRecipients:1,settled:seats};
 }finally{await host.dispose();database.close();}
}

export async function verifyChainDamage({binary,hostPath,configs,output,scenario}) {
 const seats=scenario==='chain-four'?4:8,expected=seats===4?[80,60,54]:[50,25,18],ids=['chain-a','chain-b','chain-c'];
 const {TwoMemberDatabase,TwoMemberHost,newRecord,readCatalog}=await import(pathToFileURL(hostPath).href),manifest=join(configs,'catalog.json'),catalog=readCatalog(binary,manifest),record=newRecord({catalog,scenario,accounts:JSON.parse(readFileSync(join(configs,seats===4?'accounts-a.json':'accounts-b.json'),'utf8')),manifest});
 const database=new TwoMemberDatabase(join(output,`chain-${scenario}-${randomUUID()}.sqlite`));database.create(record);const facts=[];
 const host=new TwoMemberHost({binary,manifest,database,record,autoAck:true,onEvent(recipient,event){facts.push({recipient,event:event.kind==='private'?event.event:event});}}),actor=host.members[0],source=actor.account.participant_id,own=()=>actor.snapshot.self,npc=id=>actor.snapshot.monsters.find(n=>n.entity.id===id);
 async function cast(){await host.until(()=>own().pending_action===null&&actor.snapshot.logical_ms>=own().recovery_until_ms&&actor.snapshot.logical_ms>=own().player_skills.find(s=>s.skill_id==='mark-bolt').ready_at_ms);await host.input(source,{kind:'cast_unit',skill_id:'mark-bolt',target:{id:'capped-target',generation:1}});}
 async function reconnect(){await host.reconnect(source);await host.until(()=>!own().ready);await host.input(source,{kind:'foreground'});await host.input(source,{kind:'combat_ready',publication_id:actor.readyPublication??actor.publication});await host.until(()=>own().ready);}
 try{
  await host.start();await host.joinAndReady();await host.input(source,{kind:'navigate',destination:[500,300]});await host.until(()=>Math.hypot(own().position[0]-500,own().position[1]-300)<5);await host.input(source,{kind:'stop'});const marks=[];
  for(const n of [1,2]){await cast();await host.until(()=>npc('capped-target').statuses.some(s=>s.definition_id==='threshold-mark'&&s.stacks===n));marks.push(structuredClone(npc('capped-target').statuses.find(s=>s.definition_id==='threshold-mark')));}
  await reconnect();assert.deepEqual(npc('capped-target').statuses.find(s=>s.instance_id===marks[1].instance_id),marks[1]);await cast();await host.until(()=>facts.some(r=>r.event.kind==='chain_damage_resolved'));
  const resolved=facts.filter(r=>r.event.kind==='chain_damage_resolved');assert.equal(resolved.length,1);assert.equal(resolved[0].recipient,source);const chain=resolved[0].event;assert.equal(chain.reason,'complete');assert.equal(chain.initial_raw,expected[0]);assert.deepEqual(chain.hops.map(h=>h.target.id),ids);assert.deepEqual(chain.hops.map(h=>h.from.id),['capped-target','chain-a','chain-b']);assert.deepEqual(chain.hops.map(h=>h.raw_damage),expected);assert.ok(chain.hops.every(h=>h.applied));
  await host.until(()=>ids.every((id,i)=>npc(id).hp===10000-expected[i]&&npc(id).statuses.some(s=>s.source.id===source))&&!npc('capped-target').statuses.some(s=>s.definition_id==='threshold-mark'));
  const statuses=ids.map(id=>structuredClone(npc(id).statuses.find(s=>s.source.id===source)));assert.deepEqual(statuses.map(s=>s.definition_id),['compound-lock','compound-lock','control-softened']);assert.ok(statuses.every(s=>s.expires_at_ms-s.applied_at_ms===(seats===4?1200:1800)));assert.equal(npc('capped-target').hp,seats===4?9997:9700);
  const consumed=facts.filter(r=>r.event.kind==='status_consumed'&&r.event.instance_id===marks[0].instance_id);assert.equal(consumed.length,1);assert.equal(consumed[0].event.consumed_stacks,3);
  await reconnect();assert.deepEqual(ids.map(id=>npc(id).hp),expected.map(n=>10000-n));assert.equal(facts.filter(r=>r.event.kind==='chain_damage_resolved').length,1);
  await host.until(()=>ids.every(id=>!npc(id).statuses.some(s=>s.source.id===source)));assert.ok(facts.filter(r=>r.event.status?.source.id===source||r.event.kind==='chain_damage_resolved').every(r=>r.recipient===source));
  for(const member of host.members)await host.input(member.account.participant_id,{kind:'leave'});await host.until(()=>host.phase==='closed'&&host.pending()===0&&host.publicationsComplete());const ledgers=database.views(record.id);assert.ok(ledgers.every(r=>r.admissions===1&&r.settlements===1));assert.equal(host.errors.length,0);
  writeFileSync(join(output,scenario+'.chain-facts.json'),JSON.stringify({marks,consumed,resolved,statuses},null,2)+'\n');
  return {scenario,members:seats,damage:expected,targets:ids,consumed:3,privateRecipients:1,reconnected:true,noReplay:true,expired:true,statuses:['compound-lock','compound-lock','control-softened'],settled:seats};
 }finally{await host.dispose();database.close();}
}

export async function verifyPostConsumption({binary,hostPath,configs,output,scenario}) {
 const seats=scenario==='consumed-four'?4:8,raw=seats===4?175:130,pursuit=seats===4?105:52,bps=seats===4?3000:5000,fixed=seats===4?0:60;
 const {TwoMemberDatabase,TwoMemberHost,newRecord,readCatalog}=await import(pathToFileURL(hostPath).href),manifest=join(configs,'catalog.json'),catalog=readCatalog(binary,manifest),record=newRecord({catalog,scenario,accounts:JSON.parse(readFileSync(join(configs,seats===4?'accounts-a.json':'accounts-b.json'),'utf8')),manifest});
 const database=new TwoMemberDatabase(join(output,`consumed-${scenario}-${randomUUID()}.sqlite`));database.create(record);const facts=[];
 const host=new TwoMemberHost({binary,manifest,database,record,autoAck:true,onEvent(recipient,event){facts.push({recipient,event:event.kind==='private'?event.event:event});}}),actor=host.members[0],source=actor.account.participant_id,own=()=>actor.snapshot.self,npc=id=>actor.snapshot.monsters.find(n=>n.entity.id===id),skill=id=>own().player_skills.find(s=>s.skill_id===id);
 async function cast(id){await host.until(()=>own().pending_action===null&&actor.snapshot.logical_ms>=own().recovery_until_ms&&actor.snapshot.logical_ms>=skill(id).ready_at_ms);await host.input(source,{kind:'cast_unit',skill_id:id,target:{id:'capped-target',generation:1}});}
 async function reconnect(){await host.reconnect(source);await host.until(()=>!own().ready);await host.input(source,{kind:'foreground'});await host.input(source,{kind:'combat_ready',publication_id:actor.readyPublication??actor.publication});await host.until(()=>own().ready);}
 try{
  await host.start();await host.joinAndReady();await host.input(source,{kind:'navigate',destination:[500,300]});await host.until(()=>Math.hypot(own().position[0]-500,own().position[1]-300)<5);await host.input(source,{kind:'stop'});const marks=[];
  for(const n of [1,2]){await cast('mark-bolt');await host.until(()=>npc('capped-target').statuses.some(s=>s.definition_id==='threshold-mark'&&s.stacks===n));marks.push(structuredClone(npc('capped-target').statuses.find(s=>s.definition_id==='threshold-mark')));}
  await reconnect();assert.deepEqual(npc('capped-target').statuses.find(s=>s.instance_id===marks[1].instance_id),marks[1]);await cast('mark-bolt');await host.until(()=>npc('capped-target').statuses.some(s=>s.definition_id==='threshold-mark'&&s.stacks===3));await cast('break-bolt');await host.until(()=>facts.some(r=>r.event.kind==='skill_cooldowns_reduced')&&npc('chain-a').hp===10000-pursuit);
  const resolved=facts.filter(r=>r.event.kind==='chain_damage_resolved'),refunds=facts.filter(r=>r.event.kind==='skill_cooldowns_reduced'),consumed=facts.filter(r=>r.event.kind==='status_consumed'&&r.event.instance_id===marks[0].instance_id);assert.equal(resolved.length,1);assert.equal(refunds.length,1);assert.equal(consumed.length,1);assert.equal(consumed[0].event.consumed_stacks,3);
  const chain=resolved[0].event,refund=refunds[0].event,change=refund.changes[0];assert.deepEqual(chain.hops.map(h=>[h.target.id,h.raw_damage,h.applied]),[['chain-a',pursuit,true]]);assert.equal(chain.reason,'complete');assert.equal(npc('near-behind').hp,10000);assert.equal(npc('capped-target').hp,10000-3-raw);assert.equal(change.skill_id,'break-bolt');assert.equal(change.reduced_ms,Math.min(change.previous_ready_at_ms-refund.at_ms,Math.floor((change.previous_ready_at_ms-refund.at_ms)*bps/10000)+fixed));assert.equal(change.ready_at_ms,change.previous_ready_at_ms-change.reduced_ms);await host.until(()=>skill('break-bolt').ready_at_ms===change.ready_at_ms);
  const status=structuredClone(npc('chain-a').statuses.find(s=>s.source.id===source));assert.equal(status.definition_id,'compound-lock');assert.ok([...resolved,...refunds,...consumed].every(r=>r.recipient===source));await reconnect();assert.equal(skill('break-bolt').ready_at_ms,change.ready_at_ms);assert.equal(npc('chain-a').hp,10000-pursuit);assert.equal(facts.filter(r=>r.event.kind==='skill_cooldowns_reduced').length,1);assert.equal(facts.filter(r=>r.event.kind==='chain_damage_resolved').length,1);
  await cast('break-bolt');assert.ok(actor.snapshot.logical_ms<change.previous_ready_at_ms);await host.until(()=>npc('capped-target').hp===10000-3-raw-100);assert.equal(facts.filter(r=>r.event.kind==='skill_cooldowns_reduced').length,1);assert.equal(facts.filter(r=>r.event.kind==='chain_damage_resolved').length,1);await host.until(()=>!npc('chain-a').statuses.some(s=>s.source.id===source));
  for(const member of host.members)await host.input(member.account.participant_id,{kind:'leave'});await host.until(()=>host.phase==='closed'&&host.pending()===0&&host.publicationsComplete());assert.ok(database.views(record.id).every(r=>r.admissions===1&&r.settlements===1));assert.equal(host.errors.length,0);
  writeFileSync(join(output,scenario+'.post-consumption-facts.json'),JSON.stringify({marks,status,consumed,resolved,refunds},null,2)+'\n');return {scenario,members:seats,consumed:3,raw,pursuit,target:'chain-a',skippedBehind:true,remainingBps:bps,fixedMs:fixed,privateRecipients:1,reconnected:true,noReplay:true,readyBeforeOriginal:true,settled:seats};
 }finally{await host.dispose();database.close();}
}

export async function verifyContactReaction({binary,hostPath,configs,output,scenario}) {
 const seats=scenario==='reaction-four'?4:8,expected=seats===4?25:67;
 const {TwoMemberDatabase,TwoMemberHost,newRecord,readCatalog}=await import(pathToFileURL(hostPath).href),manifest=join(configs,'catalog.json'),catalog=readCatalog(binary,manifest),record=newRecord({catalog,scenario,accounts:JSON.parse(readFileSync(join(configs,seats===4?'accounts-a.json':'accounts-b.json'),'utf8')),manifest});
 const database=new TwoMemberDatabase(join(output,`reaction-${scenario}-${randomUUID()}.sqlite`));database.create(record);const facts=[];
 const host=new TwoMemberHost({binary,manifest,database,record,autoAck:true,onEvent(recipient,event){facts.push({recipient,event:event.kind==='private'?event.event:event});}}),provoker=host.members[0],source=host.members[1],own=m=>m.snapshot.self,npc=m=>m.snapshot.monsters.find(n=>n.entity.id==='capped-target');
 async function cast(m){await host.until(()=>own(m).pending_action===null&&m.snapshot.logical_ms>=own(m).recovery_until_ms&&m.snapshot.logical_ms>=own(m).player_skills.find(s=>s.skill_id==='mark-bolt').ready_at_ms);await host.input(m.account.participant_id,{kind:'cast_unit',skill_id:'mark-bolt',target:{id:'capped-target',generation:1}});}
 async function reconnect(){await host.reconnect('P01');await host.until(()=>!own(source).ready);await host.input('P01',{kind:'foreground'});await host.input('P01',{kind:'combat_ready',publication_id:source.readyPublication??source.publication});await host.until(()=>own(source).ready);}
 try{
  await host.start();await host.joinAndReady();for(const [m,pos] of [[provoker,[500,300]],[source,[480,240]]]){await host.input(m.account.participant_id,{kind:'navigate',destination:pos});await host.until(()=>Math.hypot(own(m).position[0]-pos[0],own(m).position[1]-pos[1])<5);await host.input(m.account.participant_id,{kind:'stop'});}
  const marks=[];for(const n of [1,2]){await cast(source);await host.until(()=>npc(source).statuses.some(s=>s.definition_id==='threshold-mark'&&s.stacks===n));marks.push(structuredClone(npc(source).statuses.find(s=>s.definition_id==='threshold-mark')));}
  await cast(source);await host.until(()=>npc(source).statuses.some(s=>s.definition_id==='compound-lock'));const frozen=structuredClone(npc(source).statuses.find(s=>s.definition_id==='compound-lock'));assert.equal(frozen.source.id,'P01');assert.equal(npc(source).hp,9997);assert.equal(facts.filter(r=>r.event.kind==='contact_reaction_resolved').length,0);
  await reconnect();assert.deepEqual(npc(source).statuses.find(s=>s.instance_id===frozen.instance_id),frozen);await cast(provoker);await host.until(()=>facts.some(r=>r.event.kind==='contact_reaction_resolved')&&npc(source).hp===9996-expected);
  const reactions=facts.filter(r=>r.event.kind==='contact_reaction_resolved'),consumed=facts.filter(r=>r.event.kind==='status_consumed'&&r.event.instance_id===frozen.instance_id);assert.equal(reactions.length,1);assert.equal(consumed.length,1);assert.equal(reactions[0].recipient,'P01');assert.equal(consumed[0].recipient,'P01');assert.equal(consumed[0].event.consumed_stacks,1);
  const r=reactions[0].event;assert.equal(r.source.id,'P01');assert.equal(r.provoker.id,'P00');assert.equal(r.target.id,'capped-target');assert.equal(r.raw_damage,expected);assert.equal(r.hp_damage,expected);assert.equal(r.absorbed,0);assert.ok(!npc(source).statuses.some(s=>s.instance_id===frozen.instance_id));assert.ok(facts.some(r=>r.event.kind==='damage'&&r.event.source?.id==='P01'&&r.event.amount===expected));
  await reconnect();assert.equal(npc(source).hp,9996-expected);assert.ok(!npc(source).statuses.some(s=>s.instance_id===frozen.instance_id));await cast(provoker);await host.until(()=>npc(source).hp===9995-expected);assert.equal(facts.filter(r=>r.event.kind==='contact_reaction_resolved').length,1);
  for(const member of host.members)await host.input(member.account.participant_id,{kind:'leave'});await host.until(()=>host.phase==='closed'&&host.pending()===0&&host.publicationsComplete());assert.ok(database.views(record.id).every(r=>r.admissions===1&&r.settlements===1));assert.equal(host.errors.length,0);
  writeFileSync(join(output,scenario+'.contact-reaction-facts.json'),JSON.stringify({marks,frozen,consumed,reactions},null,2)+'\n');return {scenario,members:seats,source:'P01',provoker:'P00',raw:expected,hpDamage:expected,consumed:1,privateRecipients:1,preexistingOnly:true,reconnected:true,noReplay:true,once:true,settled:seats};
 }finally{await host.dispose();database.close();}
}

export async function verifyFinisher({binary,hostPath,configs,output,scenario}) {
 const seats=scenario==='finisher-four'?4:8,ordinary=seats===4?400:200,replacement=seats===4?100:150,replacementHp=seats===4?300:50,targets=['capped-target','replacement-target','guard-target'];
 const {TwoMemberDatabase,TwoMemberHost,newRecord,readCatalog}=await import(pathToFileURL(hostPath).href),manifest=join(configs,'catalog.json'),catalog=readCatalog(binary,manifest),record=newRecord({catalog,scenario,accounts:JSON.parse(readFileSync(join(configs,seats===4?'accounts-a.json':'accounts-b.json'),'utf8')),manifest});
 const database=new TwoMemberDatabase(join(output,`finisher-${scenario}-${randomUUID()}.sqlite`));database.create(record);const facts=[];
 const host=new TwoMemberHost({binary,manifest,database,record,autoAck:true,onEvent(recipient,event){facts.push({recipient,event:event.kind==='private'?event.event:event});}}),actor=host.members[0],own=()=>actor.snapshot.self,npc=id=>actor.snapshot.monsters.find(n=>n.entity.id===id),done=id=>facts.filter(r=>r.event.kind==='finisher_resolved'&&r.event.target.id===id);
 async function cast(id){await host.until(()=>own().pending_action===null&&actor.snapshot.logical_ms>=own().recovery_until_ms&&actor.snapshot.logical_ms>=own().player_skills.find(s=>s.skill_id==='threshold-bolt').ready_at_ms);await host.input('P00',{kind:'cast_unit',skill_id:'threshold-bolt',target:{id,generation:1}});}
 async function reconnect(){await host.reconnect('P00');await host.until(()=>!own().ready);await host.input('P00',{kind:'foreground'});await host.input('P00',{kind:'combat_ready',publication_id:actor.readyPublication??actor.publication});await host.until(()=>own().ready);}
 try{
  await host.start();await host.joinAndReady();await host.input('P00',{kind:'navigate',destination:[500,300]});await host.until(()=>Math.hypot(own().position[0]-500,own().position[1]-300)<5);await host.input('P00',{kind:'stop'});
  for(const id of targets){await cast(id);await host.until(()=>done(id).length===1);}
  const resolved=targets.map(id=>done(id)[0]);assert.deepEqual(resolved.map(r=>r.event.mode),['execute','power_damage','execute']);assert.deepEqual(resolved.map(r=>r.event.raw_damage),[ordinary,replacement,ordinary]);assert.deepEqual(resolved.map(r=>r.event.hp_damage),[ordinary,replacement,ordinary-1]);assert.deepEqual(resolved.map(r=>r.event.lethal),[true,false,false]);assert.ok(resolved.every(r=>r.recipient==='P00'&&r.event.source.id==='P00'&&r.event.absorbed===0));
  await host.until(()=>!npc('capped-target')?.hp&&npc('replacement-target')?.hp===replacementHp&&npc('guard-target')?.hp===1);const consumed=facts.filter(r=>r.event.kind==='status_consumed'&&targets.includes(r.event.target.id)&&r.event.definition_id==='threshold-mark');assert.equal(consumed.length,3);assert.ok(consumed.every(r=>r.recipient==='P00'&&r.event.consumed_stacks===1));
  await reconnect();assert.ok(!npc('capped-target')?.hp);assert.equal(npc('replacement-target').hp,replacementHp);assert.equal(npc('guard-target').hp,1);assert.equal(facts.filter(r=>r.event.kind==='finisher_resolved').length,3);
  await cast('guard-target');await host.until(()=>!npc('guard-target')?.hp);assert.equal(facts.filter(r=>r.event.kind==='finisher_resolved').length,3);
  for(const member of host.members)await host.input(member.account.participant_id,{kind:'leave'});await host.until(()=>host.phase==='closed'&&host.pending()===0&&host.publicationsComplete());assert.ok(database.views(record.id).every(r=>r.admissions===1&&r.settlements===1));assert.equal(host.errors.length,0);
  writeFileSync(join(output,scenario+'.finisher-facts.json'),JSON.stringify({consumed,resolved},null,2)+'\n');return {scenario,members:seats,targets,modes:['execute','power_damage','execute'],raw:[ordinary,replacement,ordinary],lethal:[true,false,false],guardHp:1,replacementHp,consumed:3,privateRecipients:1,reconnected:true,noReplay:true,guardKilledAfterward:true,settled:seats};
 }finally{await host.dispose();database.close();}
}

export async function verifyFinisherFollowup({binary,hostPath,configs,output,scenario}) {
 const seats=scenario==='followup-four'?4:8,spiritRestored=seats===4?20:30,spiritAfter=80+spiritRestored,maxSpirit=seats===4?250:300,chainRaw=seats===4?320:120;
 const {TwoMemberDatabase,TwoMemberHost,newRecord,readCatalog}=await import(pathToFileURL(hostPath).href),manifest=join(configs,'catalog.json'),catalog=readCatalog(binary,manifest),record=newRecord({catalog,scenario,accounts:JSON.parse(readFileSync(join(configs,seats===4?'accounts-a.json':'accounts-b.json'),'utf8')),manifest});
 const database=new TwoMemberDatabase(join(output,`followup-${scenario}-${randomUUID()}.sqlite`));database.create(record);const facts=[];
 const host=new TwoMemberHost({binary,manifest,database,record,autoAck:true,onEvent(recipient,event){facts.push({recipient,event:event.kind==='private'?event.event:event});}}),actor=host.members[0],own=()=>actor.snapshot.self,npc=id=>actor.snapshot.monsters.find(n=>n.entity.id===id),skill=()=>own().player_skills.find(s=>s.skill_id==='threshold-bolt');
 const leaves=['finish-state','finish-spirit','finish-refund','finish-chain'],post=()=>facts.filter(r=>r.event.kind==='combat_trigger_fired'&&leaves.includes(r.event.trigger_id)),results=()=>facts.filter(r=>r.event.kind==='finisher_resolved');
 async function cast(id){await host.until(()=>own().pending_action===null&&actor.snapshot.logical_ms>=own().recovery_until_ms&&actor.snapshot.logical_ms>=skill().ready_at_ms);await host.input('P00',{kind:'cast_unit',skill_id:'threshold-bolt',target:{id,generation:1}});}
 async function reconnect(){await host.reconnect('P00');await host.until(()=>!own().ready);await host.input('P00',{kind:'foreground'});await host.input('P00',{kind:'combat_ready',publication_id:actor.readyPublication??actor.publication});await host.until(()=>own().ready);}
 try{
  await host.start();await host.joinAndReady();await host.input('P00',{kind:'navigate',destination:[500,300]});await host.until(()=>Math.hypot(own().position[0]-500,own().position[1]-300)<5);await host.input('P00',{kind:'stop'});
  for(const [i,id] of ['guard-target','replacement-target'].entries()){await cast(id);await host.until(()=>results().length===i+1);assert.equal(post().length,0);assert.equal(own().spirit,200-(i+1)*40);}
  assert.equal(npc('guard-target').hp,1);assert.equal(npc('replacement-target').hp,seats===4?300:50);
  await cast('capped-target');await host.until(()=>post().length===4&&own().spirit===spiritAfter&&npc('follow-target').hp===1000-chainRaw);
  assert.deepEqual(post().map(r=>r.event.trigger_id),leaves);assert.ok(post().every(r=>r.recipient==='P00'&&r.event.source.id==='P00'));
  const status=structuredClone(own().statuses.find(s=>s.definition_id==='afterglow'));assert.ok(status);assert.equal(status.source.id,'P00');assert.equal(status.target.id,'P00');assert.equal(status.expires_at_ms-status.applied_at_ms,seats===4?3000:4500);assert.equal(own().max_spirit,maxSpirit);
  const refunds=facts.filter(r=>r.event.kind==='skill_cooldowns_reduced'),chains=facts.filter(r=>r.event.kind==='chain_damage_resolved'),resolved=results();assert.equal(refunds.length,1);assert.equal(chains.length,1);assert.ok([...refunds,...chains,...resolved].every(r=>r.recipient==='P00'));
  assert.deepEqual(resolved.map(r=>r.event.lethal),[false,false,true]);assert.deepEqual(resolved.map(r=>r.event.mode),['execute','power_damage','execute']);
  const refund=refunds[0].event,change=refund.changes[0],bps=seats===4?3000:5000,fixed=seats===4?0:60;
  assert.equal(change.reduced_ms,Math.floor((change.previous_ready_at_ms-refund.at_ms)*bps/10000)+fixed);assert.equal(change.ready_at_ms,change.previous_ready_at_ms-change.reduced_ms);assert.equal(skill().ready_at_ms,change.ready_at_ms);
  const chain=chains[0].event;assert.equal(chain.source.id,'P00');assert.equal(chain.origin.id,'capped-target');assert.equal(chain.reason,'complete');assert.equal(chain.initial_raw,chainRaw);assert.equal(chain.hops.length,1);assert.equal(chain.hops[0].target.id,'follow-target');assert.equal(chain.hops[0].raw_damage,chainRaw);assert.equal(chain.hops[0].applied,true);
  await reconnect();assert.deepEqual(own().statuses.find(s=>s.instance_id===status.instance_id),status);assert.equal(own().spirit,spiritAfter);assert.equal(own().max_spirit,maxSpirit);assert.equal(skill().ready_at_ms,change.ready_at_ms);assert.equal(npc('follow-target').hp,1000-chainRaw);assert.equal(post().length,4);assert.equal(results().length,3);assert.equal(facts.filter(r=>r.event.kind==='chain_damage_resolved').length,1);
  await cast('guard-target');assert.ok(actor.snapshot.logical_ms<change.previous_ready_at_ms);await host.until(()=>!npc('guard-target')?.hp);assert.equal(post().length,4);assert.equal(results().length,3);assert.equal(facts.filter(r=>r.event.kind==='skill_cooldowns_reduced').length,1);assert.equal(own().spirit,spiritAfter-40);
  await host.until(()=>!own().statuses.some(s=>s.instance_id===status.instance_id));assert.equal(own().max_spirit,200);assert.equal(facts.filter(r=>r.event.kind==='status_removed'&&r.event.reason==='expired'&&r.event.instance_ids.includes(status.instance_id)).length,1);assert.equal(post().length,4);
  writeFileSync(join(output,scenario+'.finisher-followup-facts.json'),JSON.stringify({status,refunds,chains,resolved,post:post()},null,2)+'\n');
  for(const member of host.members)await host.input(member.account.participant_id,{kind:'leave'});await host.until(()=>host.phase==='closed'&&host.pending()===0&&host.publicationsComplete());assert.ok(database.views(record.id).every(r=>r.admissions===1&&r.settlements===1));assert.equal(host.errors.length,0);
  return {scenario,members:seats,provider:'threshold-finish',mode:'execute',postEffects:4,spiritRestored,spiritAfter,maxSpirit,chainRaw,chainTarget:'follow-target',privateRecipients:1,reconnected:true,noReplay:true,readyBeforeOriginal:true,expired:true,settled:seats};
 }finally{await host.dispose();database.close();}
}

export async function verifyActionEnhancement({binary,hostPath,configs,output,scenario}) {
 const seats=scenario==='enhancement-four'?4:8,primaryRaw=seats===4?450:460,thresholdBps=seats===4?5500:6000,spiritAfterLine=seats===4?156:160;
 const {TwoMemberDatabase,TwoMemberHost,newRecord,readCatalog}=await import(pathToFileURL(hostPath).href),manifest=join(configs,'catalog.json'),catalog=readCatalog(binary,manifest),record=newRecord({catalog,scenario,accounts:JSON.parse(readFileSync(join(configs,seats===4?'accounts-a.json':'accounts-b.json'),'utf8')),manifest});
 const database=new TwoMemberDatabase(join(output,`enhancement-${scenario}-${randomUUID()}.sqlite`));database.create(record);const facts=[];
 const host=new TwoMemberHost({binary,manifest,database,record,autoAck:true,onEvent(recipient,event){facts.push({recipient,event:event.kind==='private'?event.event:event});}}),actor=host.members[0],own=()=>actor.snapshot.self,npc=id=>actor.snapshot.monsters.find(n=>n.entity.id===id),status=()=>own().statuses.find(s=>s.definition_id==='afterglow'),done=()=>facts.filter(r=>r.event.kind==='finisher_resolved'),grants=()=>facts.filter(r=>r.event.kind==='status_applied'&&r.event.status.definition_id==='afterglow'),uses=()=>facts.filter(r=>r.event.kind==='status_consumed'&&r.event.definition_id==='afterglow');
 async function cast(skill,id){await host.until(()=>own().pending_action===null&&actor.snapshot.logical_ms>=own().recovery_until_ms&&actor.snapshot.logical_ms>=own().player_skills.find(s=>s.skill_id===skill).ready_at_ms);await host.input('P00',id?{kind:'cast_unit',skill_id:skill,target:{id,generation:1}}:{kind:'cast',skill_id:skill,direction:[1,0]});}
 async function reconnect(){await host.reconnect('P00');await host.until(()=>!own().ready);await host.input('P00',{kind:'foreground'});await host.input('P00',{kind:'combat_ready',publication_id:actor.readyPublication??actor.publication});await host.until(()=>own().ready);}
 try {
  await host.start();await host.joinAndReady();await host.input('P00',{kind:'navigate',destination:[500,300]});await host.until(()=>Math.hypot(own().position[0]-500,own().position[1]-300)<5);await host.input('P00',{kind:'stop'});
  await cast('threshold-bolt','capped-target');await host.until(()=>status()&&done().length===1);const initial=structuredClone(status());assert.equal(own().spirit,180);assert.equal(own().max_spirit,seats===4?250:300);assert.equal(grants().length,1);
  await reconnect();assert.deepEqual(status(),initial);assert.equal(done().length,1);assert.equal(grants().length,1);
  await cast('surge-line');await host.until(()=>done().some(r=>r.event.target.id==='line-a')&&own().spirit===spiritAfterLine&&!status());
  const before=structuredClone(actor.snapshot.projectiles.find(p=>p.source.id==='P00'&&p.piercing));assert.ok(before);assert.equal(done().length,2);assert.equal(uses().length,1);assert.equal(uses()[0].event.instance_id,initial.instance_id);assert.equal(uses()[0].event.consumed_stacks,1);
  await reconnect();const after=structuredClone(actor.snapshot.projectiles.find(p=>p.id===before.id));assert.ok(after);assert.deepEqual({...after.piercing,remaining_range:0},{...before.piercing,remaining_range:0});assert.ok(after.piercing.remaining_range<=before.piercing.remaining_range&&after.piercing.remaining_range>0);assert.ok(!status());assert.equal(own().spirit,spiritAfterLine);assert.equal(done().length,2);
  await host.until(()=>done().length===3&&!npc('line-b')?.hp);assert.ok(!status());assert.equal(grants().length,1);assert.equal(uses().length,1);assert.equal(own().spirit,spiritAfterLine);
  const primaryHits=['line-a','line-b'].map(id=>facts.find(r=>r.event.kind==='damage'&&r.event.source.id==='P00'&&r.event.target.id===id&&r.event.hp>0));assert.ok(primaryHits.every(r=>r&&r.event.amount===primaryRaw&&r.event.hp===1000-primaryRaw));
  const refunds=facts.filter(r=>r.event.kind==='skill_cooldowns_reduced'&&r.event.trigger_id==='enhance-refund'),rewards=facts.filter(r=>r.event.kind==='combat_trigger_fired'&&r.event.trigger_id==='enhance-spirit');assert.equal(refunds.length,1);assert.equal(rewards.length,1);assert.equal(refunds[0].event.source.id,'P00');const refund=refunds[0].event,change=refund.changes[0];assert.equal(change.reduced_ms,Math.floor((change.previous_ready_at_ms-refund.at_ms)*(seats===4?3000:5000)/10000));assert.equal(own().player_skills.find(s=>s.skill_id==='surge-line').ready_at_ms,change.ready_at_ms);
  await cast('threshold-bolt','renew-target');await host.until(()=>done().length===4&&status());const renewed=structuredClone(status());assert.notEqual(renewed.instance_id,initial.instance_id);assert.equal(grants().length,2);assert.equal(uses().length,1);assert.equal(own().spirit,spiritAfterLine-20);assert.equal(renewed.expires_at_ms-renewed.applied_at_ms,seats===4?5000:6500);
  assert.deepEqual(done().map(r=>r.event.target.id),['capped-target','line-a','line-b','renew-target']);assert.ok(done().every(r=>r.event.lethal&&r.event.mode==='execute'));assert.deepEqual(done().map(r=>r.event.raw_damage),[400,1000-primaryRaw,1000-primaryRaw,400]);assert.ok([...done(),...grants(),...uses(),...refunds,...rewards].every(r=>r.recipient==='P00'));
  await host.until(()=>!status());assert.equal(own().max_spirit,200);assert.equal(facts.filter(r=>r.event.kind==='status_removed'&&r.event.reason==='expired'&&r.event.instance_ids.includes(renewed.instance_id)).length,1);assert.equal(grants().length,2);
  writeFileSync(join(output,scenario+'.action-enhancement-facts.json'),JSON.stringify({initial,renewed,consumptions:uses(),finishers:done(),refunds,primaryHits,flightBefore:before,flightAfter:after},null,2)+'\n');
  for(const member of host.members)await host.input(member.account.participant_id,{kind:'leave'});await host.until(()=>host.phase==='closed'&&host.pending()===0&&host.publicationsComplete());assert.ok(database.views(record.id).every(r=>r.admissions===1&&r.settlements===1));assert.equal(host.errors.length,0);
  return {scenario,members:seats,targets:['line-a','line-b'],thresholdBps,primaryRaw,consumed:1,enhancedKills:2,grantCount:2,spiritAfterLine,spiritRewards:1,cooldownRewards:1,privateRecipients:1,reconnected:true,inFlightReconnect:true,noSelfRefresh:true,newActionGranted:true,expired:true,settled:seats};
 }finally{await host.dispose();database.close();}
}

export async function verifyCastLifecycle({binary,hostPath,configs,output,scenario}) {
 const {TwoMemberDatabase,TwoMemberHost,newRecord,readCatalog}=await import(pathToFileURL(hostPath).href);
 const manifest=join(configs,'catalog.json'),catalog=readCatalog(binary,manifest),four=scenario==='casting-four';
 const accounts=JSON.parse(readFileSync(join(configs,four?'casting-accounts-a.json':'casting-accounts-b.json'),'utf8'));
 const record=newRecord({catalog,scenario,accounts,manifest}),database=new TwoMemberDatabase(join(output,`casting-${scenario}-${randomUUID()}.sqlite`));database.create(record);
 const facts=[],host=new TwoMemberHost({binary,manifest,database,record,autoAck:true,onEvent(recipient,event){facts.push({recipient,event:event.kind==='private'?event.event:event});}});
 const actor=host.members[0],own=()=>actor.snapshot.self,events=kind=>facts.filter(r=>r.event.kind===kind&&r.recipient==='P00');
 async function cast(skill,kind='cast',takeover=false){await host.until(()=>!own().pending_action&&!own().active_channel&&(takeover||actor.snapshot.logical_ms>=own().recovery_until_ms));await host.input('P00',kind==='cast_self'?{kind,skill_id:skill}:{kind,skill_id:skill,direction:[1,0]});}
 async function reconnect(){await host.reconnect('P00');await host.until(()=>!own().ready);await host.input('P00',{kind:'foreground'});await host.input('P00',{kind:'combat_ready',publication_id:actor.readyPublication??actor.publication});await host.until(()=>own().ready);}
 try {
  await host.start();await host.joinAndReady();
  const large=host.members[1];assert.equal(large.snapshot.self.spirit,8000000);await host.input('P01',{kind:'cast',skill_id:'large-cost',direction:[1,0]});await host.until(()=>large.snapshot.self.spirit===0);assert.equal(large.snapshot.self.hp,1);assert.equal(facts.filter(r=>r.recipient==='P01'&&r.event.kind==='action_committed'&&r.event.spirit_cost===8000000&&r.event.hp_cost===7999999).length,1);
  const before={hp:own().hp,spirit:own().spirit};
  await cast('breakable-windup');await host.until(()=>!!own().pending_action);await host.input('P00',{kind:'move',direction:[0,1]});await host.until(()=>!own().pending_action);await host.input('P00',{kind:'stop'});
  assert.deepEqual({hp:own().hp,spirit:own().spirit},before);assert.equal(events('action_committed').length,0);
  await cast('queued-charge','start_charge');await host.until(()=>!!own().pending_action?.charge);
  const charge=structuredClone(own().pending_action.charge);await host.input('P00',{kind:'release_charge',action_id:charge.action_id});await host.until(()=>own().pending_action?.charge.released_at_ms!==undefined);
  assert.equal(own().pending_action.commits_at_ms,charge.min_release_at_ms+(charge.release_windup_ms??0));assert.equal(own().spirit,before.spirit);
  await host.until(()=>events('action_committed').some(r=>r.event.action_id===charge.action_id));
  assert.equal(events('action_committed').filter(r=>r.event.action_id===charge.action_id).length,1);assert.equal(events('action_committed').find(r=>r.event.action_id===charge.action_id).event.charge.damage_factor_bps,4000);
  assert.ok(actor.snapshot.logical_ms<own().recovery_until_ms);await host.input('P00',{kind:'move',direction:[0,1]});await host.until(()=>own().recovery_until_ms<=actor.snapshot.logical_ms);await host.input('P00',{kind:'stop'});
  await cast('mobile-channel');await host.until(()=>!!own().active_channel);const origin=[...own().position];await host.input('P00',{kind:'move',direction:[0,1]});await host.until(()=>own().active_channel?.pulses>=1&&own().position[1]>origin[1]);await host.input('P00',{kind:'stop'});
  const channelId=own().active_channel.action_id;await host.input('P00',{kind:'cancel_action'});await host.until(()=>!own().active_channel);const pulseCount=events('channel_pulse').filter(r=>r.event.action_id===channelId).length;
  const flight=structuredClone(actor.snapshot.projectiles.find(p=>p.id===events('channel_pulse').find(r=>r.event.action_id===channelId).event.projectile_id));assert.ok(flight);await reconnect();assert.equal(events('channel_pulse').filter(r=>r.event.action_id===channelId).length,pulseCount);
  const previousRecovery=own().recovery_until_ms;assert.ok(actor.snapshot.logical_ms<previousRecovery);await cast('recovery-channel','cast_self',true);await host.until(()=>!!own().active_channel);assert.ok(own().active_channel.started_at_ms<previousRecovery);assert.equal(own().active_channel.recovery,true);assert.equal(own().active_channel.total_pulses,3);
  const recoveryId=own().active_channel.action_id;await host.until(()=>events('channel_recovery_pulse').filter(r=>r.event.action_id===recoveryId).length===3&&!own().active_channel);
  const recovery=events('channel_recovery_pulse').filter(r=>r.event.action_id===recoveryId).map(r=>r.event);
  assert.equal(recovery.reduce((n,r)=>n+r.raw_hp,0),four?17:19);assert.equal(recovery.reduce((n,r)=>n+r.raw_spirit,0),four?23:29);
  assert.deepEqual(recovery.map(r=>r.raw_hp),four?[6,6,5]:[6,6,7]);assert.ok(facts.filter(r=>r.event.kind==='channel_recovery_pulse').every(r=>r.recipient==='P00'));
  await reconnect();assert.equal(events('channel_recovery_pulse').length,3);assert.equal(events('action_committed').filter(r=>r.event.skill_id==='recovery-channel').length,1);
  for(const member of host.members)await host.input(member.account.participant_id,{kind:'leave'});await host.until(()=>host.phase==='closed'&&host.pending()===0&&host.publicationsComplete());
  const ledger=database.views(record.id);assert.ok(ledger.every(r=>r.admissions===1&&r.settlements===1));assert.equal(host.errors.length,0);
  writeFileSync(join(output,scenario+'.cast-lifecycle-facts.json'),JSON.stringify({charge,mobileChannel:channelId,pulseCount,flight,recovery},null,2)+'\n');
  return {scenario,members:host.members.length,largeCost:8000000,movementRecovery:true,atomicSelfTakeover:true,interruptedPreparationFree:true,queuedReleaseOnce:true,mobileChannel:true,committedFlightPreserved:true,reconnected:true,recoveryPulses:3,recoveryRawHp:four?17:19,recoveryRawSpirit:four?23:29,private:true,settled:ledger.length};
 }finally{await host.dispose();database.close();}
}

export async function verifyPeriodicComposition({binary,hostPath,configs,output,scenario}) {
 const {TwoMemberDatabase,TwoMemberHost,newRecord,readCatalog}=await import(pathToFileURL(hostPath).href);
 const manifest=join(configs,'catalog.json'),catalog=readCatalog(binary,manifest),four=scenario==='composed-four',accounts=JSON.parse(readFileSync(join(configs,four?'composed-accounts-a.json':'composed-accounts-b.json'),'utf8'));
 const record=newRecord({catalog,scenario,accounts,manifest}),database=new TwoMemberDatabase(join(output,`composed-${scenario}-${randomUUID()}.sqlite`));database.create(record);
 const facts=[],host=new TwoMemberHost({binary,manifest,database,record,autoAck:true,onEvent(recipient,event){facts.push({recipient,event:event.kind==='private'?event.event:event});}});
 const own=i=>host.members[i].snapshot.self,burn=i=>own(i).statuses.find(s=>s.definition_id==='cauldron-burn'),rows=(kind,pid='P00')=>facts.filter(r=>r.recipient===pid&&r.event.kind===kind);
 async function cast(i,id,target){const m=host.members[i];await host.until(()=>!own(i).pending_action&&m.snapshot.logical_ms>=own(i).recovery_until_ms&&m.snapshot.logical_ms>=own(i).player_skills.find(s=>s.skill_id===id).ready_at_ms);const start=facts.length;await host.input(m.account.participant_id,target?{kind:'cast_unit',skill_id:id,target}:{kind:'cast_self',skill_id:id});await host.until(()=>facts.slice(start).some(r=>r.recipient===m.account.participant_id&&r.event.kind==='action_committed'&&r.event.skill_id===id));}
 try{
  await host.start();await host.joinAndReady();await host.until(()=>burn(0)?.stacks>=3);
  const npc=host.members[0].snapshot.monsters[0];await cast(2,'freeze-source',{id:npc.entity.id,generation:1});
  await host.until(()=>host.members[2].snapshot.monsters[0].statuses?.some(s=>s.attribute==='stun'));
  // Flush the one already committed shot before observing a stable layered instance.
  await host.until(()=>!host.members[0].snapshot.projectiles.some(s=>s.source.kind==='monster'));
  const before=structuredClone(burn(0)),layers=before.stacks,id=before.instance_id;
  assert.ok(layers>=3&&layers<=10);assert.equal(before.periodic.per_stack,true);assert.equal(before.periodic.interval_ms,500);assert.equal(before.periodic.power_sampling,four?'target_live':'target_snapshot');
  await cast(0,'bloom');await host.until(()=>own(0).effective_atk===120);
  const hot=own(0).statuses.find(s=>s.definition_id==='compound-bloom');await host.until(()=>rows('periodic_tick').some(r=>r.event.instance_id===id&&r.event.executed_at_ms>=hot.applied_at_ms));assert.deepEqual(hot.flags,['control_immunity']);assert.equal(hot.modifiers.length,2);
  const first=rows('periodic_tick').filter(r=>r.event.instance_id===id&&r.event.executed_at_ms>=hot.applied_at_ms).at(-1).event;assert.equal(first.raw_damage,Math.floor((four?120:100)*700*layers/10000));assert.equal(first.stacks,layers);
  await cast(1,'take-burn',{id:'P00',generation:1});await host.until(()=>!!burn(1)&&!burn(0));
  const moved=structuredClone(burn(1));for(const k of ['instance_id','source','applied_at_ms','expires_at_ms','stacks','max_stacks','modifiers'])assert.deepEqual(moved[k],before[k]);assert.equal(moved.target.id,'P01');assert.equal(moved.periodic.first_tick_at_ms,before.periodic.first_tick_at_ms);
  const transferred=facts.filter(r=>r.event.kind==='status_transferred'&&r.event.status.instance_id===id);assert.deepEqual(transferred.map(r=>r.recipient).sort(),['P00','P01']);
  await host.input('P00',{kind:'leave'});await host.reconnect('P01');await host.until(()=>!own(1).ready);await host.input('P01',{kind:'foreground'});await host.input('P01',{kind:'combat_ready',publication_id:host.members[1].readyPublication??host.members[1].publication});await host.until(()=>own(1).ready);
  assert.equal(burn(1).instance_id,id);assert.equal(burn(1).stacks,layers);
  await cast(1,'bloom');await host.until(()=>own(1).effective_atk===240);const lateHot=own(1).statuses.find(s=>s.definition_id==='compound-bloom');await host.until(()=>rows('periodic_tick','P01').some(r=>r.event.instance_id===id&&r.event.executed_at_ms>=lateHot.applied_at_ms));
  const late=rows('periodic_tick','P01').filter(r=>r.event.instance_id===id&&r.event.executed_at_ms>=lateHot.applied_at_ms).at(-1).event;assert.equal(late.raw_damage,Math.floor((four?240:100)*700*layers/10000));assert.equal(late.stacks,layers);
  await host.until(()=>rows('status_cleanse_resolved','P01').some(r=>r.event.phase==='expired'&&r.event.removed_count===1));
  assert.ok(!burn(1));const count=rows('periodic_tick','P01').filter(r=>r.event.instance_id===id).length;
  const endedAt=host.members[1].snapshot.logical_ms;await host.until(()=>host.members[1].snapshot.logical_ms>=endedAt+600);assert.equal(rows('periodic_tick','P01').filter(r=>r.event.instance_id===id).length,count);
  const ticks=facts.filter(r=>r.event.kind==='periodic_tick'&&r.event.instance_id===id);assert.ok(ticks.every(r=>r.recipient===r.event.target.id&&r.event.source.id===npc.entity.id&&r.event.stacks>=1));assert.ok(ticks.filter(r=>r.event.executed_at_ms>=before.applied_at_ms).every(r=>r.event.scheduled_at_ms===before.periodic.first_tick_at_ms+(r.event.tick_index-1)*500));
  const afterTransfer=ticks.filter(r=>r.event.target.id==='P01');assert.equal(new Set(afterTransfer.map(r=>r.event.tick_index)).size,afterTransfer.length);
  assert.ok(!facts.some(r=>!['P00','P01'].includes(r.recipient)&&r.event.kind==='periodic_tick'&&r.event.instance_id===id));
  writeFileSync(join(output,scenario+'.periodic-composition-facts.json'),JSON.stringify({before,moved,first,late,events:facts.filter(r=>r.event.instance_id===id||r.event.status?.instance_id===id||r.event.kind==='status_cleanse_resolved')},null,2)+'\n');
  for(const m of host.members.slice(1))await host.input(m.account.participant_id,{kind:'leave'});await host.until(()=>host.phase==='closed'&&host.pending()===0&&host.publicationsComplete());const ledgers=database.views(record.id);assert.ok(ledgers.every(r=>r.admissions===1&&r.settlements===1));assert.equal(host.errors.length,0);
  return {scenario,members:host.members.length,layers,sampling:four?'target_live':'target_snapshot',firstRaw:first.raw_damage,transferredRaw:late.raw_damage,source:npc.entity.id,target:'P01',transferRecipients:2,subsequentRecipients:1,oldHolderLeft:true,reconnected:true,cleanseRemoved:1,noLateTick:true,settled:ledgers.length};
 }catch(error){writeFileSync(join(output,scenario+'.composed-error.json'),JSON.stringify({error:String(error),snapshots:host.members.map(m=>m.snapshot),facts,errors:host.errors},null,2));throw error;}finally{await host.dispose();database.close();}
}

export async function verifySkillLoadouts({binary,hostPath,configs,output,scenario}){
 const {TwoMemberDatabase,TwoMemberHost,newRecord,readCatalog}=await import(pathToFileURL(hostPath).href);
 const manifest=join(configs,'catalog.json'),catalog=readCatalog(binary,manifest),four=scenario==='loadouts-four',accounts=JSON.parse(readFileSync(join(configs,four?'loadouts-accounts-a.json':'loadouts-accounts-b.json'),'utf8'));
 const record=newRecord({catalog,scenario,accounts,manifest}),database=new TwoMemberDatabase(join(output,scenario+'-'+randomUUID()+'.sqlite'));database.create(record);
 const facts=[],host=new TwoMemberHost({binary,manifest,database,record,autoAck:true,onEvent(recipient,event){facts.push({recipient,event:event.kind==='private'?event.event:event});}});
 const actor=host.members[0],own=()=>actor.snapshot.self,ids=()=>own().player_skills.map(s=>s.skill_id),events=kind=>facts.filter(r=>r.recipient==='P00'&&r.event.kind===kind),receipts=[];
 const change=(operation_id,expected_revision,owned_skill_ids,equipped_skill_ids)=>({operation_id,expected_revision,loadout:{owned_skill_ids,equipped_skill_ids}});
 async function grant(body){const receipt=await host.setSkillLoadout('P00',body);receipts.push(receipt);await host.until(()=>own().skill_loadout_revision===receipt.revision);return receipt;}
 async function cast(id){await host.until(()=>!own().pending_action&&actor.snapshot.logical_ms>=own().recovery_until_ms);await host.input('P00',{kind:'cast',skill_id:id,direction:[1,0]});}
 try{
  await host.start();await host.joinAndReady();assert.deepEqual(ids(),['old-bolt']);assert.equal(own().skill_loadout_revision,0);assert.deepEqual(host.members[1].snapshot.self.player_skills.map(s=>s.skill_id),['learned-bolt']);assert.ok(host.members.slice(2).every(m=>m.snapshot.self.player_skills.length===0));
  const peerSpirit=host.members[1].snapshot.self.spirit;await assert.rejects(host.input('P01',{kind:'cast',skill_id:'old-bolt',direction:[1,0]}),e=>e.code==='INVALID_TARGET');await host.pump();assert.equal(host.members[1].snapshot.self.spirit,peerSpirit);
  const unpaid=own().spirit;await cast('old-bolt');await host.until(()=>!!own().pending_action);
  const swap=change('swap-prep',0,['old-bolt','learned-bolt'],['learned-bolt']);const first=await grant(swap);assert.ok(!own().pending_action);assert.equal(own().spirit,unpaid);const after=actor.snapshot.logical_ms;await host.until(()=>actor.snapshot.logical_ms>=after+1300);assert.equal(events('action_committed').length,0);
  const count=events('skill_loadout_changed').length;assert.deepEqual(await host.setSkillLoadout('P00',swap),first);await host.pump();assert.equal(events('skill_loadout_changed').length,count);
  await assert.rejects(host.setSkillLoadout('P00',{...swap,loadout:{owned_skill_ids:['old-bolt'],equipped_skill_ids:['old-bolt']}}),e=>e.code==='CONFLICT');
  await assert.rejects(host.setSkillLoadout('P00',change('stale',0,['old-bolt'],['old-bolt'])),e=>e.code==='REVISION_CONFLICT');assert.equal(own().skill_loadout_revision,1);
  await grant(change('return-old',1,['old-bolt','learned-bolt'],['old-bolt']));await cast('old-bolt');await host.until(()=>events('action_committed').some(r=>r.event.skill_id==='old-bolt'));
  await host.until(()=>own().player_skills[0].ready_at_ms>actor.snapshot.logical_ms);const oldReady=own().player_skills[0].ready_at_ms;const oldFlight=structuredClone(actor.snapshot.projectiles.find(p=>p.source.id==='P00'));assert.ok(oldFlight);assert.equal(own().spirit,unpaid-2);
  await grant(change('forget',2,[],[]));assert.deepEqual(ids(),[]);assert.ok(actor.snapshot.projectiles.some(p=>p.id===oldFlight.id));
  await grant(change('learn',3,['learned-bolt'],['learned-bolt']));await cast('learned-bolt');await host.until(()=>events('action_committed').some(r=>r.event.skill_id==='learned-bolt'));const paid=own().spirit;assert.equal(paid,unpaid-4);
  await grant(change('restore',4,['old-bolt','learned-bolt'],['learned-bolt','old-bolt']));assert.deepEqual(ids(),['learned-bolt','old-bolt']);assert.equal(own().player_skills.find(s=>s.skill_id==='old-bolt').ready_at_ms,oldReady);
  await assert.rejects(cast('old-bolt'),e=>e.code==='INVALID_STATE');await host.pump();assert.equal(own().spirit,paid);
  await grant(change('reorder',5,['old-bolt','learned-bolt'],['old-bolt','learned-bolt']));assert.deepEqual(ids(),['old-bolt','learned-bolt']);
  await host.reconnect('P00');await host.until(()=>!own().ready);await host.input('P00',{kind:'foreground'});await host.input('P00',{kind:'combat_ready',publication_id:actor.readyPublication??actor.publication});await host.until(()=>own().ready);
  assert.equal(own().skill_loadout_revision,6);assert.equal(own().player_skills[0].ready_at_ms,oldReady);assert.equal(own().spirit,paid);assert.ok(actor.snapshot.projectiles.some(p=>p.id===oldFlight.id));assert.equal(events('skill_loadout_changed').length,6);
  assert.ok(!facts.some(r=>r.recipient!=='P00'&&r.event.kind==='skill_loadout_changed'));
  const preserved=structuredClone(actor.snapshot.projectiles.find(p=>p.id===oldFlight.id));writeFileSync(join(output,scenario+'.loadout-facts.json'),JSON.stringify({initial:accounts.participants.map(a=>a.skills),receipts,projection:{skill_loadout_revision:own().skill_loadout_revision,player_skills:own().player_skills},controlCheckpoint:database.room(record.id).checkpoint.state.skill_control,oldReady,oldFlight,preserved,events:facts.filter(r=>['skill_loadout_changed','action_committed','action_interrupted','action_rejected'].includes(r.event.kind))},null,2)+'\n');
  for(const m of host.members)await host.input(m.account.participant_id,{kind:'leave'});await host.until(()=>host.phase==='closed'&&host.pending()===0&&host.publicationsComplete());const ledgers=database.views(record.id);assert.ok(ledgers.every(r=>r.admissions===1&&r.settlements===1));assert.equal(host.errors.length,0);
  return {scenario,members:host.members.length,revision:6,privateRecipients:1,preparedCancelledFree:true,emptySlots:true,slotOrder:['old-bolt','learned-bolt'],oldCooldownPreserved:true,committedFlightPreserved:true,paidSpirit:4,exactRetryOnce:true,casRejected:true,reconnected:true,settled:ledgers.length};
 }catch(error){writeFileSync(join(output,scenario+'.loadout-error.json'),JSON.stringify({error:String(error),snapshots:host.members.map(m=>m.snapshot),facts,errors:host.errors},null,2));throw error;}finally{await host.dispose();database.close();}
}
