import test from 'node:test';
import assert from 'node:assert/strict';
import {ReplicaV6,envelopeFromOutput,SpectatorReplicaV6,spectatorEnvelopeFromOutput} from '../../web/replication.mjs';
const key={id:'match',incarnation:1},entity={kind:'participant',id:'A',generation:1};
function full(){const snapshot={phase:'active',logical_ms:0,active_at_ms:0,self:{participant:{entity,owner_id:'A'},position:[10,20],hp:100,max_hp:100,spirit:50,max_spirit:50,phase:'active',protected:false,foreground:true,ready:true,effective_atk:10,effective_def:10,statuses:[],inventory:[{item_id:'ore',quantity:1}],inventory_revision:1,player_skills:[],pending_action:null,basic_ready_at_ms:0,recovery_until_ms:0,extraction_finishes_ms:null},members:[{entity,position:[10,20],hp:100,max_hp:100,phase:'active',protected:false}],monsters:[],resources:[{id:'r1',position:[10,20],quantity:1,item_id:'ore',owner:null,expires_at_ms:1000}],projectiles:[],extraction_point:[30,40],extraction_open:false};return {key,publication_id:'full-1',recipient_id:'A',connection:{id:'c',generation:1},stream_epoch:1,revision:1,payload:{kind:'full',reason:'initial',snapshot}};}
const delta=()=>({...full(),publication_id:'delta-2',revision:2,payload:{kind:'delta',base_publication_id:'full-1',base_revision:1,delta:{logical_ms:30}}});
function fixture(){const f=full(),r=new ReplicaV6(key,'A');r.apply(f);return {r,f,before:r.snapshot};}
const code=name=>e=>e.code===name;
test('extraction pressure has a fixed clock and terminal views preserve their exit phase', () => {
  const initial=full(), s=initial.payload.snapshot;
  const peer={kind:'participant',id:'B',generation:1};
  const pressure={grace_at_ms:60,ends_at_ms:120,phase:'exploring',warning:{id:'warning',index:1}};
  s.extraction_open=true;
  s.extraction_pressure=pressure;
  s.members.push({...s.members[0],entity:peer,extraction_channel:{position:[10,20],started_at_ms:0,ends_at_ms:90}});
  const replica=new ReplicaV6(key,'A');
  replica.apply(initial);
  const patch=delta();
  patch.payload.delta.state={phase:'active',active_at_ms:0,extraction_point:s.extraction_point,extraction_open:true,
    extraction_pressure:{...pressure,ends_at_ms:150}};
  assert.throws(()=>replica.apply(patch),code('INVALID_FRAME'));
  patch.payload.delta.state.extraction_pressure={...pressure,warning:{id:'changed',index:1}};
  assert.throws(()=>replica.apply(patch),code('INVALID_FRAME'));
  patch.payload.delta.state.extraction_pressure=pressure;
  assert.equal(replica.apply(patch).members[1].extraction_channel.ends_at_ms,90);
  const terminal=structuredClone(initial);
  terminal.publication_id='terminal-pressure';
  terminal.revision=3;
  terminal.payload.snapshot.logical_ms=120;
  delete terminal.payload.snapshot.members[1].extraction_channel;
  assert.throws(()=>replica.apply(terminal),code('INVALID_FRAME'));
  terminal.payload.snapshot.self.phase='terminal';
  terminal.payload.snapshot.members[0].phase='terminal';
  assert.equal(replica.apply(terminal).extraction_pressure.phase,'exploring');
});

test('spectator extraction events reject completion before the channel deadline', () => {
  const frame={key,slot:1,publication_id:'channel',connection:{id:'viewer',generation:1},stream_epoch:1,revision:1,
    payload:{kind:'full',reason:'initial',snapshot:{phase:'active',active_at_ms:0,logical_ms:60,
      members:[{entity,position:[10,20],hp:100,max_hp:100,phase:'active'}],
      monsters:[],projectiles:[],resources:[],areas:[],
      events:[{sequence:1,logical_ms:30,event:{kind:'extraction_channel_changed',entity,state:'completed',
        channel:{position:[10,20],started_at_ms:0,ends_at_ms:60}}}]}}};
  const replica=new SpectatorReplicaV6(key,1);
  assert.throws(()=>replica.apply(frame),code('INVALID_FRAME'));
  frame.payload.snapshot.events[0].logical_ms=60;
  assert.equal(replica.apply(frame).events[0].event.state,'completed');
});

