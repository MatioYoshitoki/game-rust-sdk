import assert from 'node:assert/strict';
import {readFileSync,writeFileSync} from 'node:fs';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {randomUUID} from 'node:crypto';
export async function verifyCharacterPower({binary,hostPath,configs,output,scenario}){
 const {TwoMemberDatabase,TwoMemberHost,newRecord,readCatalog}=await import(pathToFileURL(hostPath).href),four=scenario==='power-four',manifest=join(configs,'catalog.json'),catalog=readCatalog(binary,manifest),accounts=JSON.parse(readFileSync(join(configs,four?'power-accounts-a.json':'power-accounts-b.json'),'utf8'));
 const record=newRecord({catalog,scenario,accounts,manifest}),database=new TwoMemberDatabase(join(output,scenario+'-'+randomUUID()+'.sqlite'));database.create(record);
 const facts=[],projections=[],host=new TwoMemberHost({binary,manifest,database,record,autoAck:true,onEvent(recipient,event){facts.push({recipient,event:event.kind==='private'?event.event:event});}}),actor=host.members[0],peer=host.members[1],own=()=>actor.snapshot.self,expected=four?99:162,boosted=four?108:189,other=four?112:202,basic=four?150:200;
 const events=(kind,pid='P00')=>facts.filter(r=>r.recipient===pid&&r.event.kind===kind).map(r=>r.event),damage=pid=>facts.filter(r=>r.event.kind==='damage'&&r.event.source.id===pid).map(r=>r.event.amount);
 const cast=async(pid,id,target)=>{const m=host.member(pid);await host.until(()=>!m.snapshot.self.pending_action&&m.snapshot.logical_ms>=m.snapshot.self.recovery_until_ms&&m.snapshot.self.player_skills.find(s=>s.skill_id===id).ready_at_ms<=m.snapshot.logical_ms);const before=events('action_committed',pid).length;await host.input(pid,target?{kind:'cast_unit',skill_id:id,target:{id:target.id,generation:target.generation}}:{kind:'cast_self',skill_id:id});await host.until(()=>events('action_committed',pid).length===before+1&&!m.snapshot.self.pending_action);};
 try{
  await host.start();await host.joinAndReady();await host.until(()=>own().hp===own().max_hp-40&&peer.snapshot.self.hp===peer.snapshot.self.max_hp-40);
  assert.equal(own().max_hp,four?1000:1250);assert.equal(own().effective_atk,basic);assert.equal(own().skill_powers['growth-strike'],expected);assert.equal(peer.snapshot.self.skill_powers['growth-strike'],other);projections.push(structuredClone(own()));
  await cast('P00','growth-ward');const shield=own().statuses.find(s=>s.definition_id==='power-record');assert.equal(shield.shield.record.cap,expected);
  await host.until(()=>actor.snapshot.logical_ms>=own().recovery_until_ms);
  await host.input('P00',{kind:'attack_unit',target:{id:actor.snapshot.monsters[0].entity.id,generation:1}});await host.until(()=>damage('P00').length===1);assert.equal(damage('P00')[0],basic);
  await cast('P00','growth-strike',actor.snapshot.monsters[0].entity);await host.until(()=>damage('P00').length===2);assert.equal(damage('P00')[1],expected);
  await cast('P01','growth-strike',peer.snapshot.monsters[1].entity);await host.until(()=>damage('P01').length===1);assert.deepEqual(damage('P01'),[other]);
  await cast('P00','growth-heal',peer.snapshot.self.participant.entity);await host.until(()=>peer.snapshot.self.hp===peer.snapshot.self.max_hp);const heal=events('heal_resolved').find(e=>e.heal_kind==='ally_skill');assert.equal(heal.raw_heal,expected);assert.equal(heal.hp_restored,40);
  await cast('P00','growth-regrowth',peer.snapshot.self.participant.entity);
  await cast('P00','power-focus');await host.until(()=>own().skill_powers['growth-strike']===boosted);projections.push(structuredClone(own()));assert.equal(peer.snapshot.self.skill_powers['growth-strike'],other);assert.equal(own().statuses.find(s=>s.definition_id==='power-record').shield.record.cap,expected);
  await cast('P00','growth-strike',actor.snapshot.monsters[0].entity);await host.until(()=>damage('P00').length===3);assert.equal(damage('P00')[2],boosted);
  await host.until(()=>events('periodic_heal_tick').filter(e=>e.source.id==='P00'&&e.target.id==='P01').length===4&&!own().statuses.some(s=>s.definition_id==='power-focus'));assert.equal(own().skill_powers['growth-strike'],expected);projections.push(structuredClone(own()));
  const hot=events('periodic_heal_tick').filter(e=>e.source.id==='P00'&&e.target.id==='P01');assert.deepEqual(hot.map(e=>e.raw_heal),four?[99,99,99,99]:[189,189,162,162]);
  const ready=own().player_skills.find(s=>s.skill_id==='growth-strike').ready_at_ms;await host.reconnect('P00');await host.until(()=>!own().ready);await host.input('P00',{kind:'foreground'});await host.input('P00',{kind:'combat_ready',publication_id:actor.readyPublication??actor.publication});await host.until(()=>own().ready);assert.equal(own().skill_powers['growth-strike'],expected);assert.equal(own().player_skills.find(s=>s.skill_id==='growth-strike').ready_at_ms,ready);
  assert.ok(host.members.every(m=>m.snapshot.members.every(v=>!Object.hasOwn(v,'skill_powers')&&!Object.hasOwn(v,'combat_build'))));assert.ok(projections.every(v=>!JSON.stringify(v).includes('model_id')&&!JSON.stringify(v).includes('power_points')));
  writeFileSync(join(output,scenario+'.power-facts.json'),JSON.stringify({projections,shield,events:facts.filter(r=>['damage','heal_resolved','periodic_heal_tick','status_applied','action_committed'].includes(r.event.kind))},null,2)+'\n');
  for(const m of host.members)await host.input(m.account.participant_id,{kind:'leave'});await host.until(()=>host.phase==='closed'&&host.pending()===0&&host.publicationsComplete());assert.ok(database.views(record.id).every(r=>r.admissions===1&&r.settlements===1));assert.equal(host.errors.length,0);
  return{scenario,members:host.members.length,basic,skill:expected,boosted,peerSkill:other,recordCap:expected,healRaw:expected,healActual:40,hotRaw:hot.map(e=>e.raw_heal),sampling:four?'source_snapshot':'source_live',derivedMaxHp:four?1000:1250,rawGrowthPrivate:true,memberIsolation:true,reconnected:true,expired:true,settled:host.members.length};
 }catch(error){writeFileSync(join(output,scenario+'.power-error.json'),JSON.stringify({error:String(error),facts,snapshots:host.members.map(m=>m.snapshot),errors:host.errors},null,2));throw error;}finally{await host.dispose();database.close();}
}
