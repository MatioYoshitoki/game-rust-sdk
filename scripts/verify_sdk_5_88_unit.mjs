import assert from 'node:assert/strict';
import {readFileSync,writeFileSync} from 'node:fs';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {randomUUID} from 'node:crypto';
export async function verifyUnitEffects({binary,hostPath,configs,output,scenario}) {
 const {TwoMemberDatabase,TwoMemberHost,newRecord,readCatalog}=await import(pathToFileURL(hostPath).href);
 const four=scenario==='units-four',manifest=join(configs,'catalog.json'),catalog=readCatalog(binary,manifest),accounts=JSON.parse(readFileSync(join(configs,four?'units-accounts-a.json':'units-accounts-b.json'),'utf8'));
 const record=newRecord({catalog,scenario,accounts,manifest}),database=new TwoMemberDatabase(join(output,scenario+'-'+randomUUID()+'.sqlite'));database.create(record);
 const facts=[],projections=[],host=new TwoMemberHost({binary,manifest,database,record,autoAck:true,onEvent(recipient,event){facts.push({recipient,event:event.kind==='private'?event.event:event});}});
 const actor=host.members[0],peer=host.members[1],own=()=>actor.snapshot.self,events=kind=>facts.filter(r=>r.recipient==='P00'&&r.event.kind===kind).map(r=>r.event),pulses=id=>events('unit_channel_pulse').filter(e=>e.action_id===id),recover=id=>events('channel_recovery_pulse').filter(e=>e.action_id===id);
 const target=()=>actor.snapshot.monsters[0].entity,cast=async(id,entity)=>{await host.until(()=>!own().active_channel&&!own().pending_action&&actor.snapshot.logical_ms>=own().recovery_until_ms&&own().player_skills.find(s=>s.skill_id===id).ready_at_ms<=actor.snapshot.logical_ms);await host.input('P00',{kind:'cast_unit',skill_id:id,target:{id:entity.id,generation:entity.generation}});};
 const begin=async(id,entity)=>{await cast(id,entity);await host.until(()=>!!own().active_channel);projections.push(structuredClone(own()));return own().active_channel.action_id;};
 try {
  await host.start();await host.joinAndReady();await host.until(()=>peer.snapshot.self.hp<peer.snapshot.self.max_hp);
  const paid=own().spirit;await cast('unit-strike',target());await host.until(()=>events('action_committed').some(e=>e.skill_id==='unit-strike'));assert.ok(!actor.snapshot.projectiles.some(p=>p.source.id==='P00'));
  const enemy=await begin('enemy-channel',target());assert.equal(pulses(enemy).length,0);await host.until(()=>pulses(enemy).length===3&&!own().active_channel);assert.deepEqual(pulses(enemy).map(p=>p.raw_damage),four?[2,2,4]:[3,3,5]);
  const ally=await begin('ally-channel',peer.snapshot.self.participant.entity);assert.equal(recover(ally).length,0);await host.until(()=>recover(ally).length===3&&!own().active_channel);assert.deepEqual(recover(ally).map(p=>p.raw_hp),four?[2,2,4]:[3,3,5]);assert.equal(recover(ally).reduce((n,p)=>n+p.hp_restored,0),four?8:11);
  const heals=facts.filter(r=>r.event.kind==='heal_resolved'&&r.event.heal_kind==='ally_skill');assert.equal(heals.length,6);assert.deepEqual([...new Set(heals.map(r=>r.recipient))].sort(),['P00','P01']);assert.ok(heals.every(r=>r.event.source.id==='P00'&&r.event.target.id==='P01'));
  const grants=()=>[...events('status_applied'),...events('status_stacked')].filter(e=>e.status.definition_id==='channel-focus');assert.equal(grants().length,2);
  const stopped=await begin('enemy-channel',target());await host.until(()=>pulses(stopped).length===1);const ready=own().player_skills.find(s=>s.skill_id==='enemy-channel').ready_at_ms;await host.reconnect('P00');await host.until(()=>!own().ready);await host.input('P00',{kind:'foreground'});await host.input('P00',{kind:'combat_ready',publication_id:actor.readyPublication??actor.publication});await host.until(()=>own().ready);assert.ok(!own().active_channel);assert.equal(own().player_skills.find(s=>s.skill_id==='enemy-channel').ready_at_ms,ready);
  const lost=await begin('ally-channel',peer.snapshot.self.participant.entity);await host.until(()=>recover(lost).length===1);await host.input('P01',{kind:'leave'});await host.until(()=>!own().active_channel);const mark=actor.snapshot.logical_ms;await host.until(()=>actor.snapshot.logical_ms>=mark+1300);assert.equal(pulses(stopped).length,1);assert.equal(recover(lost).length,1);assert.equal(grants().length,2);assert.equal(own().spirit,paid-22);
  assert.ok(!facts.some(r=>r.recipient!=='P00'&&['unit_channel_pulse','channel_recovery_pulse'].includes(r.event.kind)));
  writeFileSync(join(output,scenario+'.unit-facts.json'),JSON.stringify({projections,events:facts.filter(r=>['unit_channel_pulse','channel_recovery_pulse','heal_resolved','channel_ended','status_applied','status_stacked','action_committed'].includes(r.event.kind))},null,2)+'\n');
  for(const m of host.members.filter(m=>m!==peer))await host.input(m.account.participant_id,{kind:'leave'});await host.until(()=>host.phase==='closed'&&host.pending()===0&&host.publicationsComplete());const ledgers=database.views(record.id);assert.ok(ledgers.every(r=>r.admissions===1&&r.settlements===1));assert.equal(host.errors.length,0);
  return {scenario,members:host.members.length,noProjectile:true,damageShares:four?[2,2,4]:[3,3,5],healShares:four?[2,2,4]:[3,3,5],restoredHp:four?8:11,fullOnceGates:2,healRecipients:2,planRecipients:1,reconnectStopped:true,targetExitStopped:true,paidSpirit:22,settled:host.members.length};
 }catch(error){writeFileSync(join(output,scenario+'.unit-error.json'),JSON.stringify({error:String(error),facts,snapshots:host.members.map(m=>m.snapshot),errors:host.errors},null,2));throw error;}finally{await host.dispose();database.close();}
}