test('encounter and quota deltas reject progress rollback and surge extension', () => {
  const first=full(), view=first.payload.snapshot;
  view.encounter_progress={initial:10,remaining:6,stats_bps:12500,surge_started:false,surge_ends_at_ms:null};
  view.self.mode={stage_id:'guard',stage_index:1,objective:'kill_quota',opens_at_ms:0,expires_at_ms:180,
    progress:{current:1,required:3},submitted:false,respawns_left:0,respawn_at_ms:null,outcome:null};
  const replica=new ReplicaV6(key,'A');
  replica.apply(first);
  const d=delta();
  d.payload.delta.state={phase:'active',active_at_ms:0,extraction_point:view.extraction_point,extraction_open:false,
    encounter_progress:{...view.encounter_progress,remaining:0,stats_bps:17500,surge_started:true,surge_ends_at_ms:150}};
  d.payload.delta.own=structuredClone(view.self);
  d.payload.delta.own.mode.progress.current=0;
  assert.throws(()=>replica.apply(d),code('INVALID_FRAME'));
  d.payload.delta.own.mode.progress.current=2;
  assert.equal(replica.apply(d).encounter_progress.surge_ends_at_ms,150);
  const next=structuredClone(d);
  next.publication_id='delta-3';
  next.revision=3;
  next.payload.base_publication_id=d.publication_id;
  next.payload.base_revision=d.revision;
  next.payload.delta.logical_ms=60;
  next.payload.delta.state.encounter_progress.surge_ends_at_ms=180;
  assert.throws(()=>replica.apply(next),code('INVALID_FRAME'));
  Object.assign(next.payload.delta.state.encounter_progress,{surge_ends_at_ms:null,stats_bps:15000});
  assert.throws(()=>replica.apply(next),code('INVALID_FRAME'));
  next.payload.delta.logical_ms=150;
  assert.equal(replica.apply(next).encounter_progress.surge_ends_at_ms,null);
});
test('training metrics and respawn are private and reject inconsistent frames', () => {
  const frame=full(), snapshot=frame.payload.snapshot, replica=new ReplicaV6(key,'A');
  const metrics={sampled_at_ms:0,damage:12,hits:1,critical_hits:1,highest_hit:12,rolling_dps:4,average_dps:48,
    by_source:{basic:0,skill:12,periodic:0,summon:0,other:0}};
  snapshot.self.training={respawn_at_ms:null,metrics};
  replica.apply(frame);
  const patch=delta();
  const own=structuredClone(snapshot.self);
  own.hp=0;
  own.training.respawn_at_ms=90;
  patch.payload.delta.own=own;
  patch.payload.delta.members={upsert:[{...snapshot.members[0],hp:0}],removed:[],order:[entity]};
  for(const bad of [
    {...own.training,respawn_at_ms:30},
    {...own.training,metrics:{...metrics,critical_hits:2}},
    {...own.training,metrics:{...metrics,by_source:{...metrics.by_source,skill:13}}},
  ]){
    const malformed=structuredClone(patch);
    malformed.payload.delta.own.training=bad;
    assert.throws(()=>replica.apply(malformed),code('INVALID_FRAME'));
  }
  assert.equal(replica.apply(patch).self.training.respawn_at_ms,90);
  const replacement=structuredClone(frame);
  Object.assign(replacement,{publication_id:'respawned',stream_epoch:2,revision:3,connection:{id:'new',generation:2}});
  replacement.payload.snapshot.logical_ms=90;
  replacement.payload.snapshot.self.participant.entity.generation=2;
  assert.equal(replica.apply(replacement).self.training.metrics.damage,12);
  const bad=structuredClone(replacement);
  Object.assign(bad,{publication_id:'without-private-training',revision:4});
  delete bad.payload.snapshot.self.training;
  assert.throws(()=>replica.apply(bad),code('INVALID_FRAME'));
});
test('spectator decoder accepts public deltas and rejects private rows or the old connection', () => {
  const snapshot={phase:'active',active_at_ms:0,logical_ms:0,members:[{entity,position:[10,20],hp:100,max_hp:100,phase:'active'}],
    monsters:[],projectiles:[],resources:[{id:'public',position:[20,20],expires_at_ms:300}],areas:[],events:[]};
  const first={key,slot:1,publication_id:'spectator-full',connection:{id:'viewer',generation:1},stream_epoch:1,revision:1,
    payload:{kind:'full',reason:'initial',snapshot}};
  const replica=new SpectatorReplicaV6(key,1);
  replica.apply(first);
  assert.deepEqual(spectatorEnvelopeFromOutput({key,id:first.publication_id,body:{kind:'spectator_publication_v6',
    slot:1,connection:first.connection,stream_epoch:1,revision:1,payload:first.payload}}),first);
  const patch={...first,publication_id:'spectator-delta',revision:2,payload:{kind:'delta',base_publication_id:first.publication_id,
    base_revision:1,delta:{logical_ms:30}}};
  for(const extra of [
    {own:{inventory:[]}},
    {resources:{upsert:[{...snapshot.resources[0],item_id:'hidden'}],removed:[],order:['public']}},
    {events:[{sequence:2,logical_ms:30,event:{kind:'private',event:{kind:'item_used'}}}]},
  ]){
    const bad=structuredClone(patch);
    Object.assign(bad.payload.delta,extra);
    assert.throws(()=>replica.apply(bad),code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot,snapshot);
  }
  assert.equal(replica.apply(patch).logical_ms,30);
  const replacement={...first,connection:{id:'viewer-new',generation:2},stream_epoch:2,publication_id:'spectator-rejoin',revision:3,
    payload:{...first.payload,reason:'lifecycle',snapshot:{...snapshot,logical_ms:60}}};
  assert.equal(replica.apply(replacement).logical_ms,60);
  assert.throws(()=>replica.apply(patch),code('STALE_FRAME'));
  assert.throws(()=>new SpectatorReplicaV6(key,2).apply(replacement),code('WRONG_RECIPIENT'));
  assert.throws(()=>new ReplicaV6(key,'A').apply(replacement),code('INVALID_FRAME'));
});
test('mode transitions preserve stage clocks and freeze the recipient result', () => {
  const f=full(), r=new ReplicaV6(key,'A');
  const mode={stage_id:'gate',stage_index:1,objective:'checkpoint',opens_at_ms:0,expires_at_ms:90,
    submitted:true,respawns_left:1,respawn_at_ms:null,outcome:null};
  f.payload.snapshot.self.mode=mode;
  r.apply(f);
  const d=delta();
  const next=structuredClone(f.payload.snapshot.self);
  d.payload.delta.own=next;
  for(const change of [{expires_at_ms:120},{submitted:false},{respawns_left:2},{outcome:'win'}]){
    next.mode={...mode,...change};
    assert.throws(()=>r.apply(d),code('INVALID_FRAME'));
    assert.deepEqual(r.snapshot.self.mode,mode);
  }
  next.mode={...mode,stage_id:'round',stage_index:2,objective:'contest',opens_at_ms:30,expires_at_ms:120,submitted:false};
  assert.equal(r.apply(d).self.mode.objective,'contest');
  const terminal=structuredClone(f);
  Object.assign(terminal,{revision:3,publication_id:'mode-result'});
  terminal.payload.snapshot.logical_ms=120;
  Object.assign(terminal.payload.snapshot.self,{phase:'terminal',mode:{...next.mode,outcome:'draw'}});
  assert.equal(r.apply(terminal).self.mode.outcome,'draw');
  const altered=structuredClone(terminal);
  Object.assign(altered,{revision:4,publication_id:'altered-result'});
  altered.payload.snapshot.self.mode.outcome='loss';
  assert.throws(()=>r.apply(altered),code('INVALID_FRAME'));
});
test('member conflict marker permits generation-bound targets and stays frozen across baselines',()=>{
  const frame=full(),snapshot=frame.payload.snapshot;
  const other={kind:'participant',id:'B',generation:1};
  snapshot.members.push({...snapshot.members[0],entity:other});
  snapshot.self.player_skills=[{skill_id:'strike',ready_at_ms:0}];
  snapshot.self.pending_action={skill_id:'strike',commits_at_ms:30,unit_target:{id:'B',generation:1}};
  assert.throws(()=>new ReplicaV6(key,'A').apply(frame),code('INVALID_FRAME'));
  snapshot.member_conflict=true;
  const wrong=structuredClone(frame);
  wrong.payload.snapshot.self.pending_action.unit_target.generation=2;
  assert.throws(()=>new ReplicaV6(key,'A').apply(wrong),code('INVALID_FRAME'));
  const r=new ReplicaV6(key,'A');
  r.apply(frame);
  const patch=delta();
  patch.payload.delta.own={...snapshot.self,pending_action:null};
  assert.equal(r.apply(patch).member_conflict,true);
  r.forgetBaseline();
  const replacement={...full(),publication_id:'conflict-rejoin',stream_epoch:2,revision:3};
  assert.throws(()=>r.apply(replacement),code('INVALID_FRAME'));
  replacement.payload.snapshot.member_conflict=true;
  assert.equal(r.apply(replacement).member_conflict,true);
});
test('member and spectator waves accept an opening and reject a rolled-back wave',()=>{
  const wave={index:1,total:2,opens_at_ms:30,opened:false};
  const frame=full(), snapshot=frame.payload.snapshot;
  snapshot.self.mode={stage_id:'course',stage_index:1,objective:'waves',opens_at_ms:0,expires_at_ms:180,
    submitted:false,respawns_left:0,respawn_at_ms:null,outcome:null,wave};
  const r=new ReplicaV6(key,'A');
  r.apply(frame);
  const d=delta();
  const own=structuredClone(snapshot.self);
  d.payload.delta.own=own;
  own.mode.wave={...wave,opened:true};
  assert.equal(r.apply(d).self.mode.wave.opened,true);
  const next={...d,revision:3,publication_id:'wave-next',payload:{
    kind:'delta',base_publication_id:'delta-2',base_revision:2,
    delta:{logical_ms:60,own:structuredClone(own)},
  }};
  next.payload.delta.own.mode.wave.opened=false;
  assert.throws(()=>r.apply(next),code('INVALID_FRAME'));
  next.payload.delta.own.mode.wave={...wave,index:2,opens_at_ms:90};
  assert.equal(r.apply(next).self.mode.wave.index,2);

  const stage={id:'course',index:1,objective:'waves',opens_at_ms:0,expires_at_ms:180,wave};
  const publicFrame={key,slot:1,publication_id:'wave-full',connection:{id:'viewer',generation:1},
    stream_epoch:1,revision:1,payload:{kind:'full',reason:'initial',snapshot:{
      phase:'active',active_at_ms:0,logical_ms:0,members:snapshot.members.map(({protected:_,...m})=>m),
      monsters:[],projectiles:[],resources:[],areas:[],events:[],stage,
    }}};
  const observer=new SpectatorReplicaV6(key,1);
  observer.apply(publicFrame);
  const p={...publicFrame,publication_id:'wave-public-next',revision:2,payload:{kind:'delta',
    base_publication_id:'wave-full',base_revision:1,delta:{logical_ms:30,stage:{...stage,wave:{...wave,opened:true}}}}};
  assert.equal(observer.apply(p).stage.wave.opened,true);
  const bad={...p,publication_id:'wave-rollback',revision:3,payload:{kind:'delta',
    base_publication_id:'wave-public-next',base_revision:2,delta:{logical_ms:60,stage}}};
  assert.throws(()=>observer.apply(bad),code('INVALID_FRAME'));
});
test('boon reservations are private and invalid quota deltas leave the prior view intact', () => {
  const frame = full();
  const own = frame.payload.snapshot.self;
  own.boons = {
    capacity: 1, revision: 1, slots: [],
    opportunities: [{id:'source',position:[10,20],offer:{kind:'grant',choices:['boon']},available:false,shared_remaining:2}],
    pending_claim: {opportunity_id:'source',boon_id:'boon',completes_at_ms:60},
  };
  const replica = new ReplicaV6(key, 'A');
  assert.equal(replica.apply(frame).self.boons.pending_claim.completes_at_ms, 60);
  const patch = delta();
  patch.payload.delta.own = structuredClone(own);
  const boons = patch.payload.delta.own.boons;
  delete boons.pending_claim;
  boons.opportunities[0].shared_remaining = 0;
  boons.opportunities[0].available = true;
  assert.throws(() => replica.apply(patch), code('INVALID_FRAME'));
  assert.equal(replica.snapshot.self.boons.opportunities[0].shared_remaining, 2);
  boons.opportunities[0].available = false;
  assert.deepEqual(replica.apply(patch).self.boons, boons);
});
test('world blocker publications replace the full active set and reject duplicate IDs', () => {
  const initial = full();
  initial.payload.snapshot.active_blockers = ['gate'];
  const replica = new ReplicaV6(key, 'A');
  assert.deepEqual(replica.apply(initial).active_blockers, ['gate']);
  const invalid = delta();
  invalid.payload.delta.active_blockers = ['gate', 'gate'];
  assert.throws(() => replica.apply(invalid), code('INVALID_FRAME'));
  const opened = delta();
  opened.payload.delta.active_blockers = [];
  assert.deepEqual(replica.apply(opened).active_blockers, []);
});
test('container views atomically replace the visible set on delta', () => {
  const f=full(),r=new ReplicaV6(key,'A');
  const container={id:'visible',generation:1,position:[20,20],state:'reserved'};
  f.payload.snapshot.containers=[container];
  r.apply(f);
  const d=delta();d.payload.delta.containers=[container,container];
  assert.throws(()=>r.apply(d),code('INVALID_FRAME'));
  assert.deepEqual(r.snapshot.containers,[container]);
  d.payload.delta.containers=[];
  assert.deepEqual(r.apply(d).containers,[]);
});
test('boss stages advance per entity generation and reject a rewind atomically', () => {
  const phase=(phase_index,epoch)=>({phase_index,epoch,phase_id:`p${phase_index}`,phase_name:`Phase ${phase_index}`,phase_count:3,attack_interval_ms:300,cast_chance:1});
  const boss={entity:{kind:'monster',id:'boss',generation:1},template_id:'boss',position:[20,20],hp:100,max_hp:100,radius:2,returning:false,target:null,boss_phase:phase(1,1)};
  const initial=full();
  initial.payload.snapshot.monsters=[boss];
  const replica=new ReplicaV6(key,'A');
  replica.apply(initial);
  const progressed=delta();
  progressed.payload.delta.monsters={upsert:[{...boss,hp:40,boss_phase:phase(3,1)}],removed:[],order:[boss.entity]};
  assert.equal(replica.apply(progressed).monsters[0].boss_phase.phase_index,3);
  const rewind=delta();
  rewind.publication_id='next';rewind.revision=3;
  Object.assign(rewind.payload,{base_publication_id:progressed.publication_id,base_revision:2});
  rewind.payload.delta.logical_ms=60;
  rewind.payload.delta.monsters={upsert:[boss],removed:[],order:[boss.entity]};
  assert.throws(()=>replica.apply(rewind),code('INVALID_FRAME'));
  assert.equal(replica.snapshot.monsters[0].hp,40);
  const reset={...boss,entity:{...boss.entity,generation:2},boss_phase:phase(1,2)};
  rewind.payload.delta.monsters={upsert:[reset],removed:[boss.entity],order:[reset.entity]};
  assert.deepEqual(replica.apply(rewind).monsters,[reset]);
});
test('delta decoder atomically replaces own and nullable state and removes the final resource',()=>{const {r}=fixture(),d=delta();d.payload.delta.own={...full().payload.snapshot.self,spirit:40};d.payload.delta.state={phase:'closed',active_at_ms:null,extraction_point:[30,40],extraction_open:false};d.payload.delta.resources={upsert:[],removed:['r1'],order:[]};const v=r.apply(d);assert.equal(v.self.spirit,40);assert.equal(v.active_at_ms,null);assert.deepEqual(v.resources,[]);assert.equal(v.logical_ms,30);});
test('delta with no initial full cannot construct a baseline',()=>{const r=new ReplicaV6(key,'A');assert.throws(()=>r.apply(delta()),code('BASELINE_MISSING'));assert.equal(r.snapshot,null);});
test('exact duplicate is idempotent and conflicting duplicate preserves the snapshot',()=>{const {r,f,before}=fixture();r.apply(structuredClone(f));const bad=structuredClone(f);bad.payload.snapshot.self.spirit=0;assert.throws(()=>r.apply(bad),code('CONFLICT'));assert.deepEqual(r.snapshot,before);});
test('wrong baseline id or revision cannot partially change own data',()=>{for(const field of ['base_publication_id','base_revision']){const {r,before}=fixture(),d=delta();d.payload[field]=field==='base_revision'?99:'missing';d.payload.delta.own={...full().payload.snapshot.self,spirit:0};assert.throws(()=>r.apply(d),code('BASELINE_MISSING'));assert.deepEqual(r.snapshot,before);}});
test('wrong match and wrong recipient never change the receiver cache',()=>{for(const field of ['key','recipient_id']){const {r,f,before}=fixture(),bad=structuredClone(f);bad[field]=field==='key'?{id:'other',incarnation:1}:'B';assert.throws(()=>r.apply(bad),code('WRONG_RECIPIENT'));assert.deepEqual(r.snapshot,before);}});
test('unknown and null patch fields are refused without partial application',()=>{for(const change of [{world_patch:{}},{own:null},{state:null}]){const {r,before}=fixture(),d=delta();Object.assign(d.payload.delta,change);assert.throws(()=>r.apply(d),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);}});
test('invalid removal and duplicate or incomplete order are atomic failures',()=>{for(const patch of [{upsert:[],removed:['missing'],order:[]},{upsert:[],removed:[],order:['r1','r1']},{upsert:[],removed:[],order:[]}]){const {r,before}=fixture(),d=delta();d.payload.delta.own={...before.self,spirit:0};d.payload.delta.resources=patch;assert.throws(()=>r.apply(d),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);}});
test('upsert and order faithfully reorder and update resource identity',()=>{const {r,before}=fixture(),d=delta();const r2={...before.resources[0],id:'r2'};d.payload.delta.resources={upsert:[r2,{...before.resources[0],quantity:2}],removed:[],order:['r2','r1']};assert.deepEqual(r.apply(d).resources.map(v=>[v.id,v.quantity]),[['r2',1],['r1',2]]);});
test('forgetting display cache retains fencing and recovers only with a newer full',()=>{const {r,f}=fixture();r.forgetBaseline();assert.throws(()=>r.apply(delta()),code('BASELINE_MISSING'));assert.throws(()=>r.apply(f),code('BASELINE_MISSING'));const next=structuredClone(f);next.publication_id='new-full';next.revision=4;next.stream_epoch=2;r.apply(next);assert.equal(r.lastFullId,'new-full');assert.equal(r.snapshot.self.hp,100);});
test('a new connection starts with full and old-connection frames cannot overwrite it',()=>{const {r,f}=fixture(),next=structuredClone(f);next.connection={id:'new-c',generation:2};next.stream_epoch=3;next.publication_id='full-new';next.revision=10;r.apply(next);const old=structuredClone(f);old.revision=11;old.publication_id='late-old';assert.throws(()=>r.apply(old),code('STALE_FRAME'));const d=delta();d.connection={id:'c3',generation:3};d.revision=12;assert.throws(()=>r.apply(d),code('BASELINE_MISSING'));assert.equal(r.revision,10);});
test('bad owner, nonfinite position, and out-of-range resource values are rejected',()=>{for(const mutate of [v=>v.self.participant.owner_id='B',v=>v.self.position=[NaN,0],v=>v.self.hp=2**32]){const r=new ReplicaV6(key,'A'),f=full();mutate(f.payload.snapshot);assert.throws(()=>r.apply(f),code('INVALID_FRAME'));assert.equal(r.snapshot,null);}});
test('raw output envelope contains exactly Rust payload and recipient metadata',()=>{const f=full();const o={key,id:f.publication_id,body:{kind:'publication_v6',recipient_id:f.recipient_id,connection:f.connection,stream_epoch:f.stream_epoch,revision:f.revision,payload:f.payload}};assert.deepEqual(envelopeFromOutput(o),f);const {r}=fixture();const materialized=r.snapshot;materialized.self.hp=0;assert.equal(r.snapshot.self.hp,100);});
const npc={kind:'monster',id:'npc',generation:1};
const summonEntity={kind:'monster',id:'summon',generation:1};
function summonFull(){
  const f=full(),snapshot=f.payload.snapshot;
  snapshot.monsters=[
    {entity:npc,template_id:'enemy',position:[20,20],hp:100,max_hp:100,radius:2,returning:false,target:summonEntity},
    {entity:summonEntity,template_id:'ally',position:[10,20],hp:20,max_hp:30,radius:2,returning:false,target:null,
     summon:{owner:entity,expires_at_ms:180},statuses:[{instance_id:'burn',definition_id:'burn',source:npc,target:summonEntity,
       applied_at_ms:0,expires_at_ms:120,modifier_bps:0,periodic:{kind:'physical_damage',interval_ms:30,next_tick_at_ms:30,ticks_done:0,total_ticks:3,raw_damage:10}}]},
  ];
  return f;
}
test('summon ownership and hostile status survive a full frame then clear through a delta',()=>{
  const replica=new ReplicaV6(key,'A');
  const snapshot=replica.apply(summonFull());
  assert.deepEqual(snapshot.monsters[1].summon.owner,entity);
  assert.deepEqual(snapshot.monsters[1].statuses[0].source,npc);
  const removal=delta();
  removal.payload.delta.monsters={upsert:[{...snapshot.monsters[0],target:null}],removed:[summonEntity],order:[npc]};
  assert.deepEqual(replica.apply(removal).monsters,[{...snapshot.monsters[0],target:null}]);
});
test('summon frames reject stale ownership, a dangling lock, and another owner status',()=>{
  const stale=s=>s.monsters[1].summon.owner={...entity,generation:2};
  const expired=s=>s.monsters[1].summon.expires_at_ms=s.logical_ms;
  const lock=s=>s.monsters.pop();
  const privateStatus=s=>{
    const other={...entity,id:'B'};
    s.members.push({...s.members[0],entity:other});
    s.monsters[1].summon.owner=other;
  };
  for(const mutate of [stale,expired,lock,privateStatus]){
    const frame=summonFull(),replica=new ReplicaV6(key,'A');
    mutate(frame.payload.snapshot);
    assert.throws(()=>replica.apply(frame),code('INVALID_FRAME'));
    assert.equal(replica.snapshot,null);
  }
});
const npcStatus=(source=entity)=>({instance_id:'status-1',definition_id:'wound',source,target:npc,applied_at_ms:0,expires_at_ms:2000,modifier_bps:0,periodic:{kind:'physical_damage',interval_ms:600,next_tick_at_ms:600,ticks_done:0,total_ticks:3,raw_damage:60}});
function periodicFull(){const f=full();f.payload.snapshot.monsters=[{entity:npc,template_id:'target',position:[20,20],hp:500,max_hp:500,radius:10,returning:false,target:entity,statuses:[npcStatus()]}];return f;}
test('multiplayer periodic projection supports own source and NPC self HOT through delta clear',()=>{const r=new ReplicaV6(key,'A'),f=periodicFull();r.apply(f);const d=delta(),n=structuredClone(f.payload.snapshot.monsters[0]);const hot=npcStatus(npc);hot.periodic={kind:'max_hp_heal',interval_ms:600,next_tick_at_ms:600,ticks_done:0,total_ticks:3,raw_heal:50};n.statuses=[hot];d.payload.delta.monsters={upsert:[n],removed:[],order:[npc]};assert.equal(r.apply(d).monsters[0].statuses[0].source.kind,'monster');const next=structuredClone(d);next.revision=3;next.publication_id='delta-3';next.payload.base_publication_id='delta-2';next.payload.base_revision=2;next.payload.delta.monsters.upsert[0].statuses=[];assert.deepEqual(r.apply(next).monsters[0].statuses,[]);});
test('multiplayer periodic projection rejects another member source atomically',()=>{const {r,before}=fixture(),f=periodicFull();f.revision=2;f.publication_id='bad-source';f.payload.snapshot.monsters[0].statuses[0].source={...entity,id:'B'};assert.throws(()=>r.apply(f),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);});
test('multiplayer periodic projection rejects wrong target generation and duplicate instance ids',()=>{for(const mutate of [s=>s[0].target={...npc,generation:2},s=>s.push(structuredClone(s[0]))]){const r=new ReplicaV6(key,'A'),f=periodicFull();mutate(f.payload.snapshot.monsters[0].statuses);assert.throws(()=>r.apply(f),code('INVALID_FRAME'));assert.equal(r.snapshot,null);}});
test('multiplayer periodic projection refuses null status lists and malformed periodic shape',()=>{for(const mutate of [n=>n.statuses=null,n=>n.statuses[0].periodic.raw_heal=10,n=>n.statuses[0].periodic.total_ticks=0,n=>delete n.statuses[0].applied_at_ms]){const r=new ReplicaV6(key,'A'),f=periodicFull();mutate(f.payload.snapshot.monsters[0]);assert.throws(()=>r.apply(f),code('INVALID_FRAME'));assert.equal(r.snapshot,null);}});
const shield=(source=entity,target=entity)=>({instance_id:'ward-1',definition_id:'ward',source,target,applied_at_ms:0,expires_at_ms:6000,modifier_bps:0,shield:{granted:20,remaining:15}});
function shieldFull(){const f=periodicFull();f.payload.snapshot.self.statuses=[shield()];f.payload.snapshot.self.shield_total=15;f.payload.snapshot.monsters[0].statuses=[shield(entity,npc)];f.payload.snapshot.monsters[0].shield_total=40;return f;}
test('multiplayer shield own exact total and NPC hidden-source aggregate reconstruct through delta depletion',()=>{const r=new ReplicaV6(key,'A'),f=shieldFull();assert.equal(r.apply(f).monsters[0].shield_total,40);const d=delta();d.payload.delta.own={...f.payload.snapshot.self,statuses:[],shield_total:0};assert.equal(r.apply(d).self.shield_total,0);});
test('multiplayer shield corrupt own balances and null total cannot overwrite a baseline',()=>{for(const mutate of [s=>s.shield_total=null,s=>delete s.shield_total,s=>s.shield_total=16,s=>s.statuses[0].shield.remaining=21,s=>s.statuses[0].shield.granted=-1,s=>s.statuses[0].target={...entity,generation:2},s=>s.statuses[0].source={...entity,id:'B'}]){const r=new ReplicaV6(key,'A'),f=shieldFull();r.apply(f);const before=r.snapshot,d=delta();d.payload.delta.own=structuredClone(before.self);mutate(d.payload.delta.own);assert.throws(()=>r.apply(d),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);}});
test('multiplayer shield NPC total bounds and malformed remaining are atomic failures',()=>{for(const mutate of [n=>n.shield_total=14,n=>n.shield_total=4294967296,n=>n.shield_total=null,n=>n.statuses[0].shield=null,n=>n.statuses[0].shield.remaining=20.5]){const r=new ReplicaV6(key,'A'),f=shieldFull();mutate(f.payload.snapshot.monsters[0]);assert.throws(()=>r.apply(f),code('INVALID_FRAME'));assert.equal(r.snapshot,null);}});
test('multiplayer shield private fields cannot be smuggled into public members',()=>{const r=new ReplicaV6(key,'A'),f=shieldFull();f.payload.snapshot.members[0].shield_total=15;assert.throws(()=>r.apply(f),code('INVALID_FRAME'));assert.equal(r.snapshot,null);});
function unitFull(){const f=shieldFull(),t={id:npc.id,generation:1};f.payload.snapshot.self.player_skills=[{skill_id:'unit',ready_at_ms:0}];f.payload.snapshot.self.pending_action={skill_id:'unit',commits_at_ms:120,unit_target:t};f.payload.snapshot.projectiles=[{id:'unit-shot',source:entity,position:[10,20],radius:6,unit_target:t}];return f;}
test('member decoder accepts a declared training owner and rejects foreign unit intent',()=>{
  const f=unitFull(),s=f.payload.snapshot;
  s.members.push({...s.members[0],entity:{...entity,id:'B'}});
  s.monsters[0].training_owner_id='A';
  assert.equal(new ReplicaV6(key,'A').apply(f).monsters[0].training_owner_id,'A');
  s.monsters[0].training_owner_id='B';
  assert.throws(()=>new ReplicaV6(key,'A').apply(f),code('INVALID_FRAME'));
  s.self.pending_action=null;
  s.projectiles=[];
  assert.equal(new ReplicaV6(key,'A').apply(f).monsters[0].training_owner_id,'B');
  s.monsters[0].training_owner_id='unassigned';
  assert.throws(()=>new ReplicaV6(key,'A').apply(f),code('INVALID_FRAME'));
});
test('unified unit pending and projectile identities reconstruct and clear through delta',()=>{const f=unitFull(),r=new ReplicaV6(key,'A');assert.equal(r.apply(f).self.pending_action.unit_target.id,npc.id);const d=delta();d.payload.delta.own={...f.payload.snapshot.self,pending_action:null};d.payload.delta.projectiles={upsert:[],removed:['unit-shot'],order:[]};const s=r.apply(d);assert.equal(s.self.pending_action,null);assert.deepEqual(s.projectiles,[]);});
test('unified unit malformed missing stale and null projectile targets are atomic failures',()=>{for(const mutate of [p=>p.unit_target=null,p=>p.unit_target.generation=0,p=>p.unit_target.generation=2,p=>p.unit_target.id='missing',p=>p.unit_target.extra=1,p=>p.source={...entity,generation:2},p=>p.source=npc]){const r=new ReplicaV6(key,'A'),f=unitFull();r.apply(f);const before=r.snapshot,bad=structuredClone(f);bad.revision=2;bad.publication_id='bad-unit';mutate(bad.payload.snapshot.projectiles[0]);assert.throws(()=>r.apply(bad),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);}});
test('unified unit pending target mutation cannot overwrite the prior own state',()=>{for(const target of [null,{id:npc.id,generation:0},{id:'A',generation:1},{id:npc.id,generation:2}]){const r=new ReplicaV6(key,'A'),f=unitFull();r.apply(f);const before=r.snapshot,d=delta();d.payload.delta.own=structuredClone(before.self);d.payload.delta.own.pending_action.unit_target=target;assert.throws(()=>r.apply(d),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);}});
test('unified ordinary and NPC projectile frames retain the legacy omitted target shape',()=>{const r=new ReplicaV6(key,'A'),f=periodicFull();f.payload.snapshot.projectiles=[{id:'ordinary',source:npc,position:[10,20],radius:6}];assert.ok(!Object.hasOwn(r.apply(f).projectiles[0],'unit_target'));});
function areaFull(kind='circle'){const f=shieldFull(),s=f.payload.snapshot;s.logical_ms=120;s.areas=[{id:'area-1',source:entity,skill_id:'area',mechanism:kind,center:[20,20],radius:45,created_at_ms:120,activates_at_ms:320,expires_at_ms:kind==='circle'?320:4820,interval_ms:kind==='circle'?0:600,next_tick_at_ms:kind==='circle'?320:920,completed_ticks:0,total_ticks:kind==='circle'?1:7,contact_policy:kind==='circle'?null:{max_targets_per_tick:2,max_total_hits:8,repeat_interval_ms:600,used_hits:0,remaining_hits:8,contacts:[]}}];return f;}
function contactedArea(){const f=areaFull('field'),s=f.payload.snapshot,a=s.areas[0];s.logical_ms=1000;a.completed_ticks=1;a.next_tick_at_ms=1520;a.contact_policy.used_hits=1;a.contact_policy.remaining_hits=7;a.contact_policy.contacts=[{target:{id:npc.id,generation:1},last_hit_at_ms:920,next_eligible_at_ms:1520}];return f;}
test('member area arrays create and explicitly clear through an atomic replacement delta',()=>{const r=new ReplicaV6(key,'A'),f=areaFull();assert.equal(r.apply(f).areas[0].source.id,'A');const d=delta();d.payload.delta.logical_ms=150;d.payload.delta.areas=[];assert.deepEqual(r.apply(d).areas,[]);});
test('member area field progress and generation history reconstruct from Rust-shaped data',()=>{const r=new ReplicaV6(key,'A'),f=contactedArea();assert.equal(r.apply(f).areas[0].contact_policy.used_hits,1);const copy=r.snapshot;copy.areas[0].contact_policy.remaining_hits=999;assert.equal(r.snapshot.areas[0].contact_policy.remaining_hits,7);});
test('member area null duplicate and wrong-source lists cannot replace a baseline',()=>{for(const mutate of [s=>s.areas=null,s=>s.areas.push(structuredClone(s.areas[0])),s=>s.areas[0].source={...entity,generation:2},s=>s.areas[0].source=npc,s=>s.areas[0].center=[NaN,0]]){const r=new ReplicaV6(key,'A'),f=areaFull();r.apply(f);const before=r.snapshot,bad=structuredClone(f);bad.revision=2;bad.publication_id='bad-area';mutate(bad.payload.snapshot);assert.throws(()=>r.apply(bad),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);}});
test('member area malformed schedule and completed counters fail atomically',()=>{for(const mutate of [a=>a.next_tick_at_ms=0,a=>a.completed_ticks=129,a=>a.expires_at_ms=300,a=>a.total_ticks=8,a=>a.interval_ms=0,a=>a.activates_at_ms=0]){const r=new ReplicaV6(key,'A'),f=areaFull('field');mutate(f.payload.snapshot.areas[0]);assert.throws(()=>r.apply(f),code('INVALID_FRAME'));assert.equal(r.snapshot,null);}});
test('member area budgets and semantic duplicate or future history identities are rejected',()=>{for(const mutate of [p=>p.remaining_hits=99,p=>p.used_hits=0,p=>p.contacts[0].target.generation=2,p=>p.contacts[0].next_eligible_at_ms=0,p=>{p.used_hits=2;p.remaining_hits=6;p.contacts.push({...p.contacts[0],target:{generation:1,id:npc.id}});}]){const r=new ReplicaV6(key,'A'),f=contactedArea();mutate(f.payload.snapshot.areas[0].contact_policy);assert.throws(()=>r.apply(f),code('INVALID_FRAME'));assert.equal(r.snapshot,null);}});
test('member area public views reject private fields and old snapshots retain omitted arrays',()=>{for(const field of ['action_id','raw','hit_statuses','spirit']){const r=new ReplicaV6(key,'A'),f=areaFull();f.payload.snapshot.areas[0][field]=field;assert.throws(()=>r.apply(f),code('INVALID_FRAME'));}const r=new ReplicaV6(key,'A');assert.ok(!Object.hasOwn(r.apply(full()),'areas'));});
function lineFull(){const f=contactedArea(),a=f.payload.snapshot.areas[0];a.mechanism='line_field';a.line={start:structuredClone(a.center),end:[520,20],width:90};return f;}
test('member line geometry and shared contact history rebuild and clear atomically',()=>{const f=lineFull(),r=new ReplicaV6(key,'A');assert.equal(r.apply(f).areas[0].line.end[0],520);const d=delta();d.payload.delta.logical_ms=1001;d.payload.delta.areas=[];assert.deepEqual(r.apply(d).areas,[]);});
test('member line missing null malformed and unknown geometry never overwrite cached state',()=>{for(const change of [a=>delete a.line,a=>a.line=null,a=>a.line.start=[0,0],a=>a.line.end=[Infinity,0],a=>a.line.end=[10021,20],a=>a.line.width=89,a=>a.line.extra=1,a=>a.line.end=[1]]){const r=new ReplicaV6(key,'A'),f=lineFull();r.apply(f);const before=r.snapshot,bad=structuredClone(f);bad.revision=2;bad.publication_id='bad-line';change(bad.payload.snapshot.areas[0]);assert.throws(()=>r.apply(bad),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);}});
test('member line geometry cannot be attached to either circular mechanism',()=>{for(const kind of ['circle','field']){const r=new ReplicaV6(key,'A'),f=areaFull(kind);f.payload.snapshot.areas[0].line={start:[20,20],end:[30,20],width:90};assert.throws(()=>r.apply(f),code('INVALID_FRAME'));assert.equal(r.snapshot,null);}});
test('member line clipped zero length capsule and full replacement delta retain exact endpoints',()=>{const f=lineFull(),a=f.payload.snapshot.areas[0];a.line.end=structuredClone(a.line.start);const r=new ReplicaV6(key,'A');r.apply(f);const d=delta();d.payload.delta.logical_ms=1001;d.payload.delta.areas=[structuredClone(a)];d.payload.delta.areas[0].line.end=[30,40];assert.deepEqual(r.apply(d).areas[0].line.end,[30,40]);});
function flightFull(interval=600){const f=structuredClone(shieldFull()),s=f.payload.snapshot;s.logical_ms=1000;s.projectiles=[{id:'flight-1',source:structuredClone(entity),position:[80,20],radius:8,piercing:{skill_id:'pierce',direction:[1,0],speed:240,remaining_range:400,created_at_ms:120,expires_at_ms:6120,max_hits:8,used_hits:1,remaining_hits:7,repeat_interval_ms:interval,contacts:[{target:{id:npc.id,generation:1},hit_at_ms:500,next_eligible_at_ms:interval>0?500+interval:null}]}}];return f;}
test('member piercing flight budget history reconstructs and removes through list delta',()=>{const r=new ReplicaV6(key,'A'),f=flightFull();assert.equal(r.apply(f).projectiles[0].piercing.used_hits,1);const d=delta();d.payload.delta.logical_ms=1030;d.payload.delta.projectiles={upsert:[],removed:['flight-1'],order:[]};assert.deepEqual(r.apply(d).projectiles,[]);});
test('member piercing null unknown geometry and wrong direction cannot poison a baseline',()=>{for(const change of [r=>r.piercing=null,r=>r.piercing.extra=1,r=>r.piercing.direction=[0,0],r=>r.piercing.direction=[1,1],r=>r.piercing.speed=0,r=>r.piercing.remaining_range=0,r=>r.source=npc,r=>r.source.generation=2]){const r=new ReplicaV6(key,'A'),f=flightFull();r.apply(f);const before=r.snapshot,bad=structuredClone(f);bad.revision=2;bad.publication_id='bad-flight';change(bad.payload.snapshot.projectiles[0]);assert.throws(()=>r.apply(bad),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);}});
test('member piercing future expired or inconsistent budgets and histories are rejected',()=>{for(const change of [p=>p.created_at_ms=1001,p=>p.expires_at_ms=1000,p=>p.used_hits=8,p=>p.remaining_hits=8,p=>p.contacts[0].target.generation=2,p=>p.contacts[0].hit_at_ms=120,p=>p.contacts[0].hit_at_ms=1001,p=>p.contacts[0].next_eligible_at_ms=null,p=>delete p.contacts[0].next_eligible_at_ms,p=>{p.used_hits=2;p.remaining_hits=6;p.contacts.push({...p.contacts[0],target:{generation:1,id:npc.id}});}]){const r=new ReplicaV6(key,'A'),f=flightFull();change(f.payload.snapshot.projectiles[0].piercing);assert.throws(()=>r.apply(f),code('INVALID_FRAME'));assert.equal(r.snapshot,null);}});
test('member piercing once history accepts old generation but does not invent repeated successes',()=>{const f=flightFull(0);f.payload.snapshot.monsters[0].entity.generation=2;f.payload.snapshot.monsters[0].statuses=[];const r=new ReplicaV6(key,'A');assert.equal(r.apply(f).projectiles[0].piercing.contacts[0].target.generation,1);const d=delta();d.payload.delta.logical_ms=1030;d.payload.delta.projectiles={upsert:[structuredClone(f.payload.snapshot.projectiles[0])],removed:[],order:['flight-1']};d.payload.delta.projectiles.upsert[0].piercing.used_hits=2;d.payload.delta.projectiles.upsert[0].piercing.remaining_hits=6;assert.throws(()=>r.apply(d),code('INVALID_FRAME'));});
test('member piercing cannot combine unit targeting or expose private plan values',()=>{for(const change of [r=>r.unit_target={id:npc.id,generation:1},r=>r.piercing.raw=999,r=>r.piercing.action_id='private',r=>r.piercing.hit_statuses=['fake']]){const f=flightFull(),r=new ReplicaV6(key,'A');change(f.payload.snapshot.projectiles[0]);assert.throws(()=>r.apply(f),code('INVALID_FRAME'));assert.equal(r.snapshot,null);}});
test('member piercing shared history limit rejects over-budget lists while ordinary shots stay omitted',()=>{const f=flightFull(),a=f.payload.snapshot.projectiles[0];a.piercing.max_hits=128;a.piercing.remaining_hits=127;f.payload.snapshot.projectiles.push({...structuredClone(a),id:'flight-2'});const r=new ReplicaV6(key,'A');assert.throws(()=>r.apply(f),code('INVALID_FRAME'));const old=new ReplicaV6(key,'A').apply(unitFull());assert.ok(!Object.hasOwn(old.projectiles[0],'piercing'));});
function slotsFull(){const f=structuredClone(full());f.payload.snapshot.self.player_skills=Array.from({length:8},(_,i)=>({skill_id:'slot-'+i,ready_at_ms:100+i}));return f;}
test('member slots ordered eight cooldowns and private pending rebuild atomically',()=>{const r=new ReplicaV6(key,'A'),f=slotsFull();assert.deepEqual(r.apply(f).self.player_skills.map(s=>s.skill_id),Array.from({length:8},(_,i)=>'slot-'+i));const d=delta();d.payload.delta.own=structuredClone(f.payload.snapshot.self);d.payload.delta.own.player_skills[7].ready_at_ms=300;d.payload.delta.own.pending_action={skill_id:'slot-7',commits_at_ms:120};assert.equal(r.apply(d).self.player_skills[7].ready_at_ms,300);});
test('member slots reject overlong duplicate missing and unknown row data without cache mutation',()=>{for(const change of [s=>s.player_skills.push({...s.player_skills[0]}),s=>s.player_skills[7].skill_id=s.player_skills[0].skill_id,s=>delete s.player_skills[7].skill_id,s=>s.player_skills[7].extra=1,s=>s.player_skills[7].skill_id='',s=>s.player_skills[7].ready_at_ms=-1,s=>s.player_skills[7].ready_at_ms=2**53,s=>s.player_skills[7].ready_at_ms=null]){const r=new ReplicaV6(key,'A'),f=slotsFull();r.apply(f);const before=r.snapshot,d=delta();d.payload.delta.own=structuredClone(before.self);change(d.payload.delta.own);assert.throws(()=>r.apply(d),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);}});
test('member slots pending must reference an existing private skill and valid time',()=>{for(const pending of [{skill_id:'missing',commits_at_ms:120},{skill_id:'slot-7',commits_at_ms:-1},{skill_id:'slot-7',commits_at_ms:2**53}]){const f=slotsFull(),r=new ReplicaV6(key,'A');f.payload.snapshot.self.pending_action=pending;assert.throws(()=>r.apply(f),code('INVALID_FRAME'));assert.equal(r.snapshot,null);}});
test('member slots changed connection preserves every authoritative cooldown while old frame is fenced',()=>{const r=new ReplicaV6(key,'A'),f=slotsFull();r.apply(f);const next=structuredClone(f);next.connection={id:'slots-reconnect',generation:2};next.stream_epoch=2;next.revision=10;next.publication_id='slots-new-full';const after=r.apply(next);assert.deepEqual(after.self.player_skills,f.payload.snapshot.self.player_skills);assert.throws(()=>r.apply(f),code('STALE_FRAME'));});

