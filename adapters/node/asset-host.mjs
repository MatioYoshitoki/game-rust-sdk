// Ordinary-clock protocol 2/3 consumer. Explicit files; in-memory reference account only.
import { spawn, execFileSync } from 'node:child_process';
import { createInterface } from 'node:readline';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { randomUUID } from 'node:crypto';
import assert from 'node:assert/strict';
import { AssetStore } from './asset-store.mjs';

const flag=name=>{const i=process.argv.indexOf(name);if(i<0)throw new Error(`Required ${name}`);return process.argv[i+1];};
const binary=resolve(flag('--binary')), manifest=resolve(flag('--configs'));
const account=JSON.parse(readFileSync(flag('--account'),'utf8'));
const mode=process.argv.includes('--extraction')?'extraction':'death';
const catalog=JSON.parse(execFileSync(binary,['--configs',manifest,'--catalog'],{encoding:'utf8',maxBuffer:134217728}));
const active=catalog.active.find(v=>v.scenario===catalog.defaultScenario);
const config=catalog.versions.find(v=>v.id===active.id&&v.hash===active.hash).config;
assert.ok(config.schema_version===4 && config.pve && config.mechanism_version>=19,'This scripted consumer requires a protocol-3 PVE status fixture');
const store=new AssetStore(account.stats,{config,account});
const child=spawn(binary,['--configs',manifest],{stdio:['pipe','pipe','inherit']});
const lines=createInterface({input:child.stdout}), pending=[];
function fail(error){for(const p of pending.splice(0)){clearTimeout(p.timer);p.reject(error);}}
child.on('error',fail);child.on('exit',code=>fail(new Error(`Kernel exit ${code}`)));
lines.on('line',line=>{const p=pending.shift();if(!p)return;clearTimeout(p.timer);try{const r=JSON.parse(line);if(r.error)p.reject(new Error(r.error));else p.resolve(r.ok);}catch(error){p.reject(error);}});
function rpc(value){return new Promise((resolve,reject)=>{const timer=setTimeout(()=>{child.kill();fail(new Error('RPC deadline'));},5000);pending.push({resolve,reject,timer});child.stdin.write(JSON.stringify(value)+'\n');});}
const key={id:randomUUID(),incarnation:1},connection={id:'node-reference',generation:1};
let sequence=0,snapshot,ready=false,settlement,closed=false,applied=0;
const committedSkills=[],committedTargets=[],committedGroundPoints=[],fieldTicks=[],fieldEnds=[],fieldStarts=[],piercingStarts=[],piercingHits=[],piercingEnds=[];
const input=action=>rpc({type:'submit_input',request:{key,participant_id:account.participant_id,entity_generation:1,connection,sequence:++sequence,request_id:`node-${sequence}`,action}});
async function pump(){
  const {outputs}=await rpc({type:'drain_outputs'});
  for(const output of outputs){const body=output.body;
    if(body.kind==='business_request'){
      const r=body.request,outcome=store.execute(r);
      await rpc({type:'submit_business_receipt',request:{key,operation_id:r.operation_id,attempt_id:r.attempt_id,outcome}});
    }else if(body.kind==='publication'){
      snapshot=body.snapshot;
      try{await rpc({type:'submit_transport_receipt',request:{key,output_id:output.id,recipient_id:account.participant_id,connection,outcome:'sent'}});}catch(e){if(e.message!=='CLOSED')throw e;}
      if(!ready&&snapshot.phase==='deploying'){await input({kind:'combat_ready',publication_id:output.id});ready=true;}
    }else{
      if(body.event.kind==='status_applied')applied++;
      if(body.event.kind==='piercing_started')piercingStarts.push(body.event.projectile);
      if(body.event.kind==='piercing_hit')piercingHits.push(body.event);
      if(body.event.kind==='piercing_ended')piercingEnds.push(body.event);
      if(body.event.kind==='player_field_started')fieldStarts.push(body.event.field);
      if(body.event.kind==='player_field_tick')fieldTicks.push(body.event);
      if(body.event.kind==='player_field_ended')fieldEnds.push(body.event);
      if(body.event.kind==='action_committed' && body.event.ground_position)committedGroundPoints.push(body.event.ground_position);
      if(body.event.kind==='action_committed' && body.event.unit_target)committedTargets.push(body.event.unit_target);
      if(body.event.kind==='action_committed' && body.event.skill_id)committedSkills.push(body.event.skill_id);
      if(['asset_settlement_applied','result_settlement_applied'].includes(body.event.kind))settlement=body.event;
      if(body.event.kind==='closed')closed=true;
    }
  }
}
async function until(predicate){const deadline=Date.now()+30000;while(!predicate()){assert.ok(Date.now()<deadline,'Consumer stage deadline');await pump();await new Promise(r=>setTimeout(r,20));}}
try{
  await rpc({type:'create',request:{protocol_version:config.schema_version===4?3:2,key,request_id:'create',config_id:active.id,config_hash:active.hash,seed:account.seed,participant_id:account.participant_id,authenticated_principal:account.principal,connection}});
  await until(()=>store.admissions===1);await input({kind:'join'});await until(()=>snapshot?.phase==='active');
  const skill=config.skill;
  const skills=[skill,...(config.additional_skills??[])];
  if(snapshot.pve){
    let next=0;
    await until(()=>snapshot.pve.monsters.every(m=>!m.hp)||snapshot.logical_ms>=next);
    const fightDeadline=Date.now()+30000;
    while(snapshot.pve.monsters.some(m=>m.hp)){
      assert.ok(Date.now()<fightDeadline,'Scripted fight deadline');
      const target=snapshot.pve.monsters.find(m=>m.hp);
      if(config.mechanism_version>=26){
        const selected=skills.find(s=>snapshot.spirit>=s.cost && snapshot.logical_ms >= (snapshot.player_skills?.find(v=>v.skill_id===s.id)?.ready_at_ms??0));
        if(selected && (selected.mechanism!=='unit_projectile'||target.unit_target) && !snapshot.pending_action && snapshot.logical_ms>=snapshot.recovery_until_ms && snapshot.logical_ms>=next){
          const accepted=await input(['ground_circle','ground_field'].includes(selected.mechanism) ? {kind:'cast_ground',skill_id:selected.id,position:[...target.position]} : selected.mechanism==='self_status' ? {kind:'cast_self',skill_id:selected.id} : selected.mechanism==='unit_projectile' ? {kind:'cast_unit',skill_id:selected.id,target:target.unit_target} : {kind:'cast',skill_id:selected.id,direction:target.position.map((v,i)=>v-snapshot.position[i])});
          // Wait for a publication after this accepted input, even when stdout carried older snapshots.
          next=accepted.status.logical_ms+config.params.runtime.publish_batched_ms;
        }
      }else if(snapshot.logical_ms>=Math.max(next,snapshot.skill_ready_at_ms,snapshot.recovery_until_ms)){
        await input({kind:'cast',skill_id:skill.id,direction:target.position.map((v,i)=>v-snapshot.position[i])});
        next=snapshot.logical_ms+skill.windup_ms+skill.cooldown_ms+300;
      }
      await pump();await new Promise(r=>setTimeout(r,40));
    }
    for(const resource of [...snapshot.resources]){
      await input({kind:'navigate',destination:resource.position});await until(()=>Math.hypot(...snapshot.position.map((v,i)=>v-resource.position[i]))<10);
      await input({kind:'stop'});await input({kind:'pickup',resource_id:resource.id,expected_revision:snapshot.inventory_revision});await pump();
    }
  }else{
    await input({kind:'cast',skill_id:skill.id,direction:[1,0]});await until(()=>!snapshot.target);
  }
  if(mode==='extraction'){
    if(snapshot.pve){await input({kind:'navigate',destination:config.extraction_point});await until(()=>Math.hypot(...snapshot.position.map((v,i)=>v-config.extraction_point[i]))<10);await input({kind:'stop'});}
    await until(()=>snapshot.extraction_open);await input({kind:'extract_start'});
  }else await input({kind:'leave'});
  await until(()=>closed&&settlement);await pump();
  assert.equal(store.admissions,1);assert.equal(store.settlements,1);
  console.log(JSON.stringify({clock:'ordinary',storage:'in-memory-reference',mode,statusApplications:applied,committedSkills,committedTargets,committedGroundPoints,piercingStarts,piercingHits,piercingEnds,fieldTicks,fieldEnds,fieldStarts,balances:store.balances,admissions:store.admissions,settlements:store.settlements,settlement}));
}finally{
  child.stdin.end();if(child.exitCode===null&&child.signalCode===null)await new Promise(r=>child.once('exit',r));lines.close();
}
