import assert from 'node:assert/strict';
import {readFileSync,writeFileSync} from 'node:fs';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {randomUUID} from 'node:crypto';
export async function verifyResourceSteal({binary,hostPath,configs,output,scenario}) {
 const {TwoMemberDatabase,TwoMemberHost,newRecord,readCatalog}=await import(pathToFileURL(hostPath).href);
 const four=scenario==='steal-four',manifest=join(configs,'catalog.json'),catalog=readCatalog(binary,manifest),accounts=JSON.parse(readFileSync(join(configs,four?'steal-accounts-a.json':'steal-accounts-b.json'),'utf8'));
 const record=newRecord({catalog,scenario,accounts,manifest}),database=new TwoMemberDatabase(join(output,scenario+'-'+randomUUID()+'.sqlite'));database.create(record);
 const facts=[],projections=[],host=new TwoMemberHost({binary,manifest,database,record,autoAck:true,onEvent(recipient,event){facts.push({recipient,event:event.kind==='private'?event.event:event});}}),actor=host.members[0],own=()=>actor.snapshot.self;
 const events=kind=>facts.filter(r=>r.recipient==='P00'&&r.event.kind===kind).map(r=>r.event),rows=id=>events('resource_stolen').filter(e=>e.action_id===id);
 const begin=async()=>{await host.until(()=>!own().active_channel&&!own().pending_action&&actor.snapshot.logical_ms>=own().recovery_until_ms&&own().player_skills[0].ready_at_ms<=actor.snapshot.logical_ms);const t=actor.snapshot.monsters[0].entity;await host.input('P00',{kind:'cast_unit',skill_id:'resource-channel',target:{id:t.id,generation:t.generation}});await host.until(()=>!!own().active_channel);projections.push(structuredClone(own()));return own().active_channel.action_id;};
 const noLate=async(id)=>{const peer=host.members[1],mark=peer.snapshot.logical_ms;await host.until(()=>peer.snapshot.logical_ms>=mark+1350);assert.equal(rows(id).length,1);};
 try {
  await host.start();await host.joinAndReady();await host.until(()=>own().hp===960);const beforeSpirit=own().spirit;
  const first=await begin();await host.until(()=>rows(first).length===3&&!own().active_channel);const complete=rows(first);assert.deepEqual(complete.map(e=>e.requested_hp),four?[33,33,34]:[50,50,50]);assert.deepEqual(complete.map(e=>e.requested_spirit),four?[4,4,4]:[4,4,5]);
  const removedHp=complete.reduce((n,e)=>n+e.hp_removed,0),removedSpirit=complete.reduce((n,e)=>n+e.spirit_removed,0);assert.equal(removedHp,four?100:91);assert.equal(removedSpirit,four?12:13);assert.equal(complete.reduce((n,e)=>n+e.hp_restored,0),40);assert.equal(complete.reduce((n,e)=>n+e.spirit_restored,0),6);assert.equal(own().hp,1000);assert.equal(own().spirit,beforeSpirit);assert.equal(actor.snapshot.monsters[0].hp,four?892:1405);
  for(const e of complete){assert.equal(e.adjusted_hp_return,e.hp_restored+e.hp_excess);assert.equal(e.spirit_removed,e.spirit_restored+e.spirit_excess);assert.equal(e.hp_policy,four?'direct_transfer':'damage');assert.equal(e.source.id,'P00');}
  const status=own().statuses.find(s=>s.definition_id==='devoured-power');assert.equal(status.attribute,'life_steal');assert.equal(status.modifier_bps,four?1000:1500);assert.equal(status.stacks,1);
  const cancelled=await begin();await host.until(()=>rows(cancelled).length===1);await host.input('P00',{kind:'cancel_action'});await host.until(()=>!own().active_channel);await noLate(cancelled);
  const reconnected=await begin();await host.until(()=>rows(reconnected).length===1);const ready=own().player_skills[0].ready_at_ms;await host.reconnect('P00');await host.until(()=>!own().ready);await host.input('P00',{kind:'foreground'});await host.input('P00',{kind:'combat_ready',publication_id:actor.readyPublication??actor.publication});await host.until(()=>own().ready);assert.ok(!own().active_channel);assert.equal(own().player_skills[0].ready_at_ms,ready);await noLate(reconnected);
  const left=await begin();await host.until(()=>rows(left).length===1);await host.input('P00',{kind:'leave'});await host.until(()=>own().phase!=='active');await noLate(left);assert.equal(events('status_stacked').filter(e=>e.status.definition_id==='devoured-power').length,0);
  assert.ok(!facts.some(r=>r.recipient!=='P00'&&r.event.kind==='resource_stolen'));assert.ok(!actor.snapshot.projectiles.some(p=>p.source.id==='P00'));
  writeFileSync(join(output,scenario+'.steal-facts.json'),JSON.stringify({projections,events:facts.filter(r=>['resource_stolen','heal_resolved','unit_channel_pulse','channel_ended','status_applied','status_stacked','action_committed'].includes(r.event.kind))},null,2)+'\n');
  for(const m of host.members.slice(1))await host.input(m.account.participant_id,{kind:'leave'});await host.until(()=>host.phase==='closed'&&host.pending()===0&&host.publicationsComplete());assert.ok(database.views(record.id).every(r=>r.admissions===1&&r.settlements===1));assert.equal(host.errors.length,0);
  return{scenario,members:host.members.length,hpPolicy:four?'direct_transfer':'damage',requestedHp:four?[33,33,34]:[50,50,50],requestedSpirit:four?[4,4,4]:[4,4,5],hpRemoved:removedHp,spiritRemoved:removedSpirit,hpRestored:40,spiritRestored:6,nthStatus:'life_steal',lifestealBps:four?1000:1500,prefixCounts:[1,1,1],cancelled:true,reconnected:true,sourceLeft:true,noReplay:true,privateRecipients:1,settled:host.members.length};
 }catch(error){writeFileSync(join(output,scenario+'.steal-error.json'),JSON.stringify({error:String(error),facts,snapshots:host.members.map(m=>m.snapshot),errors:host.errors},null,2));throw error;}finally{await host.dispose();database.close();}
}