function chargeFull() {
  const frame = slotsFull();
  const snapshot = frame.payload.snapshot;
  snapshot.logical_ms = 30;
  snapshot.self.pending_action = {
    skill_id: 'slot-0', commits_at_ms: 450,
    charge: { action_id: 'charge-1', started_at_ms: 30, min_release_at_ms: 120, auto_commit_at_ms: 450 },
  };
  return frame;
}

test('charge preparation rebuilds private release timing and clears on commit', () => {
  const replica = new ReplicaV6(key, 'A');
  const frame = chargeFull();
  replica.apply(frame);
  const released = delta();
  released.payload.delta.logical_ms = 120;
  released.payload.delta.own = structuredClone(frame.payload.snapshot.self);
  released.payload.delta.own.pending_action.commits_at_ms = 120;
  assert.equal(replica.apply(released).self.pending_action.charge.action_id, 'charge-1');
  const committed = structuredClone(released);
  committed.revision = 3;
  committed.publication_id = 'delta-3';
  committed.payload.base_revision = 2;
  committed.payload.base_publication_id = released.publication_id;
  committed.payload.delta.logical_ms = 150;
  committed.payload.delta.own.pending_action = null;
  assert.equal(replica.apply(committed).self.pending_action, null);
});

test('charge decoder rejects invalid identity target and deadline without changing cache', () => {
  const mutations = [
    pending => { pending.charge = null; },
    pending => { pending.charge.action_id = ''; },
    pending => { pending.charge.started_at_ms = 31; },
    pending => { pending.charge.min_release_at_ms = 450; },
    pending => { pending.commits_at_ms = 120; },
    pending => { pending.ground_position = [0, 0]; },
    pending => { pending.charge.speed = 1; },
  ];
  const replica = new ReplicaV6(key, 'A');
  const before = replica.apply(chargeFull());
  for (const change of mutations) {
    const invalid = chargeFull();
    invalid.revision = 2;
    invalid.publication_id = 'bad-charge';
    change(invalid.payload.snapshot.self.pending_action);
    assert.throws(() => replica.apply(invalid), code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot, before);
  }
});

test('reaim delta publishes the latest private unit direction and keeps its clock', () => {
  const frame = chargeFull();
  const own = frame.payload.snapshot.self;
  own.pending_action.charge.aim = { direction: [1, 0], updated_at_ms: 30 };
  const replica = new ReplicaV6(key, 'A');
  replica.apply(frame);
  const update = delta();
  update.payload.delta.logical_ms = 120;
  update.payload.delta.own = structuredClone(own);
  update.payload.delta.own.pending_action.charge.aim = { direction: [0, 1], updated_at_ms: 120 };
  assert.deepEqual(replica.apply(update).self.pending_action.charge.aim,
    { direction: [0, 1], updated_at_ms: 120 });
});

test('reaim decoder rejects malformed direction or clock atomically', () => {
  const frame = chargeFull();
  frame.payload.snapshot.self.pending_action.charge.aim = { direction: [1, 0], updated_at_ms: 30 };
  const replica = new ReplicaV6(key, 'A');
  const before = replica.apply(frame);
  for (const aim of [null, { direction: [2, 0], updated_at_ms: 30 },
    { direction: [1, 0], updated_at_ms: 29 }, { direction: [1, 0], updated_at_ms: 31 },
    { direction: [1, 0], updated_at_ms: 30, speed: 7 }]) {
    const invalid = structuredClone(frame);
    invalid.revision = 2;
    invalid.publication_id = 'bad-reaim';
    invalid.payload.snapshot.self.pending_action.charge.aim = aim;
    assert.throws(() => replica.apply(invalid), code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot, before);
  }
});

function channelFull() {
  const frame = slotsFull();
  frame.payload.snapshot.logical_ms = 30;
  frame.payload.snapshot.self.active_channel = {
    action_id: 'channel-1', skill_id: 'slot-0', direction: [1, 0],
    started_at_ms: 30, ends_at_ms: 390, next_pulse_at_ms: 120, pulses: 0,
  };
  return frame;
}

test('channel replica replaces a private active phase and clears it on recovery', () => {
  const frame = channelFull();
  const replica = new ReplicaV6(key, 'A');
  replica.apply(frame);
  const pulse = delta();
  pulse.payload.delta.logical_ms = 120;
  pulse.payload.delta.own = structuredClone(frame.payload.snapshot.self);
  pulse.payload.delta.own.active_channel.pulses = 1;
  pulse.payload.delta.own.active_channel.next_pulse_at_ms = 210;
  assert.equal(replica.apply(pulse).self.active_channel.pulses, 1);
  const ended = structuredClone(pulse);
  ended.revision = 3;
  ended.publication_id = 'delta-3';
  ended.payload.base_revision = 2;
  ended.payload.base_publication_id = pulse.publication_id;
  ended.payload.delta.logical_ms = 390;
  delete ended.payload.delta.own.active_channel;
  ended.payload.delta.own.recovery_until_ms = 510;
  assert.equal(replica.apply(ended).self.active_channel, undefined);
  assert.equal(replica.snapshot.self.recovery_until_ms, 510);
});

test('channel replica rejects invalid timing or occupied phase without mutating its snapshot', () => {
  const frame = channelFull();
  const replica = new ReplicaV6(key, 'A');
  const before = replica.apply(frame);
  const mutations = [
    own => { own.active_channel = null; },
    own => { own.active_channel.direction = [2, 0]; },
    own => { own.active_channel.next_pulse_at_ms = 30; },
    own => { delete own.active_channel.next_pulse_at_ms; },
    own => { own.active_channel.pulses = 129; },
    own => { own.active_channel.skill_id = 'missing'; },
    own => { own.protected = true; },
    own => { own.pending_action = { skill_id: 'slot-0', commits_at_ms: 50 }; },
  ];
  for (const change of mutations) {
    const invalid = structuredClone(frame);
    invalid.revision = 2;
    invalid.publication_id = 'bad-channel';
    change(invalid.payload.snapshot.self);
    assert.throws(() => replica.apply(invalid), code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot, before);
  }
});

function recastFull() {
  const frame = slotsFull();
  const snapshot = frame.payload.snapshot;
  snapshot.projectiles = [{id: 'shot-1', source: entity, position: [12, 20], radius: 1}];
  snapshot.self.projectile_recast = {
    action_id: 'recast-1', skill_id: 'slot-0', projectile_id: 'shot-1', expires_at_ms: 300, next_index: 1,
  };
  return frame;
}

test('recast replica clears the private window while retaining the same shot', () => {
  const frame = recastFull();
  const replica = new ReplicaV6(key, 'A');
  assert.equal(replica.apply(frame).self.projectile_recast.projectile_id, 'shot-1');
  const consumed = delta();
  consumed.payload.delta.own = structuredClone(frame.payload.snapshot.self);
  delete consumed.payload.delta.own.projectile_recast;
  const snapshot = replica.apply(consumed);
  assert.equal(snapshot.self.projectile_recast, undefined);
  assert.equal(snapshot.projectiles[0].id, 'shot-1');
});

test('recast replica rejects invalid references, expired windows and bad deltas atomically', () => {
  const frame = recastFull();
  const replica = new ReplicaV6(key, 'A');
  const before = replica.apply(frame);
  for (const change of [
    own => { own.projectile_recast = null; },
    own => { own.projectile_recast.expires_at_ms = 0; },
    own => { own.projectile_recast.skill_id = 'missing'; },
    own => { own.projectile_recast.next_index = 2; },
    own => { own.projectile_recast.projectile_id = 'absent'; },
  ]) {
    const invalid = structuredClone(frame);
    invalid.revision = 2;
    invalid.publication_id = 'bad-recast';
    change(invalid.payload.snapshot.self);
    assert.throws(() => replica.apply(invalid), code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot, before);
  }
  const invalid = delta();
  invalid.payload.delta.projectiles = {upsert: [], removed: ['shot-1'], order: []};
  assert.throws(() => replica.apply(invalid), code('INVALID_FRAME'));
  assert.deepEqual(replica.snapshot, before);
});

function followupFull() {
  const frame = slotsFull();
  const own = frame.payload.snapshot.self;
  own.recovery_until_ms = 155;
  own.followup = {
    source_skill_id: 'slot-0', promotes_at_ms: 180,
    queued: {action_id: 'follow-1', skill_id: 'slot-1', direction: [1, 0], expires_at_ms: 180},
  };
  return frame;
}

test('followup replica replaces private queue and clears it when preparation starts', () => {
  const frame = followupFull();
  const replica = new ReplicaV6(key, 'A');
  assert.equal(replica.apply(frame).self.followup.queued.skill_id, 'slot-1');
  const update = delta();
  update.payload.delta.own = structuredClone(frame.payload.snapshot.self);
  update.payload.delta.own.followup.queued.skill_id = 'slot-2';
  assert.equal(replica.apply(update).self.followup.queued.skill_id, 'slot-2');
  const promoted = structuredClone(update);
  promoted.revision = 3;
  promoted.publication_id = 'followup-promoted';
  promoted.payload.base_revision = 2;
  promoted.payload.base_publication_id = update.publication_id;
  promoted.payload.delta.logical_ms = 180;
  delete promoted.payload.delta.own.followup;
  promoted.payload.delta.own.pending_action = {skill_id: 'slot-2', commits_at_ms: 210};
  assert.equal(replica.apply(promoted).self.followup, undefined);
  assert.equal(replica.snapshot.self.pending_action.skill_id, 'slot-2');
});

test('followup replica rejects malformed private queues and leaves valid baseline intact', () => {
  const frame = followupFull();
  const replica = new ReplicaV6(key, 'A');
  const before = replica.apply(frame);
  for (const change of [
    own => { own.followup = null; },
    own => { delete own.followup.queued; },
    own => { own.followup.queued.direction = [0, 0]; },
    own => { own.followup.queued.skill_id = 'slot-0'; },
    own => { own.followup.queued.expires_at_ms = 179; },
    own => { own.followup.promotes_at_ms = 154; },
    own => { own.pending_action = {skill_id: 'slot-1', commits_at_ms: 50}; },
  ]) {
    const invalid = delta();
    invalid.payload.delta.own = structuredClone(frame.payload.snapshot.self);
    change(invalid.payload.delta.own);
    assert.throws(() => replica.apply(invalid), code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot, before);
  }
});

function recoveryCancelFull() {
  const frame = slotsFull();
  const own = frame.payload.snapshot.self;
  own.recovery_until_ms = 330;
  own.recovery_cancel = {skill_id: 'slot-0', cancel_at_ms: 150};
  return frame;
}

test('recovery cancel replica clears the private window when a new preparation takes over', () => {
  const frame = recoveryCancelFull();
  const replica = new ReplicaV6(key, 'A');
  assert.equal(replica.apply(frame).self.recovery_cancel.cancel_at_ms, 150);
  const update = delta();
  update.payload.delta.logical_ms = 150;
  update.payload.delta.own = structuredClone(frame.payload.snapshot.self);
  delete update.payload.delta.own.recovery_cancel;
  update.payload.delta.own.recovery_until_ms = 150;
  update.payload.delta.own.pending_action = {skill_id: 'slot-1', commits_at_ms: 210};
  assert.equal(replica.apply(update).self.recovery_cancel, undefined);
  assert.equal(replica.snapshot.self.pending_action.skill_id, 'slot-1');
});

test('recovery cancel replica rejects malformed private windows without changing its baseline', () => {
  const frame = recoveryCancelFull();
  const replica = new ReplicaV6(key, 'A');
  const before = replica.apply(frame);
  for (const change of [
    own => { own.recovery_cancel = null; },
    own => { delete own.recovery_cancel.cancel_at_ms; },
    own => { own.recovery_cancel.cancel_at_ms = 330; },
    own => { own.recovery_cancel.skill_id = 'unknown'; },
    own => { own.protected = true; },
    own => { own.followup = {source_skill_id: 'slot-1', promotes_at_ms: 330, queued: null}; },
  ]) {
    const invalid = delta();
    invalid.payload.delta.own = structuredClone(frame.payload.snapshot.self);
    change(invalid.payload.delta.own);
    assert.throws(() => replica.apply(invalid), code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot, before);
  }
});

function basicFollowupFull() {
  const frame = full();
  const own = frame.payload.snapshot.self;
  own.basic_ready_at_ms = 155;
  own.basic_followup = {
    action_id: 'basic-next', target: {id: 'npc', generation: 1}, promotes_at_ms: 180, expires_at_ms: 330,
  };
  return frame;
}

test('basic followup replica clears its private intent when preparation starts', () => {
  const frame = basicFollowupFull();
  const replica = new ReplicaV6(key, 'A');
  assert.equal(replica.apply(frame).self.basic_followup.target.generation, 1);
  const update = delta();
  update.payload.delta.logical_ms = 180;
  update.payload.delta.own = structuredClone(frame.payload.snapshot.self);
  delete update.payload.delta.own.basic_followup;
  update.payload.delta.own.pending_action = {skill_id: null, commits_at_ms: 210};
  assert.equal(replica.apply(update).self.basic_followup, undefined);
});

test('basic followup replica rejects malformed identity, deadline and phase atomically', () => {
  const frame = basicFollowupFull();
  const replica = new ReplicaV6(key, 'A');
  const before = replica.apply(frame);
  for (const change of [
    own => { own.basic_followup = null; },
    own => { own.basic_followup.target.generation = 0; },
    own => { own.basic_followup.promotes_at_ms = 154; },
    own => { own.basic_followup.expires_at_ms = 179; },
    own => { own.pending_action = {skill_id: null, commits_at_ms: 220}; },
  ]) {
    const invalid = delta();
    invalid.payload.delta.own = structuredClone(frame.payload.snapshot.self);
    change(invalid.payload.delta.own);
    assert.throws(() => replica.apply(invalid), code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot, before);
  }
});

function lockedBasicFull() {
  const frame = unitFull();
  frame.payload.snapshot.self.pending_action.skill_id = null;
  frame.payload.snapshot.projectiles = [];
  return frame;
}

test('locked basic replica reconstructs the private preparation target without a skill slot', () => {
  const replica = new ReplicaV6(key, 'A');
  const frame = lockedBasicFull();
  const pending = replica.apply(frame).self.pending_action;
  assert.equal(pending.skill_id, null);
  assert.deepEqual(pending.unit_target, {id: npc.id, generation: 1});
  const cleared = delta();
  cleared.payload.delta.own = structuredClone(frame.payload.snapshot.self);
  cleared.payload.delta.own.pending_action = null;
  assert.equal(replica.apply(cleared).self.pending_action, null);
});

test('locked basic replica rejects mixed aim and stale target without changing its baseline', () => {
  const replica = new ReplicaV6(key, 'A');
  const frame = lockedBasicFull();
  const before = replica.apply(frame);
  for (const change of [
    pending => { pending.ground_position = [0, 0]; },
    pending => { pending.unit_target = null; },
    pending => { pending.unit_target.generation += 1; },
  ]) {
    const invalid = delta();
    invalid.payload.delta.own = structuredClone(frame.payload.snapshot.self);
    change(invalid.payload.delta.own.pending_action);
    assert.throws(() => replica.apply(invalid), code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot, before);
  }
});

function pursuitFull() {
  const frame = lockedBasicFull();
  const own = frame.payload.snapshot.self;
  own.pending_action = null;
  own.basic_pursuit = {action_id: 'approach', target: {id: npc.id, generation: 1}, started_at_ms: 0, expires_at_ms: 1800};
  return frame;
}

test('basic pursuit replica moves from private pursuit to preparation of the same target', () => {
  const frame = pursuitFull();
  const replica = new ReplicaV6(key, 'A');
  const state = replica.apply(frame);
  assert.equal(state.self.basic_pursuit.target.id, npc.id);
  const update = delta();
  update.payload.delta.own = structuredClone(state.self);
  delete update.payload.delta.own.basic_pursuit;
  update.payload.delta.own.pending_action = {skill_id: null, commits_at_ms: 90, unit_target: {id: npc.id, generation: 1}};
  const promoted = replica.apply(update);
  assert.equal(promoted.self.basic_pursuit, undefined);
  assert.deepEqual(promoted.self.pending_action.unit_target, state.self.basic_pursuit.target);
});

test('basic pursuit replica rejects invalid deadline, target or overlapping phase atomically', () => {
  const frame = pursuitFull();
  const replica = new ReplicaV6(key, 'A');
  const before = replica.apply(frame);
  for (const change of [
    own => { own.basic_pursuit = null; },
    own => { own.basic_pursuit.expires_at_ms = 1801; },
    own => { own.basic_pursuit.expires_at_ms = 30; },
    own => { own.basic_pursuit.started_at_ms = 31; },
    own => { own.basic_pursuit.target.generation = 0; },
    own => { own.pending_action = {skill_id: null, commits_at_ms: 90}; },
  ]) {
    const invalid = delta();
    invalid.payload.delta.own = structuredClone(frame.payload.snapshot.self);
    change(invalid.payload.delta.own);
    assert.throws(() => replica.apply(invalid), code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot, before);
  }
});

test('item cooldown replica validates deadlines and removal without replacing a good baseline', () => {
  const frame = pursuitFull();
  const replica = new ReplicaV6(key, 'A');
  frame.payload.snapshot.self.item_cooldowns = [
    {item_id: '\uE000', ready_at_ms: 90},
    {item_id: '\u{10000}', ready_at_ms: 90},
  ];
  const before = replica.apply(frame);
  for (const cooldowns of [null, [{item_id: 'pill', ready_at_ms: 0}],
    [{item_id: 'pill', ready_at_ms: 60031}], [{item_id: '火'.repeat(22), ready_at_ms: 90}],
    [{item_id: 'pill', ready_at_ms: 90}, {item_id: 'pill', ready_at_ms: 92}]]) {
    const invalid = delta();
    invalid.payload.delta.own = {...structuredClone(before.self), item_cooldowns: cooldowns};
    assert.throws(() => replica.apply(invalid), code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot, before);
  }
  const update = delta();
  update.payload.delta.own = structuredClone(before.self);
  delete update.payload.delta.own.item_cooldowns;
  assert.equal(replica.apply(update).self.item_cooldowns, undefined);
});

test('ally pending targets retain their own identity across full and delta frames', () => {
  const f = full();
  const snapshot = f.payload.snapshot;
  const ally = {...entity, id: 'B', generation: 2};
  const target = {id: ally.id, generation: ally.generation};
  snapshot.members.push({...snapshot.members[0], entity: ally});
  snapshot.self.player_skills = [{skill_id: 'heal', ready_at_ms: 0}];
  snapshot.self.pending_action = {skill_id: 'heal', commits_at_ms: 120, ally_target: target};
  const replica = new ReplicaV6(key, 'A');
  const baseline = replica.apply(f);
  assert.deepEqual(baseline.self.pending_action.ally_target, target);
  for (const corrupt of [
    pending => { pending.ally_target = null; },
    pending => { pending.ally_target.generation++; },
    pending => { pending.ally_target.id = 'A'; },
    pending => { pending.skill_id = null; },
    pending => { pending.ground_position = [10, 20]; },
    pending => { pending.unit_target = target; delete pending.ally_target; },
  ]) {
    const update = delta();
    update.payload.delta.own = structuredClone(baseline.self);
    corrupt(update.payload.delta.own.pending_action);
    assert.throws(() => replica.apply(update), code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot, baseline);
  }
  const update = delta();
  update.payload.delta.own = {...baseline.self, pending_action: null};
  assert.equal(replica.apply(update).self.pending_action, null);
});

test('ally HOT can appear only on its target with a matching member source', () => {
  const frame = full();
  const snapshot = frame.payload.snapshot;
  const ally = {...entity, id: 'B', generation: 2};
  snapshot.members.push({...snapshot.members[0], entity: ally});
  snapshot.self.shield_total = 0;
  const replica = new ReplicaV6(key, 'A');
  const baseline = replica.apply(frame);
  const status = {
    instance_id: 'hot-1', definition_id: 'renewal', source: ally, target: entity,
    applied_at_ms: 0, expires_at_ms: 2000, modifier_bps: 0,
    periodic: {kind: 'max_hp_heal', interval_ms: 600, next_tick_at_ms: 600,
      ticks_done: 0, total_ticks: 3, raw_heal: 80},
  };
  for (const corrupt of [
    row => { row.source.generation++; },
    row => { row.target = ally; },
    row => { delete row.periodic; },
    row => { row.periodic.kind = 'physical_damage'; },
    row => { row.periodic.raw_heal = -1; },
  ]) {
    const update = delta();
    const bad = structuredClone(status);
    corrupt(bad);
    update.payload.delta.own = {...baseline.self, statuses: [bad]};
    assert.throws(() => replica.apply(update), code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot, baseline);
  }
  const update = delta();
  update.payload.delta.own = {...baseline.self, statuses: [status]};
  assert.deepEqual(replica.apply(update).self.statuses, [status]);
});

test('ally shield projection verifies the grant source and contributes only to the target total', () => {
  const frame = full();
  const state = frame.payload.snapshot;
  const ally = {...entity, id: 'B', generation: 2};
  state.members.push({...state.members[0], entity: ally});
  state.self.shield_total = 0;
  const replica = new ReplicaV6(key, 'A');
  const baseline = replica.apply(frame);
  const granted = shield(ally, entity);
  for (const corrupt of [
    own => { own.shield_total = 20; },
    own => { own.statuses[0].source.generation++; },
    own => { own.statuses[0].target = ally; },
    own => { own.statuses[0].periodic = {kind: 'physical_damage'}; },
  ]) {
    const update = delta();
    update.payload.delta.own = {...baseline.self, shield_total: 15, statuses: [structuredClone(granted)]};
    corrupt(update.payload.delta.own);
    assert.throws(() => replica.apply(update), code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot, baseline);
  }
  const update = delta();
  update.payload.delta.own = {...baseline.self, shield_total: 15, statuses: [granted]};
  assert.deepEqual(replica.apply(update).self.statuses, [granted]);
});


test('recorded shields and active reduction survive deltas and reject malformed ledgers atomically',()=>{
 const f=shieldFull();const status=f.payload.snapshot.self.statuses[0];status.shield.record={basis:'incoming',recorded:10,cap:30};status.modifiers=[{attribute:'damage_reduction',modifier_bps:2000}];
 const r=new ReplicaV6(key,'A');assert.equal(r.apply(f).self.statuses[0].shield.record.recorded,10);const baseline=r.snapshot;
 for(const change of [s=>s.shield.record.recorded=31,s=>s.shield.record.cap=-1,s=>s.shield.record.basis='raw',s=>s.shield.record=null,s=>s.shield.record.secret=1,s=>s.modifiers[0].attribute='atk',s=>delete s.shield.record]){
  const d=delta();d.payload.delta.own=structuredClone(baseline.self);change(d.payload.delta.own.statuses[0]);assert.throws(()=>r.apply(d),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,baseline);
 }
 const d=delta();d.payload.delta.own=structuredClone(baseline.self);d.payload.delta.own.statuses[0].shield.record.recorded=20;assert.equal(r.apply(d).self.statuses[0].shield.record.recorded,20);
});
test('recorded shield support retains ally attribution and NPC projection',()=>{
 const f=shieldFull(),s=f.payload.snapshot;const ally={...entity,id:'B'};s.members.push({...s.members[0],entity:ally});s.self.statuses[0].source=ally;s.self.statuses[0].shield.record={basis:'own_absorbed',recorded:4,cap:10};s.self.statuses[0].modifiers=[{attribute:'damage_reduction',modifier_bps:1500}];
 s.monsters[0].statuses[0].shield.record={basis:'incoming',recorded:6,cap:20};s.monsters[0].statuses[0].modifiers=[{attribute:'damage_reduction',modifier_bps:2000}];
 const r=new ReplicaV6(key,'A');assert.equal(r.apply(f).self.statuses[0].source.id,'B');
});
test('frozen shield balances can exceed current maxima without inventing balance or relaxing integer bounds',()=>{
 const f=shieldFull(),s=f.payload.snapshot;s.self.max_hp=10;s.self.hp=10;s.members[0].max_hp=10;s.members[0].hp=10;s.monsters[0].max_hp=20;s.monsters[0].hp=20;
 const r=new ReplicaV6(key,'A');assert.equal(r.apply(f).self.shield_total,15);assert.equal(r.snapshot.monsters[0].shield_total,40);
 const bad=delta();bad.payload.delta.own={...r.snapshot.self,shield_total:16};assert.throws(()=>r.apply(bad),code('INVALID_FRAME'));
});

test('transferred numeric statuses retain an existing ally source through full and delta snapshots',()=>{
 const f=full(),s=f.payload.snapshot,ally={...entity,id:'B',generation:2};s.members.push({...s.members[0],entity:ally});s.self.shield_total=0;
 const row={instance_id:'moved-1',definition_id:'gift',source:ally,target:entity,applied_at_ms:0,expires_at_ms:1500,modifier_bps:1000,attribute:'atk',stacks:2,max_stacks:3};
 s.self.statuses=[row];const r=new ReplicaV6(key,'A');assert.deepEqual(r.apply(f).self.statuses,[row]);
 const d=delta();d.payload.delta.own={...s.self,statuses:[]};assert.deepEqual(r.apply(d).self.statuses,[]);
});
test('transferred compound statuses reject stale sources malformed layers controls and unknown fields atomically',()=>{
 const f=full(),s=f.payload.snapshot,ally={...entity,id:'B',generation:2};s.members.push({...s.members[0],entity:ally});s.self.shield_total=0;
 const row={instance_id:'moved-1',definition_id:'gift',source:ally,target:entity,applied_at_ms:0,expires_at_ms:1500,modifier_bps:0,modifiers:[{attribute:'atk',modifier_bps:1000}]};
 const r=new ReplicaV6(key,'A');r.apply(f);const baseline=r.snapshot;
 for(const change of [s=>s.source.generation++,s=>s.target=ally,s=>s.expires_at_ms=0,s=>s.stacks=2,s=>{delete s.modifiers;s.attribute='stun';},s=>s.modifiers[0].attribute='unknown',s=>s.secret=1]){
  const d=delta();d.payload.delta.own={...baseline.self,statuses:[structuredClone(row)]};change(d.payload.delta.own.statuses[0]);assert.throws(()=>r.apply(d),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,baseline);
 }
 const d=delta();d.payload.delta.own={...baseline.self,statuses:[row]};assert.deepEqual(r.apply(d).self.statuses,[row]);
});

const windowRecordFixture=()=>({basis:'damage_dealt',recorded:60,cap:2000,contacts:2,maximum_sampling:'snapshot',max_hp_at_apply:100,max_spirit_at_apply:50});
test('status windows retain safe integer totals and immutable attribution through deltas',()=>{
 const f=full(),s=f.payload.snapshot;s.self.shield_total=0;s.self.statuses=[{instance_id:'window-1',definition_id:'permit',source:entity,target:entity,attribute:'def',modifier_bps:1,applied_at_ms:0,expires_at_ms:4200,window:windowRecordFixture()}];
 const r=new ReplicaV6(key,'A');r.apply(f);const d=delta();d.payload.delta.own=structuredClone(s.self);d.payload.delta.own.statuses[0].window.recorded=2000;assert.equal(r.apply(d).self.statuses[0].window.recorded,2000);
});
test('status windows reject malformed record caps carriers and snapshots atomically',()=>{
 const f=full(),s=f.payload.snapshot;s.self.shield_total=0;s.self.statuses=[{instance_id:'window-1',definition_id:'permit',source:entity,target:entity,attribute:'def',modifier_bps:1,applied_at_ms:0,expires_at_ms:4200,window:windowRecordFixture()}];
 const r=new ReplicaV6(key,'A');const baseline=r.apply(f);
 for(const change of [s=>s.window=null,s=>s.window.recorded=2001,s=>s.window.cap=2**53,s=>s.window.contacts=0,s=>s.window.max_hp_at_apply=0,s=>s.window.maximum_sampling='unknown',s=>s.window.secret=1,s=>s.window.basis='none',s=>s.shield={granted:0,remaining:0},s=>s.attribute='stun']){
  const d=delta();d.payload.delta.own=structuredClone(baseline.self);change(d.payload.delta.own.statuses[0]);assert.throws(()=>r.apply(d),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,baseline);
 }
});
test('ally HOT windows support pure end recovery and keep the original source generation',()=>{
 const f=full(),s=f.payload.snapshot,ally={...entity,id:'B',generation:2};s.members.push({...s.members[0],entity:ally});s.self.shield_total=0;
 s.self.statuses=[{instance_id:'hot-window',definition_id:'renewal',source:ally,target:entity,applied_at_ms:0,expires_at_ms:3900,modifier_bps:0,periodic:{kind:'max_hp_heal',interval_ms:1000,next_tick_at_ms:1000,ticks_done:0,total_ticks:3,raw_heal:10},window:{...windowRecordFixture(),basis:'none',cap:0,recorded:0,contacts:0}}];
 const r=new ReplicaV6(key,'A');assert.equal(r.apply(f).self.statuses[0].source.generation,2);
 const d=delta();d.payload.delta.own=structuredClone(s.self);d.payload.delta.own.statuses[0].source.generation=3;assert.throws(()=>r.apply(d),code('INVALID_FRAME'));
});

const flaggedStatus=(source=entity,target=entity)=>({instance_id:'compound-1',definition_id:'compound',source,target,applied_at_ms:0,expires_at_ms:2000,modifier_bps:0,flags:['root','silence'],modifiers:[{attribute:'atk',modifier_bps:-2000}]});
test('compound flags and numeric state share one NPC identity through refresh deltas',()=>{
 const f=periodicFull(),s=f.payload.snapshot;s.self.shield_total=0;s.self.statuses=[{...flaggedStatus(npc),instance_id:'own-control'}];s.monsters[0].statuses=[flaggedStatus(entity,npc)];
 const r=new ReplicaV6(key,'A');r.apply(f);const refreshed=structuredClone(s.monsters[0]);refreshed.statuses[0].applied_at_ms=30;refreshed.statuses[0].expires_at_ms=2030;
 const d=delta();d.payload.delta.monsters={upsert:[refreshed],removed:[],order:[npc]};
 assert.deepEqual(r.apply(d).monsters[0].statuses[0],refreshed.statuses[0]);
});
test('compound flags reject malformed or incompatible carriers without changing the baseline',()=>{
 const f=full();f.payload.snapshot.self.shield_total=0;const r=new ReplicaV6(key,'A'),baseline=r.apply(f);
 for(const change of [s=>s.flags=null,s=>s.flags=[],s=>s.flags=['root','root'],s=>s.flags=['root','control_immunity'],s=>s.flags=['spirit_recovery_lock'],s=>s.flags=['atk'],s=>s.flags=['unknown'],s=>s.flags=['root','silence','stun','damage_immunity'],s=>s.attribute='def',s=>s.modifier_bps=1,s=>s.stacks=1,s=>s.max_stacks=1,s=>s.window=windowRecordFixture(),s=>s.periodic={kind:'physical_damage'},s=>s.shield={granted:0,remaining:0}]){
  const d=delta();const status=flaggedStatus();change(status);d.payload.delta.own={...baseline.self,statuses:[status]};assert.throws(()=>r.apply(d),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,baseline);
 }
});
test('flag-only ally immunity preserves generation while allied hostile control is rejected',()=>{
 const f=full(),s=f.payload.snapshot,ally={...entity,id:'B',generation:2};s.members.push({...s.members[0],entity:ally});s.self.shield_total=0;
 const status=flaggedStatus(ally);delete status.modifiers;status.flags=['control_immunity','first_control_immunity'];s.self.statuses=[status];
 const r=new ReplicaV6(key,'A');assert.deepEqual(r.apply(f).self.statuses,[status]);const baseline=r.snapshot;
 for(const change of [s=>s.source.generation++,s=>s.flags=['stun']]){const d=delta();d.payload.delta.own=structuredClone(baseline.self);change(d.payload.delta.own.statuses[0]);assert.throws(()=>r.apply(d),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,baseline);}
});
test('compound cleanup removes flags and numeric values in the same own replacement',()=>{
 const f=full(),s=f.payload.snapshot;s.self.shield_total=0;const status=flaggedStatus();status.flags=['damage_immunity'];s.self.statuses=[status];const r=new ReplicaV6(key,'A');r.apply(f);
 const d=delta();d.payload.delta.own={...s.self,statuses:[]};assert.deepEqual(r.apply(d).self.statuses,[]);
});

test('native HOT cleanse capabilities and end recovery share the original status and delta',()=>{
 const f=full(),s=f.payload.snapshot;s.self.shield_total=0;s.self.statuses=[{instance_id:'clean-1',definition_id:'renewal',source:entity,target:entity,applied_at_ms:0,expires_at_ms:3900,modifier_bps:0,periodic:{kind:'max_hp_heal',interval_ms:1000,next_tick_at_ms:1000,ticks_done:0,total_ticks:3,raw_heal:10},cleanse:{on_tick:true,on_expired:true}}];
 const r=new ReplicaV6(key,'A');r.apply(f);const d=delta();d.payload.delta.own=structuredClone(s.self);d.payload.delta.own.statuses[0].periodic.ticks_done=1;d.payload.delta.own.statuses[0].periodic.next_tick_at_ms=2000;
 assert.deepEqual(r.apply(d).self.statuses[0].cleanse,{on_tick:true,on_expired:true});
});
test('cleanse metadata rejects malformed clocks empty hooks and unsupported carriers atomically',()=>{
 const f=full(),s=f.payload.snapshot;s.self.shield_total=0;const r=new ReplicaV6(key,'A'),baseline=r.apply(f);
 const row={instance_id:'clean-1',definition_id:'clean',source:entity,target:entity,applied_at_ms:0,expires_at_ms:3900,modifier_bps:0,cleanse:{on_tick:false,on_expired:true}};
 for(const change of [s=>s.cleanse=null,s=>s.cleanse.on_tick='yes',s=>s.cleanse.on_expired=null,s=>s.cleanse.on_expired=false,s=>s.cleanse.next_tick_at_ms=1000,s=>s.cleanse.on_tick=true,s=>s.stacks=1,s=>s.max_stacks=1,s=>s.shield={granted:0,remaining:0},s=>s.periodic={kind:'physical_damage',interval_ms:300,next_tick_at_ms:300,ticks_done:0,total_ticks:3,raw_damage:1}]){
  const d=delta(),status=structuredClone(row);change(status);d.payload.delta.own={...baseline.self,statuses:[status]};assert.throws(()=>r.apply(d),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,baseline);
 }
});
test('ally HOT cleanse metadata preserves original source generation and rejects stale attribution',()=>{
 const f=full(),s=f.payload.snapshot,ally={...entity,id:'B',generation:2};s.members.push({...s.members[0],entity:ally});s.self.shield_total=0;
 s.self.statuses=[{instance_id:'ally-clean',definition_id:'renewal',source:ally,target:entity,applied_at_ms:0,expires_at_ms:3900,modifier_bps:0,periodic:{kind:'max_hp_heal',interval_ms:1000,next_tick_at_ms:1000,ticks_done:0,total_ticks:3,raw_heal:10},cleanse:{on_tick:true,on_expired:true},window:{...windowRecordFixture(),basis:'none',recorded:0,cap:0,contacts:0}}];
 const r=new ReplicaV6(key,'A');assert.equal(r.apply(f).self.statuses[0].source.generation,2);const d=delta();d.payload.delta.own=structuredClone(s.self);d.payload.delta.own.statuses[0].source.generation++;assert.throws(()=>r.apply(d),code('INVALID_FRAME'));
});

function resourceHotFrame(){const f=full();f.payload.snapshot.self.shield_total=0;f.payload.snapshot.self.statuses=[{instance_id:'resource-hot',definition_id:'energy',source:entity,target:entity,applied_at_ms:0,expires_at_ms:3900,modifier_bps:0,periodic:{kind:'max_hp_heal',interval_ms:1000,next_tick_at_ms:1000,ticks_done:0,total_ticks:3,raw_heal:66,raw_spirit:2,power_sampling:'source_live'}}];return f;}
test('pending ally self-target requires a matching marker and current generation',()=>{
 const f=full(), snapshot=f.payload.snapshot, target={id:entity.id,generation:entity.generation};
 snapshot.self.player_skills=[{skill_id:'heal',ready_at_ms:0}];
 snapshot.self.pending_action={skill_id:'heal',commits_at_ms:120,ally_target:target,ally_target_is_self:true};
 const replica=new ReplicaV6(key,'A'), baseline=replica.apply(f);
 assert.deepEqual(baseline.self.pending_action.ally_target,target);
 for(const corrupt of [
  p=>delete p.ally_target_is_self,p=>p.ally_target_is_self=false,p=>p.ally_target_is_self=null,
  p=>p.ally_target.generation++,p=>delete p.ally_target,
 ]){
  const d=delta();
  d.payload.delta.own=structuredClone(baseline.self);
  corrupt(d.payload.delta.own.pending_action);
  assert.throws(()=>replica.apply(d),code('INVALID_FRAME'));
  assert.deepEqual(replica.snapshot,baseline);
 }
});
test('fixed HOT can publish the expiry tick, with strict clock and type checks',()=>{
 const f=resourceHotFrame(), status=f.payload.snapshot.self.statuses[0], p=status.periodic;
 status.expires_at_ms=3000;
 delete p.raw_spirit;
 delete p.power_sampling;
 p.tick_on_expiry=true;
 const replica=new ReplicaV6(key,'A'), baseline=replica.apply(f);
 for(const invalid of [s=>s.periodic.tick_on_expiry=false,s=>s.periodic.raw_spirit=1,s=>s.expires_at_ms=3001,s=>s.periodic.total_ticks=2]){
  const d=delta();
  d.payload.delta.own=structuredClone(baseline.self);
  invalid(d.payload.delta.own.statuses[0]);
  assert.throws(()=>replica.apply(d),code('INVALID_FRAME'));
  assert.deepEqual(replica.snapshot,baseline);
 }
 const d=delta();
 d.payload.delta.own=structuredClone(baseline.self);
 Object.assign(d.payload.delta.own.statuses[0].periodic,{ticks_done:2,next_tick_at_ms:3000});
 assert.equal(replica.apply(d).self.statuses[0].periodic.next_tick_at_ms,3000);
});
test('periodic spirit amounts and live sampling survive a native HOT delta',()=>{
 const f=resourceHotFrame(),r=new ReplicaV6(key,'A');r.apply(f);const d=delta();d.payload.delta.own=structuredClone(f.payload.snapshot.self);const p=d.payload.delta.own.statuses[0].periodic;p.raw_spirit=4;p.ticks_done=1;p.next_tick_at_ms=2000;assert.equal(r.apply(d).self.statuses[0].periodic.raw_spirit,4);
});
test('periodic decoder accepts all four typed sampling modes without inventing a second clock',()=>{
 for(const mode of ['source_snapshot','target_snapshot','source_live','target_live']){const f=resourceHotFrame();f.payload.snapshot.self.statuses[0].periodic.power_sampling=mode;assert.equal(new ReplicaV6(key,'A').apply(f).self.statuses[0].periodic.power_sampling,mode);}
});
test('periodic recovery rejects malformed sampling and spirit values atomically',()=>{
 const f=resourceHotFrame(),r=new ReplicaV6(key,'A'),baseline=r.apply(f);
 for(const change of [p=>p.raw_spirit=null,p=>p.raw_spirit=-1,p=>p.raw_spirit=2**32,p=>p.raw_spirit=1.5,p=>p.power_sampling=null,p=>p.power_sampling='unknown',p=>{p.kind='physical_damage';p.raw_damage=10;delete p.raw_heal;},p=>p.next_spirit_tick_ms=1000]){const d=delta();d.payload.delta.own=structuredClone(baseline.self);change(d.payload.delta.own.statuses[0].periodic);assert.throws(()=>r.apply(d),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,baseline);}
});
test('damage sampling accepts typed damage kinds while refusing healing or spirit fields',()=>{
 for(const kind of ['physical_damage','elemental_damage','true_damage']){const f=periodicFull();const p=f.payload.snapshot.monsters[0].statuses[0].periodic;p.kind=kind;p.power_sampling='target_live';const r=new ReplicaV6(key,'A');r.apply(f);const d=delta();d.payload.delta.monsters={upsert:[structuredClone(f.payload.snapshot.monsters[0])],removed:[],order:[npc]};d.payload.delta.monsters.upsert[0].statuses[0].periodic.raw_spirit=0;assert.throws(()=>r.apply(d),code('INVALID_FRAME'));}
});

test('HOT excess shield records preserve ally source and frozen balance across a fresh full baseline',()=>{
 const f=resourceHotFrame(),s=f.payload.snapshot,ally={...entity,id:'B',generation:2};s.members.push({...s.members[0],entity:ally});s.self.statuses[0].source=ally;
 const overflow={instance_id:'overflow-1',definition_id:'overflow-shield',source:ally,target:entity,applied_at_ms:3000,expires_at_ms:6000,modifier_bps:0,shield:{granted:96,remaining:96,record:{basis:'incoming',recorded:0,cap:40}},modifiers:[{attribute:'damage_reduction',modifier_bps:2000}]};
 s.self.statuses.push(overflow);s.self.shield_total=96;
 const first=new ReplicaV6(key,'A').apply(f);assert.equal(first.self.statuses[1].source.generation,2);
 const resumed=new ReplicaV6(key,'A').apply(structuredClone(f));assert.deepEqual(resumed.self.statuses[1],overflow);
});
test('record shield refills and depletion share the ordinary balance decoder and invalid deltas stay atomic',()=>{
 const f=shieldFull(),s=f.payload.snapshot;const status=s.self.statuses[0];status.shield.record={basis:'own_absorbed',recorded:4,cap:10};status.modifiers=[{attribute:'damage_reduction',modifier_bps:1000}];
 const replica=new ReplicaV6(key,'A');const baseline=replica.apply(f);
 const invalid=delta();invalid.payload.delta.own=structuredClone(baseline.self);invalid.payload.delta.own.statuses[0].shield.record.recorded=11;
 assert.throws(()=>replica.apply(invalid),code('INVALID_FRAME'));assert.deepEqual(replica.snapshot,baseline);
 const depleted=delta();depleted.payload.delta.own={...baseline.self,statuses:[],shield_total:0};assert.deepEqual(replica.apply(depleted).self.statuses,[]);
});

test('shield transfer delta removes old NPC projection and retains frozen record on the new holder',()=>{
 const f=shieldFull(),s=f.payload.snapshot,offered=s.monsters[0].statuses[0];offered.shield.record={basis:'own_absorbed',recorded:12,cap:180};
 const replica=new ReplicaV6(key,'A');replica.apply(f);const moved={...structuredClone(offered),target:entity};
 const d=delta();d.payload.delta.own={...s.self,statuses:[moved],shield_total:moved.shield.remaining};
 const monster={...s.monsters[0],statuses:[],shield_total:0};d.payload.delta.monsters={upsert:[monster],removed:[],order:[npc]};
 const after=replica.apply(d);assert.deepEqual(after.self.statuses,[moved]);assert.deepEqual(after.monsters[0].statuses,[]);
});
test('transferred NPC sourced shields reject stale attribution and malformed balances atomically',()=>{
 const f=shieldFull(),s=f.payload.snapshot;s.self.statuses[0].source=npc;s.self.statuses[0].shield.record={basis:'incoming',recorded:4,cap:30};
 const replica=new ReplicaV6(key,'A'),baseline=replica.apply(f);
 for(const change of [row=>row.source.generation++,row=>row.target=npc,row=>row.shield.remaining=row.shield.granted+1,row=>row.shield.record.recorded=31]){
  const d=delta();d.payload.delta.own=structuredClone(baseline.self);change(d.payload.delta.own.statuses[0]);assert.throws(()=>replica.apply(d),code('INVALID_FRAME'));assert.deepEqual(replica.snapshot,baseline);
 }
});

test('transferred NPC HOT retains original attribution sampling and dual resource metadata through a fresh baseline',()=>{
 const f=resourceHotFrame();f.payload.snapshot.monsters=shieldFull().payload.snapshot.monsters;f.payload.snapshot.self.statuses[0].source=npc;
 const replica=new ReplicaV6(key,'A'),first=replica.apply(f);assert.equal(first.self.statuses[0].periodic.raw_spirit,2);
 const d=delta();d.payload.delta.own=structuredClone(first.self);d.payload.delta.own.statuses[0].periodic.ticks_done=1;d.payload.delta.own.statuses[0].periodic.next_tick_at_ms=2000;
 assert.equal(replica.apply(d).self.statuses[0].source.id,npc.id);
 assert.deepEqual(new ReplicaV6(key,'A').apply(f).self.statuses[0],first.self.statuses[0]);
});
test('transferred DOT views keep typed damage while malformed target or healing metadata cannot pollute the replica',()=>{
 const f=resourceHotFrame(),row=f.payload.snapshot.self.statuses[0];f.payload.snapshot.monsters=shieldFull().payload.snapshot.monsters;row.source=npc;row.periodic.kind='true_damage';row.periodic.raw_damage=10;delete row.periodic.raw_heal;delete row.periodic.raw_spirit;
 const replica=new ReplicaV6(key,'A'),baseline=replica.apply(f);
 for(const change of [s=>s.source.generation++,s=>s.target=npc,s=>s.periodic.raw_heal=1,s=>s.periodic.raw_spirit=1]){const d=delta();d.payload.delta.own=structuredClone(baseline.self);change(d.payload.delta.own.statuses[0]);assert.throws(()=>replica.apply(d),code('INVALID_FRAME'));assert.deepEqual(replica.snapshot,baseline);}
});

test('transferred friendly scalar immunities accept native zero modifiers and reject malformed carriers atomically',()=>{
 const f=full(),s=f.payload.snapshot,ally={...entity,id:'B',generation:2};s.members.push({...s.members[0],entity:ally});s.self.shield_total=0;
 s.self.statuses=[{instance_id:'immune-moved',definition_id:'guard',source:ally,target:entity,attribute:'damage_immunity',modifier_bps:0,applied_at_ms:0,expires_at_ms:1500}];
 const replica=new ReplicaV6(key,'A'),baseline=replica.apply(f);
 for(const change of [s=>s.modifier_bps=1,s=>s.modifiers=[{attribute:'atk',modifier_bps:1000}],s=>s.flags=['damage_immunity'],s=>s.stacks=1,s=>s.window=windowRecordFixture(),s=>s.attribute='stun',s=>s.source.generation++]){const d=delta();d.payload.delta.own=structuredClone(baseline.self);change(d.payload.delta.own.statuses[0]);assert.throws(()=>replica.apply(d),code('INVALID_FRAME'));assert.deepEqual(replica.snapshot,baseline);}
});
test('transferred compound immunity groups retain identity and reject friendly hard control or mixed groups',()=>{
 const f=full(),s=f.payload.snapshot,ally={...entity,id:'B',generation:2};s.members.push({...s.members[0],entity:ally});s.self.shield_total=0;
 s.self.statuses=[{...flaggedStatus(ally),flags:['control_immunity','damage_immunity','first_control_immunity']}];
 const replica=new ReplicaV6(key,'A'),baseline=replica.apply(f);
 for(const flags of [['stun'],['root','damage_immunity'],['control_immunity','control_immunity']]){const d=delta();d.payload.delta.own=structuredClone(baseline.self);d.payload.delta.own.statuses[0].flags=flags;assert.throws(()=>replica.apply(d),code('INVALID_FRAME'));assert.deepEqual(replica.snapshot,baseline);}
 const d=delta();d.payload.delta.own={...baseline.self,statuses:[]};assert.deepEqual(replica.apply(d).self.statuses,[]);
});

test('NPC control transferred between members keeps typed compound flags and rejects malformed delta atomically',()=>{
 const f=shieldFull(),s=f.payload.snapshot;s.self.statuses=[{...flaggedStatus(npc),flags:['silence','root']}];s.self.shield_total=0;
 const replica=new ReplicaV6(key,'A'),baseline=replica.apply(f);assert.equal(baseline.self.statuses[0].source.id,npc.id);
 for(const change of [r=>r.source.generation++,r=>r.target=npc,r=>r.flags=['root','damage_immunity'],r=>r.flags=['stun','stun'],r=>r.modifier_bps=1]){
  const d=delta();d.payload.delta.own=structuredClone(baseline.self);change(d.payload.delta.own.statuses[0]);assert.throws(()=>replica.apply(d),code('INVALID_FRAME'));assert.deepEqual(replica.snapshot,baseline);
 }
 const d=delta();d.payload.delta.own={...baseline.self,statuses:[]};assert.deepEqual(replica.apply(d).self.statuses,[]);
});
test('NPC scalar hard control and spirit lock require native values without mixed carriers',()=>{
 for(const attribute of ['silence','root','stun','spirit_recovery_lock']){
  const f=shieldFull(),s=f.payload.snapshot;s.self.shield_total=0;s.self.statuses=[{instance_id:'npc-control',definition_id:'gift',source:npc,target:entity,attribute,modifier_bps:attribute==='spirit_recovery_lock'?-10000:0,applied_at_ms:0,expires_at_ms:1500}];
  const replica=new ReplicaV6(key,'A'),baseline=replica.apply(f);
  for(const change of [r=>r.modifier_bps=1,r=>r.modifiers=[{attribute:'def',modifier_bps:-2000}],r=>r.flags=['root'],r=>r.stacks=1,r=>r.window=windowRecordFixture()]){const d=delta();d.payload.delta.own=structuredClone(baseline.self);change(d.payload.delta.own.statuses[0]);assert.throws(()=>replica.apply(d),code('INVALID_FRAME'));assert.deepEqual(replica.snapshot,baseline);}
 }
});

test('moved HOT window and cleanse keep original metadata and scheduling through delta and new baseline',()=>{
 const f=resourceHotFrame(),s=f.payload.snapshot,ally={...entity,id:'B',generation:2};s.members.push({...s.members[0],entity:ally});const row=s.self.statuses[0];row.source=ally;row.cleanse={on_tick:true,on_expired:true};row.window={...windowRecordFixture(),basis:'none',recorded:0,cap:0,contacts:0};row.periodic.ticks_done=1;row.periodic.next_tick_at_ms=2000;
 const replica=new ReplicaV6(key,'A'),baseline=replica.apply(f);assert.deepEqual(new ReplicaV6(key,'A').apply(f),baseline);
 const d=delta();d.payload.delta.own=structuredClone(baseline.self);d.payload.delta.own.statuses[0].periodic.ticks_done=2;d.payload.delta.own.statuses[0].periodic.next_tick_at_ms=3000;const advanced=replica.apply(d);assert.deepEqual(advanced.self.statuses[0].window,row.window);assert.deepEqual(advanced.self.statuses[0].cleanse,row.cleanse);assert.equal(advanced.self.statuses[0].expires_at_ms,3900);
});
test('moved NPC window with cleanse rejects malformed records carriers and attribution atomically',()=>{
 const f=shieldFull(),s=f.payload.snapshot;s.self.shield_total=0;s.self.statuses=[{instance_id:'moved-window',definition_id:'cargo',source:npc,target:entity,attribute:'def',modifier_bps:1,applied_at_ms:0,expires_at_ms:4200,window:windowRecordFixture(),cleanse:{on_tick:false,on_expired:true}}];
 const replica=new ReplicaV6(key,'A'),baseline=replica.apply(f);
 for(const change of [r=>r.source.generation++,r=>r.target=npc,r=>r.window.recorded=2001,r=>r.window.contacts=0,r=>r.window.max_hp_at_apply=0,r=>r.stacks=1,r=>r.max_stacks=2,r=>r.cleanse={on_tick:false,on_expired:false},r=>r.cleanse.on_tick=true]){const d=delta();d.payload.delta.own=structuredClone(baseline.self);change(d.payload.delta.own.statuses[0]);assert.throws(()=>replica.apply(d),code('INVALID_FRAME'));assert.deepEqual(replica.snapshot,baseline);}
});

test('control replacement uses one compound numeric identity with movement slow and interval delay',()=>{
 const f=shieldFull(),s=f.payload.snapshot;const n=s.monsters[0];n.statuses=[{instance_id:'softened',definition_id:'replacement',source:entity,target:n.entity,modifier_bps:0,applied_at_ms:0,expires_at_ms:1800,modifiers:[{attribute:'movement_slow',modifier_bps:-5000},{attribute:'basic_interval_delay',modifier_bps:1500}]}];n.shield_total=0;s.self.statuses=[];s.self.shield_total=0;
 const replica=new ReplicaV6(key,'A'),baseline=replica.apply(f);assert.equal(baseline.monsters[0].statuses[0].modifiers.length,2);
 for(const change of [r=>r.modifiers[0].modifier_bps=0,r=>r.modifiers[1].modifier_bps=-1500,r=>r.stacks=2,r=>r.modifiers.push(r.modifiers[0])]){const d=delta();d.payload.delta.monsters={upsert:[structuredClone(baseline.monsters[0])],removed:[],order:[npc]};change(d.payload.delta.monsters.upsert[0].statuses[0]);assert.throws(()=>replica.apply(d),code('INVALID_FRAME'));assert.deepEqual(replica.snapshot,baseline);}
});
test('scalar basic interval delay validates positive native bps and rejects malformed carriers',()=>{
 const f=shieldFull(),s=f.payload.snapshot;s.self.statuses=[{instance_id:'interval',definition_id:'delay',source:npc,target:entity,attribute:'basic_interval_delay',modifier_bps:1500,applied_at_ms:0,expires_at_ms:1800}];s.self.shield_total=0;const replica=new ReplicaV6(key,'A'),baseline=replica.apply(f);
 for(const change of [r=>r.modifier_bps=0,r=>r.modifier_bps=-1,r=>r.modifier_bps=100001,r=>r.flags=['root'],r=>r.modifiers=[{attribute:'basic_interval_delay',modifier_bps:1500}]]){const d=delta();d.payload.delta.own=structuredClone(baseline.self);change(d.payload.delta.own.statuses[0]);assert.throws(()=>replica.apply(d),code('INVALID_FRAME'));assert.deepEqual(replica.snapshot,baseline);}
});

test('source-owned slow stacks preserve one identity through delta and new baseline',()=>{
 const f=shieldFull(),s=f.payload.snapshot,n=s.monsters[0];s.self.statuses=[];s.self.shield_total=0;n.shield_total=0;n.statuses=[{instance_id:'layered-slow',definition_id:'mark',source:entity,target:n.entity,attribute:'movement_slow',modifier_bps:-800,stacks:2,max_stacks:3,applied_at_ms:0,expires_at_ms:6000}];
 const replica=new ReplicaV6(key,'A'),baseline=replica.apply(f);assert.deepEqual(new ReplicaV6(key,'A').apply(f),baseline);
 const d=delta();d.payload.delta.monsters={upsert:[{...structuredClone(n),statuses:[]}],removed:[],order:[npc]};assert.deepEqual(replica.apply(d).monsters[0].statuses,[]);
});
test('slow stacks reject invalid strength layers and mixed carriers without changing the cache',()=>{
 const f=shieldFull(),s=f.payload.snapshot,n=s.monsters[0];s.self.statuses=[];s.self.shield_total=0;n.shield_total=0;n.statuses=[{instance_id:'layered-slow',definition_id:'mark',source:entity,target:n.entity,attribute:'movement_slow',modifier_bps:-800,stacks:2,max_stacks:3,applied_at_ms:0,expires_at_ms:6000}];
 const replica=new ReplicaV6(key,'A'),baseline=replica.apply(f);
 for(const change of [r=>r.stacks=4,r=>delete r.max_stacks,r=>r.modifier_bps=0,r=>r.modifier_bps=-10000,r=>r.modifiers=[{attribute:'def',modifier_bps:-1000}],r=>r.flags=['root']]){const d=delta();d.payload.delta.monsters={upsert:[structuredClone(n)],removed:[],order:[npc]};change(d.payload.delta.monsters.upsert[0].statuses[0]);assert.throws(()=>replica.apply(d),code('INVALID_FRAME'));assert.deepEqual(replica.snapshot,baseline);}
});

test('queued early release accepts only a locked minimum deadline and rolls back invalid frames',()=>{
  const valid=chargeFull(),p=valid.payload.snapshot.self.pending_action;
  p.commits_at_ms=120;p.charge.released_at_ms=30;
  const r=new ReplicaV6(key,'A'),before=r.apply(valid);
  for(const change of [p=>p.charge.released_at_ms=null,p=>p.charge.released_at_ms=31,p=>p.charge.released_at_ms=29,p=>p.commits_at_ms=150,p=>p.charge.aim={direction:[1,0],updated_at_ms:31}]){
    const f=structuredClone(valid);f.revision=2;f.publication_id='bad-queued-charge';change(f.payload.snapshot.self.pending_action);
    assert.throws(()=>r.apply(f),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);
  }
});

test('explicit channel count accepts endpoint scheduling and rejects partial or impossible clocks',()=>{
  const f=channelFull();f.payload.snapshot.logical_ms=270;
  Object.assign(f.payload.snapshot.self.active_channel,{total_pulses:3,pulses:2,next_pulse_at_ms:390});
  const r=new ReplicaV6(key,'A'),before=r.apply(f);
  for(const change of [c=>c.total_pulses=null,c=>c.total_pulses=0,c=>c.total_pulses=129,c=>c.pulses=3,c=>c.next_pulse_at_ms=null,c=>c.next_pulse_at_ms=389,c=>c.ends_at_ms=391]) {
    const bad=structuredClone(f);bad.revision=2;bad.publication_id='bad-count-channel';change(bad.payload.snapshot.self.active_channel);
    assert.throws(()=>r.apply(bad),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);
  }
});

test('release windup accepts early and later locked requests and rejects inconsistent promises',()=>{
  for(const [logical,release,commit] of [[30,30,210],[200,180,270]]) {
    const f=chargeFull();f.payload.snapshot.logical_ms=logical;const p=f.payload.snapshot.self.pending_action;
    Object.assign(p.charge,{release_windup_ms:90,released_at_ms:release});p.commits_at_ms=commit;
    const r=new ReplicaV6(key,'A'),before=r.apply(f);
    for(const change of [p=>p.charge.release_windup_ms=null,p=>p.charge.release_windup_ms=0,p=>p.charge.release_windup_ms=5001,p=>p.commits_at_ms--,p=>delete p.charge.released_at_ms]) {
      const bad=structuredClone(f);bad.revision=2;bad.publication_id='bad-release-windup';change(bad.payload.snapshot.self.pending_action);
      assert.throws(()=>r.apply(bad),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);
    }
  }
});

test('nonspatial recovery channel declares its kind without a projectile direction or hidden budget',()=>{
  const f=channelFull();const c=f.payload.snapshot.self.active_channel;
  Object.assign(c,{recovery:true,total_pulses:3,direction:[0,0],next_pulse_at_ms:150});
  const r=new ReplicaV6(key,'A'),before=r.apply(f);
  for(const change of [c=>c.recovery=null,c=>c.recovery=false,c=>c.direction=[1,0],c=>delete c.total_pulses,c=>c.raw_spirit=99]) {
    const bad=structuredClone(f);bad.revision=2;bad.publication_id='bad-recovery-kind';change(bad.payload.snapshot.self.active_channel);
    assert.throws(()=>r.apply(bad),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);
  }
});

function layeredPeriodicFull(){
 const f=periodicFull(),s=f.payload.snapshot.monsters[0].statuses[0];
 Object.assign(s,{applied_at_ms:300,expires_at_ms:2100,stacks:2,max_stacks:10,modifiers:[{attribute:'healing_received',modifier_bps:-300}]});
 Object.assign(s.periodic,{interval_ms:500,first_tick_at_ms:500,next_tick_at_ms:500,total_ticks:4,per_stack:true,raw_damage:6});return f;
}
test('layered periodic baseline declares raw per layer and exact preserved first tick through full and delta',()=>{
 const r=new ReplicaV6(key,'A'),f=layeredPeriodicFull(),baseline=r.apply(f);assert.equal(baseline.monsters[0].statuses[0].stacks,2);
 const d=delta(),n=structuredClone(baseline.monsters[0]);d.payload.delta.logical_ms=510;n.statuses[0].periodic.ticks_done=1;n.statuses[0].periodic.next_tick_at_ms=1000;d.payload.delta.monsters={upsert:[n],removed:[],order:[npc]};
 assert.equal(r.apply(d).monsters[0].statuses[0].periodic.raw_damage,6);
});
test('layered periodic rejects contradictory stack and clock metadata without mutating baseline',()=>{
 const r=new ReplicaV6(key,'A'),baseline=r.apply(layeredPeriodicFull());
 for(const change of [s=>delete s.stacks,s=>delete s.max_stacks,s=>s.stacks=11,s=>s.max_stacks=33,s=>s.periodic.per_stack=false,s=>s.periodic.per_stack=null,s=>delete s.periodic.per_stack,s=>s.periodic.first_tick_at_ms=null,s=>delete s.periodic.first_tick_at_ms,s=>s.periodic.first_tick_at_ms=299,s=>s.periodic.first_tick_at_ms=801,s=>s.periodic.total_ticks=3,s=>s.periodic.next_tick_at_ms=1000,s=>s.periodic.ticks_done=1,s=>s.periodic.inputs={attack:99},s=>s.periodic.raw_heal=0,s=>s.flags=['control_immunity']]){
  const d=delta(),n=structuredClone(baseline.monsters[0]);change(n.statuses[0]);d.payload.delta.monsters={upsert:[n],removed:[],order:[npc]};assert.throws(()=>r.apply(d),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,baseline);
 }
});
test('shortened periodic clock has zero ticks only when its exact expiry precedes first tick',()=>{
 const f=layeredPeriodicFull(),s=f.payload.snapshot.monsters[0].statuses[0];s.expires_at_ms=480;s.periodic.total_ticks=0;s.periodic.next_tick_at_ms=null;
 const r=new ReplicaV6(key,'A');assert.equal(r.apply(f).monsters[0].statuses[0].periodic.total_ticks,0);
 const d=delta(),n=structuredClone(f.payload.snapshot.monsters[0]);n.statuses[0].expires_at_ms=501;d.payload.delta.monsters={upsert:[n],removed:[],order:[npc]};assert.throws(()=>r.apply(d),code('INVALID_FRAME'));
});
test('HOT immunity and numeric modifier are one ally support identity and cannot carry hostile control',()=>{
 const f=full(),s=f.payload.snapshot;s.self.statuses=[{instance_id:'hot-compound',definition_id:'hot',source:entity,target:entity,applied_at_ms:0,expires_at_ms:1800,modifier_bps:0,modifiers:[{attribute:'atk',modifier_bps:2000},{attribute:'movement_haste',modifier_bps:1000}],flags:['first_control_immunity'],periodic:{kind:'max_hp_heal',interval_ms:500,next_tick_at_ms:500,ticks_done:0,total_ticks:3,raw_heal:17}}];s.self.shield_total=0;
 const r=new ReplicaV6(key,'A'),baseline=r.apply(f);assert.equal(baseline.self.statuses[0].modifiers.length,2);
 for(const change of [s=>s.flags=['root'],s=>s.flags.push('root'),s=>s.modifiers[1].modifier_bps=-1]){const d=delta();d.payload.delta.own=structuredClone(baseline.self);change(d.payload.delta.own.statuses[0]);assert.throws(()=>r.apply(d),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,baseline);}
});
test('allied compound numeric layers retain the configured legacy128 limit while periodic layers remain32',()=>{
 const f=full(),s=f.payload.snapshot,b={kind:'participant',id:'B',generation:1};s.members.push({entity:b,position:[20,20],hp:100,max_hp:100,phase:'active',protected:false});s.self.statuses=[{instance_id:'ally-stacks',definition_id:'stacked-atk',source:b,target:entity,applied_at_ms:0,expires_at_ms:1800,modifier_bps:0,stacks:64,max_stacks:128,modifiers:[{attribute:'atk',modifier_bps:1}]}];s.self.shield_total=0;
 const r=new ReplicaV6(key,'A');assert.equal(r.apply(f).self.statuses[0].stacks,64);
 const d=delta();d.payload.delta.own=structuredClone(s.self);d.payload.delta.own.statuses[0].max_stacks=129;assert.throws(()=>r.apply(d),code('INVALID_FRAME'));
});
test('member loadout revisions replace ordered slots and accept zero equipped skills through delta',()=>{
 const r=new ReplicaV6(key,'A'),f=slotsFull();r.apply(f);
 const d=delta();d.payload.delta.own=structuredClone(f.payload.snapshot.self);d.payload.delta.own.player_skills=[d.payload.delta.own.player_skills[7],d.payload.delta.own.player_skills[0]];d.payload.delta.own.skill_loadout_revision=1;
 assert.deepEqual(r.apply(d).self.player_skills.map(s=>s.skill_id),['slot-7','slot-0']);
 const empty=structuredClone(d);empty.revision=3;empty.publication_id='empty-3';empty.payload.base_revision=2;empty.payload.base_publication_id=d.publication_id;empty.payload.delta.own.skill_loadout_revision=2;empty.payload.delta.own.player_skills=[];
 assert.deepEqual(r.apply(empty).self.player_skills,[]);
});
test('member loadout revision shape rollback and same-version slot edits preserve baseline',()=>{
 const f=slotsFull();f.payload.snapshot.self.skill_loadout_revision=2;
 for(const change of [s=>s.skill_loadout_revision=null,s=>s.skill_loadout_revision=-1,s=>s.skill_loadout_revision=2**32,s=>s.skill_loadout_revision='3',s=>s.skill_loadout_revision=1,s=>delete s.skill_loadout_revision,s=>s.player_skills.reverse(),s=>s.player_skills=[]]){
  const r=new ReplicaV6(key,'A'),before=r.apply(f),d=delta();d.payload.delta.own=structuredClone(before.self);change(d.payload.delta.own);assert.throws(()=>r.apply(d),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);
 }
});
test('reconnected full frame keeps the latest private loadout revision',()=>{
 const f=slotsFull();f.payload.snapshot.self.skill_loadout_revision=4;const r=new ReplicaV6(key,'A'),before=r.apply(f);
 const next=structuredClone(f);next.publication_id='reconnect-loadout';next.connection={id:'new',generation:2};next.stream_epoch=2;next.revision=3;assert.deepEqual(r.apply(next).self,before.self);
 const bad=structuredClone(next);bad.publication_id='revoked-4';bad.revision=4;bad.payload.snapshot.self.skill_loadout_revision=3;assert.throws(()=>r.apply(bad),code('INVALID_FRAME'));assert.deepEqual(r.snapshot.self,before.self);
});
function targetChannelFull(ally){const f=periodicFull(),s=f.payload.snapshot,b={kind:'participant',id:'B',generation:1};s.members.push({entity:b,position:[20,20],hp:100,max_hp:100,phase:'active',protected:false});s.self.player_skills=[{skill_id:'unit',ready_at_ms:1000}];s.self.active_channel={action_id:'unit-chan',skill_id:'unit',direction:ally?[0,0]:[1,0],started_at_ms:0,ends_at_ms:1200,next_pulse_at_ms:400,pulses:0,total_pulses:3,target:ally?b:npc};if(ally)s.self.active_channel.recovery=true;return f;}
test('bound unit channels reconstruct friendly and hostile identities through full and delta',()=>{for(const ally of [true,false]){const f=targetChannelFull(ally),r=new ReplicaV6(key,'A'),s=r.apply(f);assert.deepEqual(s.self.active_channel.target,ally?s.members[1].entity:npc);const d=delta();d.payload.delta.logical_ms=420;d.payload.delta.own=structuredClone(s.self);Object.assign(d.payload.delta.own.active_channel,{pulses:1,next_pulse_at_ms:800});assert.equal(r.apply(d).self.active_channel.pulses,1);}});
test('unit channel rejects unknown target wrong relation and clocks atomically',()=>{for(const ally of [true,false])for(const change of [c=>c.target=null,c=>c.target.generation=2,c=>c.target.kind=ally?'monster':'participant',c=>c.target.id='unknown',c=>delete c.total_pulses,c=>c.total_pulses=4,c=>c.next_pulse_at_ms=300,c=>c.target.extra=true]){const r=new ReplicaV6(key,'A'),before=r.apply(targetChannelFull(ally)),d=delta();d.payload.delta.own=structuredClone(before.self);change(d.payload.delta.own.active_channel);assert.throws(()=>r.apply(d),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);}});
test('same active unit action cannot retarget another valid ally or erase its binding',()=>{for(const erase of [true,false]){const f=targetChannelFull(true),s=f.payload.snapshot,c={kind:'participant',id:'C',generation:1};s.members.push({entity:c,position:[30,30],hp:100,max_hp:100,phase:'active',protected:false});const r=new ReplicaV6(key,'A'),before=r.apply(f),d=delta();d.payload.delta.own=structuredClone(before.self);if(erase)delete d.payload.delta.own.active_channel.target;else d.payload.delta.own.active_channel.target=c;assert.throws(()=>r.apply(d),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);}});
function reservationFull(){const f=slotsFull();f.payload.snapshot.self.pending_action={skill_id:'slot-0',commits_at_ms:300,action_id:'reserve-action',resource_reservation:{spirit:10,hp:50,minimum_hp_after_cost:1}};return f;}
test('reservation is private metadata with no early debit and clears on cancellation delta',()=>{const r=new ReplicaV6(key,'A'),before=r.apply(reservationFull());assert.equal(before.self.hp,100);assert.equal(before.self.pending_action.resource_reservation.hp,50);const d=delta();d.payload.delta.own=structuredClone(before.self);d.payload.delta.own.pending_action=null;assert.equal(r.apply(d).self.hp,100);assert.equal(r.snapshot.self.pending_action,null);});
test('reservation requires a real skill action and strict bounded resource amounts',()=>{for(const change of [p=>p.action_id=null,p=>delete p.action_id,p=>p.skill_id=null,p=>p.resource_reservation=null,p=>p.resource_reservation.hp=-1,p=>p.resource_reservation.spirit=2**32,p=>p.resource_reservation.minimum_hp_after_cost=0,p=>p.resource_reservation.extra=1,p=>p.untrusted=true]){const f=reservationFull();change(f.payload.snapshot.self.pending_action);assert.throws(()=>new ReplicaV6(key,'A').apply(f),code('INVALID_FRAME'));}});
test('same preparation cannot change or erase its frozen reservation through delta or full',()=>{for(const change of [p=>p.resource_reservation.hp++,p=>p.resource_reservation.spirit++,p=>p.resource_reservation.minimum_hp_after_cost++,p=>delete p.resource_reservation,p=>delete p.action_id]){const r=new ReplicaV6(key,'A'),before=r.apply(reservationFull()),d=delta();d.payload.delta.own=structuredClone(before.self);change(d.payload.delta.own.pending_action);assert.throws(()=>r.apply(d),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);const f=reservationFull();f.publication_id='bad-full-2';f.revision=2;change(f.payload.snapshot.self.pending_action);assert.throws(()=>r.apply(f),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);}});
test('reservation cannot erase both metadata fields for the same skill but permits a replacement legacy skill',()=>{const r=new ReplicaV6(key,'A'),before=r.apply(reservationFull()),d=delta();d.payload.delta.own=structuredClone(before.self);delete d.payload.delta.own.pending_action.action_id;delete d.payload.delta.own.pending_action.resource_reservation;assert.throws(()=>r.apply(d),code('INVALID_FRAME'));d.payload.delta.own.pending_action.skill_id='slot-1';assert.equal(r.apply(d).self.pending_action.skill_id,'slot-1');});

test('live life-steal scalar layers and compound signed reductions survive full and delta',()=>{
 const f=full(),r=new ReplicaV6(key,'A'),s={instance_id:'vamp',definition_id:'devoured',source:entity,target:entity,applied_at_ms:0,expires_at_ms:12000,modifier_bps:1000,attribute:'life_steal',stacks:2,max_stacks:3};
 f.payload.snapshot.self.statuses=[s];assert.equal(r.apply(f).self.statuses[0].stacks,2);
 const d=delta();d.payload.delta.own=structuredClone(f.payload.snapshot.self);d.payload.delta.own.statuses=[{...s,instance_id:'reduction',modifier_bps:0,modifiers:[{attribute:'life_steal',modifier_bps:-1000},{attribute:'atk',modifier_bps:500}]}];delete d.payload.delta.own.statuses[0].attribute;assert.equal(r.apply(d).self.statuses[0].modifiers[0].modifier_bps,-1000);
});
test('life-steal invalid modifier and private-budget smuggling are atomic frame failures',()=>{
 for(const mutate of [s=>s.modifier_bps=-10001,s=>s.modifier_bps=100001,s=>s.stacks=4,s=>s.resource_budget=[3,4],s=>{delete s.attribute;s.modifier_bps=0;s.modifiers=[{attribute:'life_steal',modifier_bps:1000},{attribute:'life_steal',modifier_bps:1000}];}]){
  const {r,before}=fixture(),d=delta();d.payload.delta.own=structuredClone(before.self);const s={instance_id:'vamp',definition_id:'devoured',source:entity,target:entity,applied_at_ms:0,expires_at_ms:12000,modifier_bps:1000,attribute:'life_steal',stacks:2,max_stacks:3};mutate(s);d.payload.delta.own.statuses=[s];assert.throws(()=>r.apply(d),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);
 }
});

test('own skill power values are native display data through full and delta',()=>{
 const f=full(),r=new ReplicaV6(key,'A');f.payload.snapshot.self.player_skills=[{skill_id:'first',ready_at_ms:0},{skill_id:'second',ready_at_ms:0}];f.payload.snapshot.self.skill_powers={first:99,second:112};assert.equal(r.apply(f).self.skill_powers.first,99);
 const d=delta();d.payload.delta.own=structuredClone(f.payload.snapshot.self);d.payload.delta.own.skill_powers.first=108;assert.equal(r.apply(d).self.skill_powers.first,108);d.payload.delta.own.skill_powers.first=0;assert.equal(r.snapshot.self.skill_powers.first,108);
});
test('corrupt power tables and member-view power leaks cannot partially overwrite a baseline',()=>{
 for(const bad of [null,[],{first:null},{first:-1},{first:1.5},{first:4294967296},{first:99,unowned:10},{}]){
  const f=full(),r=new ReplicaV6(key,'A');f.payload.snapshot.self.player_skills=[{skill_id:'first',ready_at_ms:0}];f.payload.snapshot.self.skill_powers={first:99};r.apply(f);const before=r.snapshot,d=delta();d.payload.delta.own=structuredClone(before.self);d.payload.delta.own.skill_powers=bad;assert.throws(()=>r.apply(d),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);
 }
 const {r,before}=fixture(),d=delta();d.payload.delta.members={upsert:[{...before.members[0],skill_powers:{first:99}}],removed:[],order:[entity]};assert.throws(()=>r.apply(d),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);
});
test('trusted slot replacement must publish its corresponding current power table',()=>{
 const f=full(),r=new ReplicaV6(key,'A');f.payload.snapshot.self.player_skills=[{skill_id:'first',ready_at_ms:0}];f.payload.snapshot.self.skill_powers={first:99};f.payload.snapshot.self.skill_loadout_revision=0;r.apply(f);const d=delta();d.payload.delta.own=structuredClone(f.payload.snapshot.self);d.payload.delta.own.skill_loadout_revision=1;d.payload.delta.own.player_skills=[{skill_id:'second',ready_at_ms:0}];assert.throws(()=>r.apply(d),code('INVALID_FRAME'));d.payload.delta.own.skill_powers={second:112};assert.equal(r.apply(d).self.skill_powers.second,112);
});

test('delayed status uses its native one-tick clock and private lineage cannot enter full or delta',()=>{
 const f=periodicFull(),s=f.payload.snapshot.monsters[0].statuses[0];Object.assign(s,{applied_at_ms:0,expires_at_ms:1230});Object.assign(s.periodic,{interval_ms:1200,next_tick_at_ms:1200,total_ticks:1,ticks_done:0,raw_damage:0,power_sampling:'source_snapshot'});
 const r=new ReplicaV6(key,'A'),before=r.apply(f);
 for(const [field,value] of [['derived',true],['action_origin',{skill:'borrowed-skill'}],['end_effects',1],['periodic_inputs',{attack:999}],['delayed_damage',{fixed:999}]]){
  const bad=structuredClone(f);bad.revision=2;bad.publication_id='private-lineage';bad.payload.snapshot.monsters[0].statuses[0][field]=value;assert.throws(()=>r.apply(bad),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);
  const d=delta(),n=structuredClone(before.monsters[0]);n.statuses[0][field]=value;d.payload.delta.monsters={upsert:[n],removed:[],order:[npc]};assert.throws(()=>r.apply(d),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);
 }
});
function counterFull(){const f=full();f.payload.snapshot.self.statuses=[{instance_id:'seed-1',definition_id:'counter',source:structuredClone(entity),target:structuredClone(entity),applied_at_ms:0,expires_at_ms:1200,modifier_bps:0,base_modifiers:[{attribute:'life_steal',modifier_bps:1500},{attribute:'damage_reduction',modifier_bps:2000}],modifiers:[{attribute:'life_steal',modifier_bps:300}],stack_counter:true,stacks:0,max_stacks:3,window:{basis:'damage_dealt',recorded:0,cap:10000,contacts:0,maximum_sampling:'snapshot',max_hp_at_apply:100,max_spirit_at_apply:50}}];return f;}
test('counter zero full and layered delta preserve one identity fixed effects and window record',()=>{const r=new ReplicaV6(key,'A'),f=counterFull();assert.equal(r.apply(f).self.statuses[0].stacks,0);const d=delta();d.payload.delta.own=structuredClone(r.snapshot.self);const s=d.payload.delta.own.statuses[0];s.stacks=3;s.window.recorded=300;s.window.contacts=3;const v=r.apply(d);assert.equal(v.self.statuses.length,1);assert.equal(v.self.statuses[0].instance_id,'seed-1');assert.deepEqual(v.self.statuses[0].base_modifiers,f.payload.snapshot.self.statuses[0].base_modifiers);});
test('counter consumption delta to real zero retains fixed effects original deadline and record',()=>{const f=counterFull(),s=f.payload.snapshot.self.statuses[0];s.stacks=3;s.window.recorded=400;s.window.contacts=4;const r=new ReplicaV6(key,'A');r.apply(f);const d=delta();d.payload.delta.own=structuredClone(r.snapshot.self);d.payload.delta.own.statuses[0].stacks=0;const after=r.apply(d).self.statuses[0];assert.equal(after.stacks,0);assert.equal(after.expires_at_ms,1200);assert.equal(after.window.recorded,400);});
test('counter invalid flags zero legacy and malformed fixed components never change full or delta baseline',()=>{for(const mutate of [s=>delete s.stack_counter,s=>s.stack_counter=false,s=>s.stack_counter=null,s=>s.stack_counter={},s=>s.stack_counter=1,s=>s.action_origin="basic",s=>s.derived=false,s=>s.stacks=4,s=>s.max_stacks=0,s=>s.base_modifiers=[],s=>s.base_modifiers=null,s=>s.base_modifiers[1]={...s.base_modifiers[0]},s=>s.base_modifiers[0].extra=1,s=>s.flags=['root'],s=>s.attribute='life_steal',s=>s.base_modifiers[0].modifier_bps=NaN,s=>s.base_modifiers[0].attribute='control_immunity']){const f=counterFull();mutate(f.payload.snapshot.self.statuses[0]);const empty=new ReplicaV6(key,'A');assert.throws(()=>empty.apply(f),code('INVALID_FRAME'));assert.equal(empty.snapshot,null);const r=new ReplicaV6(key,'A');r.apply(counterFull());const before=r.snapshot,d=delta();d.payload.delta.own=structuredClone(f.payload.snapshot.self);assert.throws(()=>r.apply(d),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);}});
test('counter reconnect reconstructs zero and rejects old generation without replaying state',()=>{const f=counterFull(),r=new ReplicaV6(key,'A');r.apply(f);const next=structuredClone(f);next.connection={id:'counter-new',generation:2};next.stream_epoch=2;next.revision=5;next.publication_id='counter-full';assert.equal(r.apply(next).self.statuses[0].stacks,0);assert.throws(()=>r.apply(f),code('STALE_FRAME'));assert.equal(r.snapshot.self.statuses[0].window.recorded,0);});
test('counter fixed-only carrier and ally source keep the real zero without a second status',()=>{const f=counterFull(),s=f.payload.snapshot.self.statuses[0];delete s.modifiers;s.source={kind:'participant',id:'B',generation:1};f.payload.snapshot.members.push({...structuredClone(f.payload.snapshot.members[0]),entity:structuredClone(s.source)});const r=new ReplicaV6(key,'A');assert.equal(r.apply(f).self.statuses.length,1);assert.equal(r.snapshot.self.statuses[0].stacks,0);});

test('counter zero expiration cleanse projection remains valid with a single native lifetime',()=>{const f=counterFull();f.payload.snapshot.self.statuses[0].cleanse={on_tick:false,on_expired:true};const r=new ReplicaV6(key,'A');assert.equal(r.apply(f).self.statuses[0].stacks,0);const d=delta();d.payload.delta.own=structuredClone(r.snapshot.self);d.payload.delta.own.statuses[0].cleanse.on_tick=true;const before=r.snapshot;assert.throws(()=>r.apply(d),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);});

function activationFull(){const f=full();f.payload.snapshot.self.statuses=[{instance_id:'activation-1',definition_id:'source-power',source:structuredClone(entity),target:structuredClone(entity),applied_at_ms:0,expires_at_ms:5000,modifier_bps:0,modifiers:[{attribute:'atk',modifier_bps:500},{attribute:'def',modifier_bps:500}]}];return f;}
test('activation compound effects replace and expire through native full and delta without adding a second authority',()=>{const f=activationFull(),r=new ReplicaV6(key,'A');assert.equal(r.apply(f).self.statuses[0].modifiers.length,2);const d=delta();d.payload.delta.own=structuredClone(r.snapshot.self);d.payload.delta.own.statuses[0].modifiers.push({attribute:'life_steal',modifier_bps:2000});assert.equal(r.apply(d).self.statuses[0].modifiers.length,3);const expired=structuredClone(d);expired.publication_id='expired-activation';expired.revision=3;expired.payload.base_publication_id=d.publication_id;expired.payload.base_revision=2;expired.payload.delta.own.statuses=[];assert.equal(r.apply(expired).self.statuses.length,0);});
test('activation private eligibility and frozen budget cannot be injected into compound full or delta status',()=>{for(const field of ['maximum_mastery','minimum_mastery','qualification','status_bonus','action_origin','unit_target','budget','derived']){const f=activationFull(),r=new ReplicaV6(key,'A');r.apply(f);const before=r.snapshot,bad=structuredClone(f);bad.publication_id='bad-activation';bad.revision=2;bad.payload.snapshot.self.statuses[0][field]={value:1};assert.throws(()=>r.apply(bad),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);const d=delta();d.payload.delta.own=structuredClone(bad.payload.snapshot.self);assert.throws(()=>r.apply(d),code('INVALID_FRAME'));assert.deepEqual(r.snapshot,before);}});

test('basic wave full recovery and empty delta preserve strict schedules and target generations', () => {
  const frame = periodicFull();
  const state = frame.payload.snapshot;
  state.logical_ms = 150;
  state.monsters[0].entity = { ...state.monsters[0].entity, generation: 2 };
  state.monsters[0].statuses = [];
  const wave = {
    id: 'wave-1', plan_id: 'fan-wave', source: entity, origin: [10, 20],
    geometry: { kind: 'directional', direction: [1, 0], cone_degrees: 84 },
    range: 270, speed: 500, released_at_ms: 120,
    pending: [{ target: { id: npc.id, generation: 1 }, at_ms: 200 }],
  };
  state.basic_waves = [wave];
  const replica = new ReplicaV6(key, 'A');
  const before = replica.apply(frame);
  const malformed = [
    w => { w[0].pending[0].at_ms = 150; },
    w => { w[0].pending[0].at_ms = 661; },
    w => { w[0].pending[0].target.generation = 3; },
    w => { w[0].source = { ...entity, generation: 2 }; },
    w => { w[0].geometry = { kind: 'radial', direction: [1, 0] }; },
    w => { w[0].pending.push(structuredClone(w[0].pending[0])); },
    w => { w.push({ ...w[0], id: 'wave-2' }); },
    w => { w[0].pending[0].raw = 1000; },
  ];
  for (const mutate of malformed) {
    const bad = delta();
    bad.payload.delta.logical_ms = 150;
    bad.payload.delta.basic_waves = structuredClone(state.basic_waves);
    mutate(bad.payload.delta.basic_waves);
    assert.throws(() => replica.apply(bad), code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot, before);
  }
  const reconnect = structuredClone(frame);
  reconnect.connection = { id: 'replacement', generation: 2 };
  reconnect.stream_epoch = 2;
  reconnect.revision = 2;
  reconnect.publication_id = 'full-2';
  assert.deepEqual(replica.apply(reconnect).basic_waves, [wave]);
  const clear = {
    ...reconnect, revision: 3, publication_id: 'delta-3',
    payload: { kind: 'delta', base_publication_id: 'full-2', base_revision: 2,
      delta: { logical_ms: 210, basic_waves: [] } },
  };
  assert.deepEqual(replica.apply(clear).basic_waves, []);
  assert.equal(Object.hasOwn(new ReplicaV6(key, 'A').apply(full()), 'basic_waves'), false);
});

test('trusted basic profile is private and immutable across deltas and reconnection', () => {
  const frame = full();
  frame.payload.snapshot.self.basic_profile = 'tower';
  const replica = new ReplicaV6(key, 'A');
  const before = replica.apply(frame);
  for (const value of ['shield', '', null, '台'.repeat(22), undefined]) {
    const patch = delta();
    patch.payload.delta.own = { ...before.self };
    if (value === undefined) delete patch.payload.delta.own.basic_profile;
    else patch.payload.delta.own.basic_profile = value;
    assert.throws(() => replica.apply(patch), code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot, before);
  }
  const reconnect = structuredClone(frame);
  reconnect.connection = { id: 'replacement', generation: 2 };
  reconnect.stream_epoch = 2;
  reconnect.revision = 2;
  reconnect.publication_id = 'profile-full-2';
  assert.equal(replica.apply(reconnect).self.basic_profile, 'tower');
  assert.equal(Object.hasOwn(replica.snapshot.members[0], 'basic_profile'), false);
});

test('alternating basic wave projects the next phase privately across delta and reconnect', () => {
  const frame = full();
  frame.payload.snapshot.basic_waves = [];
  frame.payload.snapshot.self.basic_profile = 'tower_guard_domain';
  frame.payload.snapshot.self.basic_wave_phase = 'primary';
  const replica = new ReplicaV6(key, 'A');
  const initial = replica.apply(frame);
  assert.equal(initial.self.basic_wave_phase, 'primary');
  assert.equal(Object.hasOwn(initial.members[0], 'basic_wave_phase'), false);
  for (const value of ['open', null, true]) {
    const bad = delta();
    bad.payload.delta.own = { ...initial.self, basic_wave_phase: value };
    assert.throws(() => replica.apply(bad), code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot, initial);
  }
  const patch = delta();
  patch.payload.delta.own = { ...initial.self, basic_wave_phase: 'alternate' };
  assert.equal(replica.apply(patch).self.basic_wave_phase, 'alternate');
  const reconnected = structuredClone(frame);
  reconnected.connection = { id: 'replacement', generation: 2 };
  reconnected.stream_epoch = 2;
  reconnected.revision = 3;
  reconnected.publication_id = 'gate-full-3';
  reconnected.payload.snapshot.logical_ms = patch.payload.delta.logical_ms;
  reconnected.payload.snapshot.self.basic_wave_phase = 'alternate';
  assert.equal(replica.apply(reconnected).self.basic_wave_phase, 'alternate');
  delete reconnected.payload.snapshot.basic_waves;
  assert.throws(() => new ReplicaV6(key, 'A').apply(reconnected), code('INVALID_FRAME'));
});

test('basic projectile segments preserve one action and private volley progress on reconnect', () => {
  const frame = full();
  const state = frame.payload.snapshot;
  state.logical_ms = 150;
  state.self.basic_projectile_progress = {stored:6, maximum:6, attack_speed_bonus_bps:4800};
  const projectile = {
    id:'shot-1', source:entity, position:[15,20], radius:4,
    basic_segment:{action_id:'action-1', plan_id:'volley', index:1, launched_at_ms:120, aim_point:[60,20]},
  };
  state.projectiles = [projectile];
  const replica = new ReplicaV6(key, 'A');
  const initial = replica.apply(frame);
  for (const change of [
    {basic_segment: { ...projectile.basic_segment, launched_at_ms: 181 }},
    {basic_segment: { ...projectile.basic_segment, index: 9 }},
    {basic_segment: null},
    {id:'duplicate-shot'},
  ]) {
    const patch = delta();
    patch.payload.delta.logical_ms = 180;
    const shot = {...projectile, ...change};
    patch.payload.delta.projectiles = {
      upsert:[shot], removed:[], order:shot.id === projectile.id ? [shot.id] : [projectile.id,shot.id],
    };
    assert.throws(() => replica.apply(patch), code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot, initial);
  }
  const badProgress = delta();
  badProgress.payload.delta.logical_ms = 180;
  badProgress.payload.delta.own = {
    ...initial.self, basic_projectile_progress: {stored:6, maximum:6, attack_speed_bonus_bps:4801},
  };
  assert.throws(() => replica.apply(badProgress), code('INVALID_FRAME'));
  assert.deepEqual(replica.snapshot, initial);
  const resumed = structuredClone(frame);
  resumed.publication_id = 'resumed-projectiles';
  resumed.connection = {id:'next', generation:2};
  resumed.stream_epoch = 2;
  resumed.revision = 2;
  resumed.payload.snapshot.projectiles.push({
    ...projectile, id:'shot-2', basic_segment:{...projectile.basic_segment, index:2, launched_at_ms:150},
  });
  const next = replica.apply(resumed);
  assert.deepEqual(next.projectiles.map(p=>p.basic_segment.index),[1,2]);
  assert.equal(next.self.basic_projectile_progress.stored,6);
  assert.equal(Object.hasOwn(next.members[0],'basic_projectile_progress'),false);
});

test('landed seals and recall or return segment roles validate atomically', () => {
  const frame = full();
  const state = frame.payload.snapshot;
  state.logical_ms = 150;
  state.members[0].basic_seals = [[45, 20], [60, 20]];
  const projectile = {
    id:'seal-1', source:entity, position:[40,20], radius:4,
    basic_segment:{action_id:'recall-1', plan_id:'recall', index:1, launched_at_ms:120, aim_point:[10,20], role:'seal_recall'},
  };
  state.projectiles = [projectile];
  const replica = new ReplicaV6(key, 'A');
  const initial = replica.apply(frame);
  for (const value of [undefined, null, [[1]], Array.from({length:9}, () => [1,2])]) {
    const patch = delta();
    const member = structuredClone(state.members[0]);
    if (value === undefined) delete member.basic_seals;
    else member.basic_seals = value;
    patch.payload.delta.logical_ms = 180;
    patch.payload.delta.members = {upsert:[member], removed:[], order:[entity]};
    assert.throws(() => replica.apply(patch), code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot, initial);
  }
  for (const role of ['seal_outbound', 'unknown', null]) {
    const patch = delta();
    patch.payload.delta.logical_ms = 180;
    patch.payload.delta.projectiles = {
      upsert:[{...projectile,basic_segment:{...projectile.basic_segment,role}}],
      removed:[],order:[projectile.id],
    };
    assert.throws(() => replica.apply(patch), code('INVALID_FRAME'));
  }
  const next = delta();
  next.payload.delta.logical_ms = 180;
  next.payload.delta.members = {upsert:[{...state.members[0],basic_seals:[[60,20]]}],removed:[],order:[entity]};
  assert.deepEqual(replica.apply(next).members[0].basic_seals, [[60,20]]);
});

test('basic world fields and landing waves preserve independent clocks through publication', () => {
  const frame = periodicFull();
  const state = frame.payload.snapshot;
  const npc = state.monsters[0].entity;
  const field = {
    id:'field-one', source:entity, skill_id:'tower-field', basic_action_id:'basic-one',
    mechanism:'basic_field', center:[260,20], radius:150, created_at_ms:0, activates_at_ms:0,
    expires_at_ms:6000, interval_ms:1000, next_tick_at_ms:1000, completed_ticks:0,
    total_ticks:6, contact_policy:null,
  };
  const wave = {
    id:'drop-one', source:entity, plan_id:'tower-landing', origin:[260,20], range:150,
    geometry:{kind:'landing',from:[10,20]}, speed:1400, released_at_ms:0,
    pending:[{target:{id:npc.id,generation:npc.generation},at_ms:179}],
  };
  state.areas = [field];
  state.basic_waves = [wave];
  const replica = new ReplicaV6(key, 'A');
  const initial = replica.apply(frame);
  for (const change of [{total_ticks:5},{basic_action_id:null},{next_tick_at_ms:900},{radius:513}]) {
    const patch = delta();
    patch.payload.delta.areas = [{...field,...change}];
    assert.throws(() => replica.apply(patch), code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot, initial);
  }
  const bad = delta();
  bad.payload.delta.basic_waves = [{...wave,pending:[{...wave.pending[0],at_ms:107}]}];
  assert.throws(() => replica.apply(bad), code('INVALID_FRAME'));
  assert.deepEqual(replica.snapshot, initial);

  const resumed = structuredClone(frame);
  resumed.publication_id = 'world-field-full';
  resumed.connection = {id:'next',generation:2};
  resumed.stream_epoch = 2;
  resumed.revision = 2;
  resumed.payload.snapshot.logical_ms = 30;
  assert.deepEqual(replica.apply(resumed).areas, [field]);
});

test('fixed-delay basic impacts rehydrate and clear atomically without accepting expired plans', () => {
  const frame = periodicFull();
  const state = frame.payload.snapshot;
  state.logical_ms = 180;
  state.monsters[0].entity = { ...state.monsters[0].entity, generation: 2 };
  state.monsters[0].statuses = [];
  state.basic_impacts = [{
    id: 'impact-1', plan_id: 'fixed-impact', source: entity, kind: 'fixed_delay',
    origin: [10, 20], target: { id: npc.id, generation: 1 },
    released_at_ms: 180, impact_at_ms: 530,
  }];
  const replica = new ReplicaV6(key, 'A');
  const before = replica.apply(frame);
  const invalid = [
    plans => { plans[0].impact_at_ms = 180; },
    plans => { plans[0].impact_at_ms = 60181; },
    plans => { plans[0].released_at_ms = 181; },
    plans => { plans[0].target.generation = 3; },
    plans => { plans[0].source = { ...entity, generation: 2 }; },
    plans => { plans[0].kind = 'hitscan'; },
    plans => { plans.push({ ...plans[0], id: 'impact-2' }); },
    plans => { plans[0].raw = 78; },
  ];
  for (const mutate of invalid) {
    const bad = delta();
    bad.payload.delta.logical_ms = 180;
    bad.payload.delta.basic_impacts = structuredClone(state.basic_impacts);
    mutate(bad.payload.delta.basic_impacts);
    assert.throws(() => replica.apply(bad), code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot, before);
  }
  const reconnect = structuredClone(frame);
  reconnect.connection = { id: 'replacement', generation: 2 };
  reconnect.stream_epoch = 2;
  reconnect.revision = 2;
  reconnect.publication_id = 'impact-reconnect';
  assert.deepEqual(replica.apply(reconnect).basic_impacts, state.basic_impacts);
  const clear = {
    ...reconnect, revision: 3, publication_id: 'impact-clear',
    payload: { kind: 'delta', base_publication_id: 'impact-reconnect', base_revision: 2,
      delta: { logical_ms: 540, basic_impacts: [] } },
  };
  assert.deepEqual(replica.apply(clear).basic_impacts, []);
  assert.equal(Object.hasOwn(new ReplicaV6(key, 'A').apply(full()), 'basic_impacts'), false);
});

test('basic beam display replaces aim and clears on protection without replaying a prior session', () => {
  const frame = full();
  frame.payload.snapshot.basic_beams = [{
    action_id: 'beam-1', plan_id: 'open-ray', source: entity, direction: [1, 0],
    range: 440, half_width: 22, interval_ms: 150, started_at_ms: 0, next_tick_at_ms: 150,
    ticks: 0, released: false,
  }];
  const replica = new ReplicaV6(key, 'A');
  const before = replica.apply(frame);
  for (const mutate of [
    v => { v[0].direction = [1, 1]; },
    v => { v[0].next_tick_at_ms = 180; },
    v => { v[0].source.generation = 2; },
    v => { v.push({ ...v[0], action_id: 'beam-2' }); },
    v => { v[0].released = true; v[0].ticks = 1; v[0].next_tick_at_ms = 300; },
    v => { v[0].max_duration_ms = 999; },
  ]) {
    const bad = delta();
    bad.payload.delta.basic_beams = structuredClone(before.basic_beams);
    mutate(bad.payload.delta.basic_beams);
    assert.throws(() => replica.apply(bad), code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot, before);
  }
  const aimed = delta();
  aimed.payload.delta.basic_beams = [{ ...before.basic_beams[0], direction: [0, 1] }];
  assert.deepEqual(replica.apply(aimed).basic_beams[0].direction, [0, 1]);
  const next = full();
  next.publication_id = 'beam-reconnect';
  next.revision = 3;
  next.connection = { id: 'new', generation: 2 };
  next.stream_epoch = 2;
  next.payload.snapshot.logical_ms = 60;
  next.payload.snapshot.self.protected = true;
  next.payload.snapshot.self.ready = false;
  next.payload.snapshot.members[0].protected = true;
  next.payload.snapshot.basic_beams = structuredClone(before.basic_beams);
  assert.throws(() => replica.apply(next), code('INVALID_FRAME'));
  next.payload.snapshot.basic_beams = [];
  assert.deepEqual(replica.apply(next).basic_beams, []);
  assert.equal(Object.hasOwn(new ReplicaV6(key, 'A').apply(full()), 'basic_beams'), false);
});

test('ground basic impacts restore a frozen position and reject mixed or non-finite targets', () => {
  const frame = periodicFull();
  const plan = {
    id: 'ground-impact', plan_id: 'ground-basic', source: entity, kind: 'ground_field',
    origin: [10, 20], position: [80, 20], released_at_ms: 0, impact_at_ms: 350,
  };
  frame.payload.snapshot.basic_impacts = [plan];
  const replica = new ReplicaV6(key, 'A');
  const before = replica.apply(frame);
  const invalid = [
    p => { p.target = { id: npc.id, generation: npc.generation }; },
    p => { delete p.position; },
    p => { p.position[0] = Infinity; },
    p => { p.kind = 'fixed_delay'; },
  ];
  for (const mutate of invalid) {
    const bad = delta();
    bad.payload.delta.basic_impacts = [structuredClone(plan)];
    mutate(bad.payload.delta.basic_impacts[0]);
    assert.throws(() => replica.apply(bad), code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot, before);
  }
  const reconnected = structuredClone(frame);
  reconnected.connection = {id:'ground-reconnect', generation:2};
  reconnected.stream_epoch = 2;
  reconnected.revision = 2;
  reconnected.publication_id = 'ground-full';
  assert.deepEqual(replica.apply(reconnected).basic_impacts, [plan]);
});

test('basic charge publishes the holder’s tier and removes the session on release', () => {
  const frame = full();
  const own = frame.payload.snapshot.self;
  own.basic_charge = {session_id:'charge-a',kind:'staged_wave',started_at_ms:0,max_charge_at_ms:3000,tier:1};
  const replica = new ReplicaV6(key, 'A');
  const initial = replica.apply(frame);
  for (const bad of [{tier:4},{kind:'dash'},{started_at_ms:31},{max_charge_at_ms:30001}]) {
    const patch = delta();
    patch.payload.delta.own = {...own,basic_charge:{...own.basic_charge,...bad}};
    assert.throws(() => replica.apply(patch), code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot, initial);
  }
  const patch = delta();
  patch.payload.delta.logical_ms = 660;
  patch.payload.delta.own = {...own,basic_charge:{...own.basic_charge,tier:2}};
  assert.equal(replica.apply(patch).self.basic_charge.tier, 2);
  const next = delta();
  next.revision = 3;
  next.publication_id = 'delta-3';
  next.payload.base_revision = 2;
  next.payload.base_publication_id = 'delta-2';
  next.payload.delta.logical_ms = 690;
  next.payload.delta.own = {...own};
  delete next.payload.delta.own.basic_charge;
  assert.equal(replica.apply(next).self.basic_charge, undefined);
});

test('charged flight snapshots preserve a frozen tier and reject forged segment fields', () => {
  const frame = full();
  const launched = {
    id:'flight-1',source:entity,position:[10,20],radius:42,
    basic_segment:{role:'charged_flight',tier:2,action_id:'release-1',plan_id:'charged-flight',
      index:0,launched_at_ms:0,aim_point:[300,20]},
  };
  frame.payload.snapshot.projectiles = [launched];
  const replica = new ReplicaV6(key, 'A');
  const original = replica.apply(frame);
  for (const patch of [{tier:0},{tier:undefined},{index:1},{role:'seal_outbound'}]) {
    const update = delta();
    update.payload.delta.projectiles = {order:[launched.id],removed:[],
      upsert:[{...launched,basic_segment:{...launched.basic_segment,...patch}}]};
    assert.throws(() => replica.apply(update),code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot,original);
  }
  const resumed = structuredClone(frame);
  resumed.connection = {id:'next',generation:2};
  resumed.stream_epoch = 2;
  resumed.revision = 2;
  resumed.publication_id = 'flight-resume';
  resumed.payload.snapshot.logical_ms = 30;
  resumed.payload.snapshot.projectiles[0].position = [25,20];
  assert.deepEqual(replica.apply(resumed).projectiles[0].basic_segment,launched.basic_segment);
});

test('equipment changes require a newer private revision and survive a reconnect full', () => {
  const frame = full();
  const initial = frame.payload.snapshot.self;
  initial.basic_profile = 'mirror_world_beam';
  initial.equipment = {revision:0,weapon:{instance_id:'mirror-1',model_id:'mirror',morph_id:'mirror_world_beam'}};
  const replica = new ReplicaV6(key,'A');
  const before = replica.apply(frame);
  for (const bad of [
    {...initial,basic_profile:'sword'},
    {...initial,equipment:undefined},
    {...initial,equipment:{revision:1}},
    {...initial,equipment:{revision:1,weapon:{...initial.equipment.weapon,morph_id:null}}},
  ]) {
    const patch = delta();
    patch.payload.delta.own = bad;
    assert.throws(()=>replica.apply(patch),code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot,before);
  }
  const patch = delta();
  patch.payload.delta.own = {...initial,basic_profile:'sword',
    equipment:{revision:1,weapon:{instance_id:'sword-1',model_id:'guhong-sword'}}};
  assert.equal(replica.apply(patch).self.basic_profile,'sword');
  const resumed = structuredClone(frame);
  resumed.connection = {id:'new',generation:2};
  resumed.stream_epoch = 2;
  resumed.revision = 3;
  resumed.publication_id = 'equipment-reconnect';
  resumed.payload.snapshot.logical_ms = 60;
  resumed.payload.snapshot.self = {...patch.payload.delta.own,equipment:{revision:2}};
  delete resumed.payload.snapshot.self.basic_profile;
  assert.deepEqual(replica.apply(resumed).self.equipment,{revision:2});
});

test('moving area restores its warning and committed member contact history', () => {
  const frame = periodicFull();
  const area = {
    id:'moving-1',source:npc,skill_id:'wind',mechanism:'moving_enter',
    center:[10,20],radius:60,created_at_ms:0,activates_at_ms:30,expires_at_ms:2280,
    interval_ms:0,next_tick_at_ms:null,completed_ticks:0,total_ticks:0,
    movement:{origin:[10,20],direction:[1,0],speed:400,distance:900,committed:false},
    contact_policy:{max_targets_per_tick:8,max_total_hits:32,repeat_interval_ms:2000,
      used_hits:0,remaining_hits:32,contacts:[]},
  };
  frame.payload.snapshot.areas=[area];
  const replica=new ReplicaV6(key,'A');
  const initial=replica.apply(frame);
  const hit=structuredClone(area);
  hit.movement.committed=true;
  hit.center=[22,20];
  Object.assign(hit.contact_policy,{used_hits:1,remaining_hits:31,
    contacts:[{target:{id:'A',generation:1},last_hit_at_ms:30,next_eligible_at_ms:2030}]});
  for(const forged of [{center:[23,20]},{source:entity},{movement:{...hit.movement,committed:false}},
    {contact_policy:{...hit.contact_policy,contacts:[{target:{id:npc.id,generation:1},last_hit_at_ms:30,next_eligible_at_ms:2030}]}}]){
    const patch=delta();
    patch.payload.delta.logical_ms=60;
    patch.payload.delta.areas=[{...hit,...forged}];
    assert.throws(()=>replica.apply(patch),code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot,initial);
  }
  const resumed=structuredClone(frame);
  resumed.connection={id:'moving-reconnect',generation:2};
  resumed.stream_epoch=2;
  resumed.revision=2;
  resumed.publication_id='moving-full';
  resumed.payload.snapshot.logical_ms=60;
  resumed.payload.snapshot.areas=[hit];
  assert.deepEqual(replica.apply(resumed).areas,[hit]);
});

test('retained area slow preserves the original NPC generation through a reconnect full', () => {
  const frame=periodicFull();
  frame.payload.snapshot.self.shield_total=0;
  const slow={instance_id:'windbound-1',definition_id:'windbound',source:npc,target:entity,
    applied_at_ms:0,expires_at_ms:3000,modifier_bps:-2000,attribute:'movement_slow',retained_source:true};
  frame.payload.snapshot.self.statuses=[slow];
  const replica=new ReplicaV6(key,'A');
  replica.apply(frame);
  const resumed=structuredClone(frame);
  resumed.connection={id:'wind-resume',generation:2};
  resumed.stream_epoch=2;
  resumed.revision=2;
  resumed.publication_id='wind-full';
  resumed.payload.snapshot.logical_ms=30;
  resumed.payload.snapshot.monsters[0].entity={...npc,generation:2};
  resumed.payload.snapshot.monsters[0].statuses=[];
  for(const forged of [
    {...slow,retained_source:undefined},
    {...slow,retained_source:false},
    {...slow,source:{...npc,generation:3}},
    {...slow,source:{...npc,id:'forged'}},
    {...slow,attribute:'atk'},
    {...slow,stacks:1,max_stacks:1},
  ]){
    const bad=structuredClone(resumed);
    bad.payload.snapshot.self.statuses=[forged];
    assert.throws(()=>replica.apply(bad),code('INVALID_FRAME'));
  }
  assert.deepEqual(replica.apply(resumed).self.statuses,[slow]);
});

test('NPC ground fields preserve a fixed schedule and original caster on reconnect', () => {
  const initial=periodicFull();
  const warning={
    id:'ground-1',source:npc,skill_id:'fire-pool',mechanism:'npc_ground_field',
    center:[10,20],radius:140,created_at_ms:0,activates_at_ms:750,expires_at_ms:8750,
    interval_ms:1000,next_tick_at_ms:1750,completed_ticks:0,total_ticks:8,
    contact_policy:{max_targets_per_tick:16,max_total_hits:256,repeat_interval_ms:1000,
      used_hits:0,remaining_hits:256,contacts:[]},
  };
  initial.payload.snapshot.areas=[warning];
  const replica=new ReplicaV6(key,'A');
  const before=replica.apply(initial);
  const reconnect=structuredClone(initial);
  Object.assign(reconnect,{connection:{id:'ground-resume',generation:2},stream_epoch:2,
    revision:2,publication_id:'ground-full'});
  const snapshot=reconnect.payload.snapshot;
  snapshot.logical_ms=1760;
  snapshot.monsters[0].entity={...npc,generation:2};
  snapshot.monsters[0].statuses=[];
  const field=snapshot.areas[0];
  Object.assign(field,{completed_ticks:1,next_tick_at_ms:2750});
  Object.assign(field.contact_policy,{used_hits:1,remaining_hits:255,
    contacts:[{target:{id:'A',generation:1},last_hit_at_ms:1750,next_eligible_at_ms:2750}]});
  for(const corrupt of [
    a=>a.completed_ticks=2,
    a=>a.next_tick_at_ms=2760,
    a=>a.source.generation=3,
    a=>a.contact_policy.max_targets_per_tick=17,
    a=>a.contact_policy.contacts[0].last_hit_at_ms=1760,
    a=>a.contact_policy.contacts[0].target.id=npc.id,
    a=>a.movement={},
  ]){
    const bad=structuredClone(reconnect);
    corrupt(bad.payload.snapshot.areas[0]);
    assert.throws(()=>replica.apply(bad),code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot,before);
  }
  assert.deepEqual(replica.apply(reconnect).areas,[field]);
});

test('NPC ground strike warning reconnects with the same point and clears on impact', () => {
  const frame=periodicFull();
  const warning={
    id:'strike-1',source:npc,skill_id:'thunder-strike',mechanism:'npc_ground_strike',
    center:[10,20],radius:96,created_at_ms:0,activates_at_ms:750,expires_at_ms:750,
    interval_ms:0,next_tick_at_ms:750,completed_ticks:0,total_ticks:1,contact_policy:null,
  };
  frame.payload.snapshot.areas=[warning];
  const replica=new ReplicaV6(key,'A');
  const before=replica.apply(frame);
  const reconnect=structuredClone(frame);
  Object.assign(reconnect,{connection:{id:'strike-resume',generation:2},stream_epoch:2,
    revision:2,publication_id:'strike-full'});
  reconnect.payload.snapshot.logical_ms=100;
  for(const corrupt of [
    s=>s.areas[0].source=entity,
    s=>s.areas[0].source={...npc,generation:2},
    s=>{s.monsters[0].hp=0;s.monsters[0].statuses=[];},
    s=>s.logical_ms=750,
    s=>s.areas[0].expires_at_ms=751,
    s=>s.areas[0].contact_policy={},
    s=>s.areas[0].completed_ticks=1,
  ]){
    const bad=structuredClone(reconnect);
    corrupt(bad.payload.snapshot);
    assert.throws(()=>replica.apply(bad),code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot,before);
  }
  assert.deepEqual(replica.apply(reconnect).areas,[warning]);
  const clear=delta();
  Object.assign(clear,{connection:reconnect.connection,stream_epoch:2,revision:3,publication_id:'strike-end'});
  Object.assign(clear.payload,{base_publication_id:'strike-full',base_revision:2,delta:{logical_ms:750,areas:[]}});
  assert.deepEqual(replica.apply(clear).areas,[]);
});

test('checkpoint windows and private progress reconnect and clear independently', () => {
  const start=full();
  const before=start.payload.snapshot;
  const progress={schedule_id:'route',collected:0,required:2,total_windows:3,last_window_index:0};
  before.self.checkpoint_progress=[progress];
  before.checkpoint_zones=[];
  const r=new ReplicaV6(key,'A');
  r.apply(start);
  const update=delta();
  const zone={schedule_id:'route',window_index:1,center:[10,20],radius:40,opens_at_ms:30,expires_at_ms:90,avoid:true};
  const reached={...progress,collected:1,last_window_index:1};
  update.payload.delta.checkpoint_zones=[zone];
  update.payload.delta.own={...before.self,checkpoint_progress:[reached]};
  assert.deepEqual(r.apply(update).self.checkpoint_progress,[reached]);
  const reconnect=structuredClone(start);
  Object.assign(reconnect,{connection:{id:'checkpoint-reconnect',generation:2},stream_epoch:2,
    revision:3,publication_id:'checkpoint-full'});
  const fresh=reconnect.payload.snapshot;
  Object.assign(fresh,{logical_ms:60,checkpoint_zones:[zone],self:{...before.self,checkpoint_progress:[reached]}});
  for(const corrupt of [
    s=>s.checkpoint_zones=null,
    s=>s.checkpoint_zones.push(zone),
    s=>s.checkpoint_zones[0].center=[11,20],
    s=>s.checkpoint_zones[0].avoid=false,
    s=>{delete s.checkpoint_zones[0].avoid;},
    s=>s.logical_ms=90,
    s=>s.checkpoint_zones[0].schedule_id='other',
    s=>s.self.checkpoint_progress=[progress],
    s=>s.self.checkpoint_progress[0].required=3,
    s=>{delete s.checkpoint_zones;delete s.self.checkpoint_progress;},
  ]){
    const bad=structuredClone(reconnect);
    corrupt(bad.payload.snapshot);
    assert.throws(()=>r.apply(bad),code('INVALID_FRAME'));
    assert.deepEqual(r.snapshot.checkpoint_zones,[zone]);
  }
  assert.deepEqual(r.apply(reconnect).checkpoint_zones,[zone]);
  const clear=delta();
  Object.assign(clear,{connection:reconnect.connection,stream_epoch:2,revision:4,publication_id:'checkpoint-end'});
  Object.assign(clear.payload,{base_publication_id:'checkpoint-full',base_revision:3,
    delta:{logical_ms:90,checkpoint_zones:[]}});
  const after=r.apply(clear);
  assert.deepEqual(after.checkpoint_zones,[]);
  assert.deepEqual(after.self.checkpoint_progress,[reached]);
});

test('checkpoint counter window cannot be extended or recreated by a later frame', () => {
  const initial=full(), snapshot=initial.payload.snapshot;
  const target={id:'npc',generation:1};
  const counter={window_index:1,target,opens_at_ms:0,expires_at_ms:90,on_contact:true};
  const progress={schedule_id:'route',collected:1,required:1,total_windows:2,last_window_index:1,counter};
  snapshot.checkpoint_zones=[];
  snapshot.monsters=[{entity:{kind:'monster',...target},template_id:'mob',position:[30,40],hp:80,max_hp:100,radius:2,returning:false,target:null}];
  snapshot.self.checkpoint_progress=[progress];
  const replica=new ReplicaV6(key,'A');
  replica.apply(initial);
  const update=delta();
  const own=structuredClone(snapshot.self);
  update.payload.delta.own=own;
  own.checkpoint_progress[0].counter.expires_at_ms=120;
  assert.throws(()=>replica.apply(update),code('INVALID_FRAME'));
  assert.equal(replica.snapshot.self.checkpoint_progress[0].counter.expires_at_ms,90);
  own.checkpoint_progress[0].counter.expires_at_ms=90;
  own.checkpoint_progress[0].counter.on_contact=false;
  assert.throws(()=>replica.apply(update),code('INVALID_FRAME'));
  delete own.checkpoint_progress[0].counter.on_contact;
  assert.throws(()=>replica.apply(update),code('INVALID_FRAME'));
  delete own.checkpoint_progress[0].counter;
  replica.apply(update);
  const late=structuredClone(initial);
  Object.assign(late,{revision:3,publication_id:'late-counter'});
  late.payload.snapshot.logical_ms=60;
  assert.throws(()=>replica.apply(late),code('INVALID_FRAME'));
  assert.equal(replica.snapshot.self.checkpoint_progress[0].counter,undefined);
});

test('ground trap restores the armed budget with its original owner and target generation', () => {
  const frame=periodicFull();
  const trap={
    id:'trap-1',source:entity,skill_id:'trap',mechanism:'member_ground_trap',
    center:[20,20],radius:60,created_at_ms:0,activates_at_ms:60,expires_at_ms:4560,
    interval_ms:0,next_tick_at_ms:null,completed_ticks:0,total_ticks:0,
    contact_policy:{max_targets_per_tick:2,max_total_hits:3,repeat_interval_ms:900,
      used_hits:0,remaining_hits:3,contacts:[]},
  };
  frame.payload.snapshot.areas=[trap];
  const replica=new ReplicaV6(key,'A');
  const before=replica.apply(frame);
  const full=structuredClone(frame);
  Object.assign(full,{connection:{id:'trap-resume',generation:2},stream_epoch:2,
    revision:2,publication_id:'trap-full'});
  const snapshot=full.payload.snapshot;
  snapshot.logical_ms=90;
  snapshot.monsters[0].entity={...npc,generation:2};
  snapshot.monsters[0].statuses=[];
  const armed=snapshot.areas[0];
  Object.assign(armed.contact_policy,{used_hits:1,remaining_hits:2,
    contacts:[{target:{id:npc.id,generation:1},last_hit_at_ms:60,next_eligible_at_ms:960}]});
  for(const corrupt of [
    a=>a.source=npc,
    a=>a.expires_at_ms=90,
    a=>a.next_tick_at_ms=120,
    a=>a.contact_policy.remaining_hits=3,
    a=>a.contact_policy.contacts[0].target.generation=3,
    a=>a.contact_policy.contacts[0].last_hit_at_ms=59,
  ]){
    const bad=structuredClone(full);
    corrupt(bad.payload.snapshot.areas[0]);
    assert.throws(()=>replica.apply(bad),code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot,before);
  }
  assert.deepEqual(replica.apply(full).areas,[armed]);
  const instant=structuredClone(full);
  const sameStep=instant.payload.snapshot.areas[0];
  instant.payload.snapshot.logical_ms=0;
  Object.assign(sameStep,{activates_at_ms:0,expires_at_ms:30});
  Object.assign(sameStep.contact_policy.contacts[0],{last_hit_at_ms:0,next_eligible_at_ms:900});
  assert.deepEqual(new ReplicaV6(key,'A').apply(instant).areas,[sameStep]);
});

function anchorFull(){
  const initial=full();
  const anchor={id:'anchor-1',source:entity,skill_id:'place',mechanism:'member_cast_anchor',
    center:[150,20],radius:0,created_at_ms:0,activates_at_ms:0,expires_at_ms:3000,
    interval_ms:0,next_tick_at_ms:null,completed_ticks:0,total_ticks:0,contact_policy:null};
  initial.payload.snapshot.areas=[anchor];
  return {initial,anchor};
}

test('cast anchors preserve their world point through reconnect and replace by new identity', () => {
  const {initial,anchor}=anchorFull();
  const r=new ReplicaV6(key,'A');
  const before=r.apply(initial);
  const reconnect=structuredClone(initial);
  Object.assign(reconnect,{connection:{id:'anchor-resume',generation:2},stream_epoch:2,
    revision:2,publication_id:'anchor-full'});
  reconnect.payload.snapshot.logical_ms=60;
  for(const corrupt of [
    s=>s.areas[0].center=[151,20],
    s=>s.areas[0].expires_at_ms=3001,
    s=>s.areas[0].radius=1,
    s=>s.areas.push({...s.areas[0],id:'duplicate-owner'}),
    s=>s.areas[0].source={...entity,generation:2},
    s=>s.logical_ms=3000,
  ]){
    const bad=structuredClone(reconnect);
    corrupt(bad.payload.snapshot);
    assert.throws(()=>r.apply(bad),code('INVALID_FRAME'));
    assert.deepEqual(r.snapshot,before);
  }
  assert.deepEqual(r.apply(reconnect).areas,[anchor]);
  const replace=delta();
  Object.assign(replace,{connection:reconnect.connection,stream_epoch:2,revision:3,publication_id:'anchor-new'});
  const next={...anchor,id:'anchor-2',center:[180,20],created_at_ms:90,activates_at_ms:90,expires_at_ms:3090};
  Object.assign(replace.payload,{base_publication_id:'anchor-full',base_revision:2,delta:{logical_ms:90,areas:[next]}});
  assert.deepEqual(r.apply(replace).areas,[next]);
});

test('cast anchor relocation advances only its center and recast count through delta and reconnect', () => {
  const {initial,anchor}=anchorFull();
  anchor.anchor_recast={action_id:'placement',expires_at_ms:1800,max_count:2,used_count:0};
  const replica=new ReplicaV6(key,'A');
  const before=replica.apply(initial);
  const update=delta();
  const relocated=structuredClone(anchor);
  relocated.center=[180,20];
  relocated.anchor_recast.used_count=1;
  update.payload.delta.areas=[relocated];
  for(const corrupt of [
    a=>a.anchor_recast.used_count=0,
    a=>a.anchor_recast.used_count=3,
    a=>a.anchor_recast.action_id='forged',
    a=>a.anchor_recast.expires_at_ms=1810,
    a=>a.anchor_recast.expires_at_ms=3001,
    a=>a.anchor_recast.max_count=3,
    a=>a.expires_at_ms=3010,
    a=>a.center=anchor.center,
    a=>a.anchor_recast=null,
    a=>delete a.anchor_recast,
  ]){
    const bad=structuredClone(update);
    corrupt(bad.payload.delta.areas[0]);
    assert.throws(()=>replica.apply(bad),code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot,before);
  }
  assert.deepEqual(replica.apply(update).areas,[relocated]);
  const reconnect=structuredClone(initial);
  Object.assign(reconnect,{connection:{id:'anchor-resume',generation:2},stream_epoch:2,
    revision:3,publication_id:'anchor-resume-full'});
  Object.assign(reconnect.payload.snapshot,{logical_ms:1800,areas:[relocated]});
  assert.deepEqual(replica.apply(reconnect).areas,[relocated]);
  const tooLate=structuredClone(reconnect);
  Object.assign(tooLate,{revision:4,publication_id:'anchor-late'});
  tooLate.payload.snapshot.areas[0].anchor_recast.used_count=2;
  tooLate.payload.snapshot.areas[0].center=[200,20];
  assert.throws(()=>replica.apply(tooLate),code('INVALID_FRAME'));
  assert.deepEqual(replica.snapshot.areas,[relocated]);
});

test('cast anchor relocation may coalesce multiple moves back to the original center', () => {
  const {initial,anchor}=anchorFull();
  anchor.anchor_recast={action_id:'placement',expires_at_ms:1800,max_count:2,used_count:0};
  const replica=new ReplicaV6(key,'A');
  replica.apply(initial);
  const update=delta();
  const relocated=structuredClone(anchor);
  relocated.anchor_recast.used_count=2;
  update.payload.delta.areas=[relocated];
  assert.deepEqual(replica.apply(update).areas,[relocated]);
});

test('attached fields follow the same owned anchor in deltas and reject broken links atomically', () => {
  const {initial,anchor}=anchorFull();
  anchor.anchor_recast={action_id:'placement',expires_at_ms:1800,max_count:2,used_count:0};
  const field={...structuredClone(anchor),id:'field-1',skill_id:'pulse',mechanism:'field',
    attached_anchor_id:anchor.id,radius:60,activates_at_ms:90,expires_at_ms:540,
    interval_ms:90,next_tick_at_ms:180,total_ticks:4,
    contact_policy:areaFull('field').payload.snapshot.areas[0].contact_policy};
  delete field.anchor_recast;
  initial.payload.snapshot.areas.push(field);
  const r=new ReplicaV6(key,'A'),before=r.apply(initial);
  const d=delta(), moved=structuredClone(anchor), movedField=structuredClone(field);
  moved.center=[180,20];
  moved.anchor_recast.used_count=1;
  movedField.center=moved.center;
  d.payload.delta.areas=[moved,movedField];
  for(const corrupt of [
    v=>v.shift(),
    v=>v[1].center=field.center,
    v=>v[1].attached_anchor_id='other',
    v=>v[1].attached_anchor_id=null,
    v=>delete v[1].attached_anchor_id,
    v=>v[1].expires_at_ms+=1,
    v=>v[0].attached_anchor_id=v[0].id,
    v=>v[1].mechanism='circle',
  ]){
    const bad=structuredClone(d);
    corrupt(bad.payload.delta.areas);
    assert.throws(()=>r.apply(bad),code('INVALID_FRAME'));
    assert.deepEqual(r.snapshot,before);
  }
  assert.deepEqual(r.apply(d).areas,[moved,movedField]);
});

test('cast anchor behavior lists are validated and remain fixed until the instance ends', () => {
  const {initial,anchor}=anchorFull();
  anchor.anchor_recast={action_id:'placement',expires_at_ms:1800,max_count:2,used_count:0,
    behaviors:['return_to_source','end']};
  for(const behaviors of [null,[],['end','end'],['redirect'],['end',4]]){
    const bad=structuredClone(initial);
    bad.payload.snapshot.areas[0].anchor_recast.behaviors=behaviors;
    assert.throws(()=>new ReplicaV6(key,'A').apply(bad),code('INVALID_FRAME'));
  }
  const endOnly=structuredClone(initial);
  endOnly.payload.snapshot.areas[0].anchor_recast.behaviors=['end'];
  endOnly.payload.snapshot.areas[0].anchor_recast.used_count=1;
  assert.throws(()=>new ReplicaV6(key,'A').apply(endOnly),code('INVALID_FRAME'));
  const r=new ReplicaV6(key,'A'),before=r.apply(initial);
  const moved=structuredClone(anchor);
  moved.center=[10,20];
  moved.anchor_recast.used_count=1;
  const bad=delta();
  bad.payload.delta.areas=[structuredClone(moved)];
  bad.payload.delta.areas[0].anchor_recast.behaviors=['relocate','end'];
  assert.throws(()=>r.apply(bad),code('INVALID_FRAME'));
  assert.deepEqual(r.snapshot,before);
  const returned=delta();
  returned.payload.delta.areas=[moved];
  assert.deepEqual(r.apply(returned).areas,[moved]);
  const ended=delta();
  Object.assign(ended,{publication_id:'delta-3',revision:3});
  Object.assign(ended.payload,{base_publication_id:'delta-2',base_revision:2,delta:{logical_ms:60,areas:[]}});
  assert.deepEqual(r.apply(ended).areas,[]);
});

test('contested equipment publishes holder changes and keeps the effective profile private', () => {
  const initial=full(), snapshot=initial.payload.snapshot;
  const node={id:'claim',position:[10,20],revision:1,state:'available'};
  snapshot.contested_equipment=[node];
  const replica=new ReplicaV6(key,'A');
  replica.apply(initial);
  const update=delta(), state={phase:'active',active_at_ms:0,extraction_point:[30,40],extraction_open:false};
  const held={...node,state:'held',revision:2,holder:entity};
  update.payload.delta.state={...state,contested_equipment:[held]};
  assert.throws(()=>replica.apply(update),code('INVALID_FRAME'));
  update.payload.delta.own={...snapshot.self,world_weapon_profile:'temporary'};
  assert.equal(replica.apply(update).self.world_weapon_profile,'temporary');
  const replacement=full();
  Object.assign(replacement,{publication_id:'full-resync',revision:3,stream_epoch:2,
    connection:{id:'new',generation:2}});
  Object.assign(replacement.payload.snapshot,{logical_ms:30,contested_equipment:[held]});
  replacement.payload.snapshot.self.world_weapon_profile='temporary';
  assert.deepEqual(replica.apply(replacement).contested_equipment,[held]);
  const forged=structuredClone(replacement);
  Object.assign(forged,{publication_id:'wrong-holder',revision:4});
  forged.payload.snapshot.contested_equipment[0].holder={...entity,generation:2};
  assert.throws(()=>replica.apply(forged),code('INVALID_FRAME'));
  assert.deepEqual(replica.snapshot.contested_equipment,[held]);
});

test('spectators see the contest marker with no access to private equipment data', () => {
  const node={id:'claim',position:[10,20],revision:2,state:'held',holder:entity};
  const frame={key,slot:1,publication_id:'first',connection:{id:'viewer',generation:1},stream_epoch:1,revision:1,
    payload:{kind:'full',reason:'initial',snapshot:{phase:'active',active_at_ms:0,logical_ms:30,
      members:[{entity,position:[10,20],hp:100,max_hp:100,phase:'active'}],
      monsters:[],projectiles:[],resources:[],areas:[],events:[],contested_equipment:[node]}}};
  const replica=new SpectatorReplicaV6(key,1);
  replica.apply(frame);
  const dropped={id:node.id,position:[25,20],revision:3,state:'dropped'};
  const patch={...frame,publication_id:'next',revision:2,payload:{kind:'delta',
    base_publication_id:'first',base_revision:1,delta:{logical_ms:60,contested_equipment:[dropped]}}};
  const corrupt=structuredClone(patch);
  corrupt.payload.delta.contested_equipment[0].world_weapon_profile='private';
  assert.throws(()=>replica.apply(corrupt),code('INVALID_FRAME'));
  assert.deepEqual(replica.apply(patch).contested_equipment,[dropped]);
});

test('zone exposure updates only the owner and validates node revisions and expiry', () => {
  const initial=full(), view=initial.payload.snapshot;
  const node={id:'site',position:[10,20],revision:1,ready_at_ms:0};
  view.self.zone_exposure={band_id:'band',stacks:2,max_stacks:4,nodes:[node]};
  const replica=new ReplicaV6(key,'A'),before=replica.apply(initial);
  const change=delta();
  change.payload.delta.own={...view.self,zone_exposure:{
    band_id:'band',stacks:2,max_stacks:4,boost_ends_at_ms:90,
    nodes:[{...node,revision:2,ready_at_ms:60}],
  }};
  for (const corrupt of [
    exposure=>exposure.stacks=5,
    exposure=>exposure.boost_ends_at_ms=30,
    exposure=>exposure.nodes[0].revision=1,
    exposure=>exposure.nodes.push(exposure.nodes[0]),
  ]) {
    const bad=structuredClone(change);
    corrupt(bad.payload.delta.own.zone_exposure);
    assert.throws(()=>replica.apply(bad),code('INVALID_FRAME'));
    assert.deepEqual(replica.snapshot,before);
  }
  assert.equal(replica.apply(change).self.zone_exposure.boost_ends_at_ms,90);
  const reconnect=structuredClone(initial);
  Object.assign(reconnect,{publication_id:'zone-reconnect',revision:3,stream_epoch:2,
    connection:{id:'new',generation:2}});
  reconnect.payload.snapshot.logical_ms=60;
  reconnect.payload.snapshot.self.zone_exposure=change.payload.delta.own.zone_exposure;
  assert.equal(replica.apply(reconnect).self.zone_exposure.nodes[0].revision,2);
  const expired=structuredClone(reconnect);
  Object.assign(expired,{publication_id:'zone-expired',revision:4});
  expired.payload.snapshot.logical_ms=90;
  assert.throws(()=>replica.apply(expired),code('INVALID_FRAME'));
  delete expired.payload.snapshot.self.zone_exposure.boost_ends_at_ms;
  assert.equal(replica.apply(expired).self.zone_exposure.stacks,2);
});
