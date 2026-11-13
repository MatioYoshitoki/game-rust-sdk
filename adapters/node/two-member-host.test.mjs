import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync,rmSync,readFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createHash,randomUUID} from 'node:crypto';
import {TwoMemberDatabase,checkpointDigest} from './two-member-host.mjs';
import {equippedWeaponRewardAssetId} from './asset-store.mjs';

const hash=v=>createHash('sha256').update(typeof v==='string'?v:JSON.stringify(v)).digest('hex');
function fixture(t,configure=()=>{},configureAccount=()=>{}){const dir=mkdtempSync(join(tmpdir(),'duo-store-'));const db=new TwoMemberDatabase(join(dir,'test.sqlite'));t.after(()=>{if(db.db.isOpen)db.close();rmSync(dir,{recursive:true,force:true});});
  const config={
    id:'test-config',item_id:'test-item',
    asset_contract:{entry_cost_asset_id:'entry-credit',reward_asset_id:'experience'},
  };
  configure(config);
  const raw=JSON.stringify(config);
  const accounts=[['A',0],['B',10]].map(([id,revision])=>({
    participant_id:id,principal:`principal-${id}`,revision,
    stats:{hp_points:1,spirit_points:1,atk_points:1,def_points:1,spd_points:1},
    assets:{'entry-credit':100,'test-item':100},
  }));
  accounts.forEach(configureAccount);
  const record={id:randomUUID(),raw,config,accounts,originalCreate:{config_hash:hash(raw),key:{id:'room',incarnation:1}}};db.create(record);return {db,record,dir};}
const load=pid=>({operation_id:`load-${pid}`,attempt_id:1,body:{kind:'load_assets'}});
const admit=(pid,revision)=>({operation_id:`admit-${pid}`,attempt_id:1,body:{kind:'commit_assets',transaction:{kind:'admission',expected_revision:revision,debits:[{asset_id:'entry-credit',quantity:1},{asset_id:'test-item',quantity:5}]}}});
function output(record,pid,request,id='o1'){return {key:record.originalCreate.key,id,logical_ms:0,body:{kind:'business_request_v4',participant_id:pid,principal:`principal-${pid}`,request}};}
function checkpointOutput(record,operations=[],seq=1,id='cp1'){
  const state={schema_version:1,identity_sha256:'x',logical_ms:0,output_sequence:seq,recovery_generation:0,members:record.accounts.map(a=>({participant_id:a.participant_id,checkpoint:{state:{operations:operations.filter(([pid])=>pid===a.participant_id).map(([,request])=>({request,terminal:null}))}}}))};
  return {key:record.originalCreate.key,id,logical_ms:0,body:{kind:'responsibility_v4',checkpoint:{state,sha256:checkpointDigest(state)}}};
}

test('two accounts retain independent revisions and stable debit proofs',t=>{const {db,record}=fixture(t);for(const [pid,revision]of[['A',0],['B',10]]){const facts=db.execute(record.id,pid,load(pid));assert.equal(facts.facts.revision,revision);const r=admit(pid,revision);assert.deepEqual(db.execute(record.id,pid,r),db.execute(record.id,pid,r));}const views=db.views(record.id);assert.deepEqual(views.map(v=>v.revision),[1,11]);assert.ok(views.every(v=>v.admissions===1&&v.balances['test-item']===95));});
test('output batch principal mismatch rolls back both member registrations',t=>{const {db,record}=fixture(t);const a=output(record,'A',admit('A',0),'a'),b=output(record,'B',admit('B',10),'b');b.body.principal='principal-A';assert.throws(()=>db.persist(record.id,[a,b]),/PRINCIPAL/);assert.equal(db.state(record.id,'A').requests.length,0);assert.equal(db.db.prepare('SELECT COUNT(*) n FROM duo_outputs').get().n,0);});
test('cross-match output is rejected before any durable I/O',t=>{const {db,record}=fixture(t);const o=output(record,'A',admit('A',0));o.key={id:'other',incarnation:1};assert.throws(()=>db.persist(record.id,[o]),/MATCH/);assert.equal(db.state(record.id,'A').requests.length,0);});
test('frozen body and output identities cannot be reassigned',t=>{const {db,record}=fixture(t);const o=output(record,'A',admit('A',0));db.persist(record.id,[o]);const changed=structuredClone(o);changed.body.request.body.transaction.debits[1].quantity=99;assert.throws(()=>db.persist(record.id,[changed]),/OUTPUT_ID/);changed.id='new-output';assert.throws(()=>db.persist(record.id,[changed]));assert.equal(db.state(record.id,'A').requests[0][1].body.transaction.debits[1].quantity,5);});
test('storage rollback and committed lost receipt have distinct durable proofs',t=>{const {db,record}=fixture(t);db.execute(record.id,'A',load('A'));const r=admit('A',0);assert.throws(()=>db.execute(record.id,'A',r,{beforeCommit(){throw new Error('rollback')}}));assert.equal(db.state(record.id,'A').admissions,0);assert.equal(db.state(record.id,'A').balances['test-item'],100);const outcome=db.execute(record.id,'A',r,{dropReceipt:true});assert.equal(outcome.kind,'unknown');const query={operation_id:'query',attempt_id:1,body:{kind:'query_operation',operation_id:r.operation_id}};assert.equal(db.execute(record.id,'A',query).kind,'committed');assert.equal(db.state(record.id,'A').admissions,1);});
test('checkpoint-only frozen requests are durable and old checkpoints do not regress',t=>{const {db,record}=fixture(t);const op=admit('A',0);db.persist(record.id,[checkpointOutput(record,[['A',op]],50,'cp50')]);assert.equal(db.state(record.id,'A').requests.length,1);db.persist(record.id,[checkpointOutput(record,[],20,'cp20')]);assert.equal(db.room(record.id).checkpoint.state.output_sequence,50);});
test('recovery fences only unproved attempts and keeps committed member proof',t=>{const {db,record}=fixture(t);for(const pid of['A','B'])db.execute(record.id,pid,load(pid));const a=admit('A',0),b=admit('B',10);db.execute(record.id,'A',a);db.persist(record.id,[checkpointOutput(record,[['A',a],['B',b]])]);const first=db.recovery(record.id),second=db.recovery(record.id,first.checkpoint);assert.equal(first.generation,1);assert.equal(second.generation,2);assert.equal(db.state(record.id,'A').fenced.length,0);assert.deepEqual(db.state(record.id,'B').fenced,['admit-B/1']);assert.equal(db.state(record.id,'B').admissions,0);});
test('frozen catalog materializes original bytes and checksum',t=>{const {db,record}=fixture(t);const manifest=db.frozenManifest(record.id);const path=join(manifest.slice(0,manifest.lastIndexOf('/')),'arena.json');assert.equal(readFileSync(path,'utf8'),record.raw);assert.equal(hash(readFileSync(path,'utf8')),record.originalCreate.config_hash);const corrupt=db.room(record.id);corrupt.raw+=' ';db.writeRoom(corrupt);assert.throws(()=>db.frozenManifest(record.id),/FROZEN_CONFIG/);});

test('zero-fee admission and late compensation commit stable empty wallet changes',t=>{
  const {db,record}=fixture(t,config=>{
    config.params={entry_years:0};
    config.settlement_progression={age_asset_id:'elapsed'};
  },account=>{account.assets={};});
  const id=record.id;
  assert.deepEqual(db.execute(id,'A',load('A')).facts.assets,[]);
  const request=admit('A',0);
  request.body.transaction.debits=[];
  assert.equal(db.execute(id,'A',request,{dropReceipt:true}).kind,'unknown');
  const query={operation_id:'zero-admit-query',attempt_id:1,body:{kind:'query_operation',operation_id:request.operation_id}};
  assert.equal(db.execute(id,'A',query).revision,1);
  const refund={operation_id:'zero-refund',attempt_id:1,body:{kind:'commit_assets',transaction:{
    kind:'compensation',original_operation_id:request.operation_id,credits:[],
  }}};
  const proof=db.execute(id,'A',refund);
  assert.equal(proof.revision,2);
  assert.deepEqual(db.execute(id,'A',refund),proof);
  const state=db.state(id,'A');
  assert.deepEqual(state.balances,{});
  assert.deepEqual([state.years,state.admissions,state.compensations],[0,1,1]);
});

test('instance identity and stack credits share one durable settlement', t => {
  const {db,record}=fixture(t,config=>{
    config.world_loot={instances:{'unique-item':{quality:8,relic_eligible:true}}};
  });
  const item={instance_id:`${'a'.repeat(64)}-0000000000000001`,item_id:'unique-item',quality:8};
  const request={operation_id:'settle-A',attempt_id:1,body:{kind:'commit_assets',transaction:{
    kind:'result_settlement',expected_revision:1,credits:[{asset_id:'test-item',quantity:5}],
    summary:{instances:[item]}
  }}};
  db.execute(record.id,'A',load('A'));
  db.execute(record.id,'A',admit('A',0));
  assert.throws(()=>db.execute(record.id,'A',request,{beforeCommit(){throw new Error('rollback')}}),/rollback/);
  assert.equal(db.views(record.id)[0].instances,undefined);
  assert.equal(db.views(record.id)[0].balances['test-item'],95);
  const committed=db.execute(record.id,'A',request);
  assert.equal(committed.kind,'committed');
  assert.deepEqual(db.execute(record.id,'A',request),committed);
  const view=db.views(record.id)[0];
  assert.deepEqual(view.instances,[item]);
  assert.equal(view.settlements,1);
  assert.equal(view.balances['test-item'],100);
});

test('reward balance conflict leaves the entire settlement unapplied', t => {
  const {db,record}=fixture(t);
  db.execute(record.id,'A',load('A'));
  db.execute(record.id,'A',admit('A',0));
  const tx={kind:'result_settlement',expected_revision:1,
    credits:[{asset_id:'experience',quantity:3},{asset_id:'test-item',quantity:5}],
    summary:{rewards:[{asset_id:'experience',base_points:9,requested:9,awarded:3,
      balance_before:1,balance_after:4}]}};
  const request={operation_id:'settle-A',attempt_id:1,body:{kind:'commit_assets',transaction:tx}};
  assert.equal(db.execute(record.id,'A',request).kind,'refused');
  const view=db.views(record.id)[0];
  assert.equal(view.balances['test-item'],95);
  assert.equal(view.balances.experience,undefined);
  assert.equal(view.revision,1);
  assert.equal(view.settlements,0);
});

test('growth settlement stores only admitted capacity and fences stale region and wallet changes', t => {
  const {db,record}=fixture(t,config=>{
    config.settlement_progression={age_asset_id:'age',instance_capacity:1};
    config.world_loot={instances:{'unique-item':{quality:8}}};
  },account=>{
    account.assets.age=18;
    account.location={region_id:'origin',revision:7};
  });
  const id=record.id;
  const a=admit('A',0);
  a.body.transaction.clock={
    elapsed:{asset_id:'age',before:18,after:19},
    remaining:{asset_id:'entry-credit',before:100,after:99},
  };
  const loaded=db.execute(id,'A',load('A')).facts;
  assert.equal(loaded.instance_count,0);
  assert.deepEqual(loaded.location,{region_id:'origin',revision:7});
  assert.equal(db.execute(id,'A',a).kind,'committed');
  const items=[1,2].map(n=>({instance_id:`${'a'.repeat(64)}-${String(n).padStart(16,'0')}`,item_id:'unique-item',quality:8}));
  const request={operation_id:'settle-A',attempt_id:1,body:{kind:'commit_assets',transaction:{
    kind:'result_settlement',expected_revision:1,
    credits:[{asset_id:'experience',quantity:5},{asset_id:'test-item',quantity:2}],
    summary:{
      inventory:[{item_id:'test-item',kept:5}],instances:items,
      rewards:[{asset_id:'experience',balance_before:0,balance_after:5}],
      progression:{
        storage:[{asset_id:'test-item',before:95,after:97,stored:2,overflow:3}],
        instance_storage:{before:0,after:1,stored_ids:[items[0].instance_id],overflow_ids:[items[1].instance_id]},
        conversion:{rule_id:'threshold',changes:[{asset_id:'experience',before:5,after:2}]},
        region:{before:{region_id:'origin',revision:7},after:{region_id:'destination',revision:8}},
      }
    }
  }}};
  const admitted=db.state(id,'A');
  assert.throws(()=>db.execute(id,'A',request,{beforeCommit(){throw new Error('rollback')}}),/rollback/);
  assert.deepEqual(db.state(id,'A'),admitted);
  for(const change of [
    state=>{state.location.revision=8;},
    state=>{state.balances.experience=1;},
    state=>{state.revision=2;},
  ]){
    const concurrent=structuredClone(admitted);
    change(concurrent);
    db.writeState(id,'A',concurrent);
    assert.equal(db.execute(id,'A',request).kind,'refused');
    const view=db.views(id)[0];
    assert.deepEqual(view.balances,concurrent.balances);
    assert.deepEqual(view.location,concurrent.location);
    assert.equal(view.revision,concurrent.revision);
    assert.equal(view.settlements,0);
    assert.equal(view.instances,undefined);
  }
  db.writeState(id,'A',admitted);
  assert.equal(db.execute(id,'A',request,{dropReceipt:true}).kind,'unknown');
  const query={operation_id:'query-growth',attempt_id:1,body:{kind:'query_operation',operation_id:request.operation_id}};
  const proof=db.execute(id,'A',query);
  assert.equal(proof.kind,'committed');
  assert.deepEqual(db.execute(id,'A',request),proof);
  const view=db.views(id)[0];
  assert.deepEqual(view.balances,{age:19,'entry-credit':99,experience:2,'test-item':97});
  assert.deepEqual(view.location,{region_id:'destination',revision:8});
  assert.deepEqual(view.instances,[items[0]]);
  assert.equal(view.settlements,1);

  const b=admit('B',10);
  b.body.transaction.clock=structuredClone(a.body.transaction.clock);
  db.execute(id,'B',load('B'));
  assert.equal(db.execute(id,'B',b).kind,'committed');
  const reverse=({asset_id,before,after})=>({asset_id,before:after,after:before});
  const refund={operation_id:'refund-B',attempt_id:1,body:{kind:'commit_assets',transaction:{
    kind:'compensation',original_operation_id:b.operation_id,credits:b.body.transaction.debits,
    clock:{elapsed:reverse(b.body.transaction.clock.elapsed),remaining:reverse(b.body.transaction.clock.remaining)},
  }}};
  const refunded=db.execute(id,'B',refund);
  assert.equal(refunded.kind,'committed');
  assert.deepEqual(db.execute(id,'B',refund),refunded);
  assert.deepEqual(db.views(id)[1].balances,record.accounts[1].assets);
  assert.equal(db.views(id)[1].compensations,1);
});

test('shared warehouse and same-region migration commit with an independent position fence', t => {
  const unique=n=>({instance_id:`${'b'.repeat(64)}-${String(n).padStart(16,'0')}`,item_id:'unique-item',quality:8});
  const {db,record,dir}=fixture(t,config=>{
    config.settlement_progression={warehouse_slots:{capacity:28,stack_sizes:{'test-item':4}},destinations:{extract:'origin'}};
    config.world_loot={instances:{'unique-item':{quality:8}}};
  },account=>{
    account.warehouse_other_slots=2;
    account.instances=[unique(1)];
    account.location={region_id:'origin',revision:7};
  });
  const id=record.id;
  const loaded=db.execute(id,'A',load('A')).facts;
  assert.equal(loaded.instance_count,1);
  assert.equal(loaded.warehouse_other_slots,2);
  assert.equal(db.execute(id,'A',admit('A',0)).kind,'committed');
  const position=(revision,sequence,xy)=>({region_id:'origin',revision,sequence,position:xy});
  assert.equal(db.savePosition(id,'A',position(7,2,[3,4])),true);
  assert.equal(db.savePosition(id,'A',position(7,1,[7,8])),false);
  assert.equal(db.savePosition(id,'A',position(7,2,[3,4])),true);
  assert.equal(db.savePosition(id,'A',position(7,2,[3,5])),false);
  assert.equal(db.state(id,'A').revision,1);
  const settlement={operation_id:'warehouse-core',attempt_id:1,body:{kind:'commit_assets',transaction:{
    kind:'result_settlement',expected_revision:1,credits:[{asset_id:'test-item',quantity:2}],
    summary:{inventory:[{item_id:'test-item',kept:5}],instances:[unique(2)],progression:{
      storage:[{asset_id:'test-item',before:95,after:97,stored:2,overflow:3}],
      instance_storage:{before:1,after:1,stored_ids:[],overflow_ids:[unique(2).instance_id]},
      warehouse_slots:{before:27,after:28},
      region:{before:{region_id:'origin',revision:7},after:{region_id:'origin',revision:8}},
    }},
  }}};
  const admitted=db.state(id,'A');
  const conflict=structuredClone(admitted);
  conflict.warehouseOtherSlots++;
  db.writeState(id,'A',conflict);
  assert.equal(db.execute(id,'A',settlement).kind,'refused');
  assert.deepEqual(db.state(id,'A').savedPosition,admitted.savedPosition);
  assert.equal(db.state(id,'A').balances['test-item'],95);
  assert.equal(db.state(id,'A').location.revision,7);
  db.writeState(id,'A',admitted);
  assert.throws(()=>db.execute(id,'A',settlement,{beforeCommit(){throw new Error('rollback')}}),/rollback/);
  assert.deepEqual(db.state(id,'A'),admitted);
  assert.equal(db.execute(id,'A',settlement,{dropReceipt:true}).kind,'unknown');
  assert.equal(db.state(id,'A').savedPosition,undefined);
  assert.equal(db.savePosition(id,'A',position(7,3,[9,10])),false);
  assert.equal(db.savePosition(id,'A',position(8,1,[1,2])),true);
  db.close();
  const restarted=new TwoMemberDatabase(join(dir,'test.sqlite'));
  try {
    const query={operation_id:'warehouse-query',attempt_id:1,body:{kind:'query_operation',operation_id:settlement.operation_id}};
    const proof=restarted.execute(id,'A',query);
    assert.equal(proof.kind,'committed');
    assert.deepEqual(restarted.execute(id,'A',settlement),proof);
    assert.equal(restarted.savePosition(id,'A',position(7,4,[9,10])),false);
    assert.equal(restarted.savePosition(id,'A',position(8,1,[1,3])),false);
    assert.equal(restarted.savePosition(id,'A',position(8,2,[2,3])),true);
    const state=restarted.state(id,'A');
    assert.deepEqual(state.savedPosition,{sequence:2,position:[2,3]});
    assert.deepEqual(state.location,{region_id:'origin',revision:8});
    assert.equal(state.balances['test-item'],97);
    assert.deepEqual(state.instances,[unique(1)]);
    assert.equal(state.settlements,1);
  } finally {restarted.close();}
});

test('settlement facts require a core proof and commit independently once', t => {
  const {db,record}=fixture(t,config=>{config.settlement_facts={extract:['second','first']};});
  const id=record.id;
  const facts={operation_id:'facts-A',attempt_id:1,body:{kind:'append_facts',
    settlement_operation_id:'settle-A',result_id:'extract',fact_ids:['first','second']}};
  const core={operation_id:'settle-A',attempt_id:1,body:{kind:'commit_assets',transaction:{
    kind:'result_settlement',expected_revision:1,credits:[{asset_id:'test-item',quantity:5}],
    summary:{result:{result_id:'extract',policy:'preserve'}}
  }}};
  db.execute(id,'A',load('A'));
  db.execute(id,'A',admit('A',0));
  assert.throws(()=>db.execute(id,'A',facts),/MISSING_CORE_PROOF/);
  db.execute(id,'A',core);
  assert.throws(()=>db.execute(id,'A',facts,{beforeCommit(){throw new Error('rollback')}}),/rollback/);
  assert.equal(db.views(id)[0].appended_facts,undefined);
  assert.equal(db.views(id)[0].settlements,1);
  assert.equal(db.execute(id,'A',facts,{notCommitted:true}).kind,'not_committed');
  assert.equal(db.execute(id,'A',facts).kind,'not_committed');
  const retry={...facts,attempt_id:2};
  const proof=db.execute(id,'A',retry);
  assert.equal(proof.kind,'facts_appended');
  assert.deepEqual(db.execute(id,'A',retry),proof);
  assert.throws(()=>db.execute(id,'A',{...facts,operation_id:'duplicate'}),/DUPLICATE_SETTLEMENT_FACTS/);
  const view=db.views(id)[0];
  assert.equal(view.appended_facts.length,1);
  assert.equal(view.revision,2);
  assert.equal(view.balances['test-item'],100);
  assert.equal(view.settlements,1);
});

test('ordered growth conversions reject a stale later step without committing an earlier step',t=>{
  const {db,record}=fixture(t,config=>{
    config.settlement_progression={conversion_mode:'ordered'};
  },account=>Object.assign(account.assets,{age:18,pending:0}));
  const id=record.id;
  db.execute(id,'A',load('A'));
  db.execute(id,'A',admit('A',0));
  const admitted=db.state(id,'A');
  const request={operation_id:'ordered-settle',attempt_id:1,body:{kind:'commit_assets',transaction:{
    kind:'result_settlement',expected_revision:1,credits:[{asset_id:'test-item',quantity:5}],
    summary:{result:{result_id:'death',policy:'attrition'},progression:{conversion_steps:[
      {rule_id:'charge',changes:[{asset_id:'age',before:18,after:117},{asset_id:'entry-credit',before:99,after:0}]},
      {rule_id:'exhausted',changes:[{asset_id:'entry-credit',before:0,after:0},{asset_id:'pending',before:0,after:1}]},
    ]}},
  }}};
  const stale=structuredClone(admitted);
  stale.balances.pending=2;
  db.writeState(id,'A',stale);
  assert.equal(db.execute(id,'A',request).kind,'refused');
  assert.deepEqual(db.views(id)[0].balances,stale.balances);
  assert.equal(db.views(id)[0].settlements,0);
  db.writeState(id,'A',admitted);
  assert.throws(()=>db.execute(id,'A',request,{beforeCommit(){throw new Error('rollback')}}),/rollback/);
  assert.deepEqual(db.state(id,'A'),admitted);
  const proof=db.execute(id,'A',request);
  assert.equal(proof.kind,'committed');
  assert.deepEqual(db.execute(id,'A',request),proof);
  assert.deepEqual(db.views(id)[0].balances,{age:117,'entry-credit':0,pending:1,'test-item':100});
});

test('growth settlement rejects stale threshold, scoped wallet, earning rate and adjustment balances atomically',t=>{
  const scoped=equippedWeaponRewardAssetId('channel','instance');
  const {db,record}=fixture(t,config=>{
    config.settlement_progression={threshold_progress:{
      stage_asset_id:'stage',total_asset_id:'experience',progress_asset_id:'progress',thresholds:[0,8],
    }};
  },account=>Object.assign(account.assets,{stage:0,progress:2,experience:3,[scoped]:4,'earning-rate':15000,escrow:7,uses:1}));
  const id=record.id;
  db.execute(id,'A',load('A'));
  db.execute(id,'A',admit('A',0));
  const admitted=db.state(id,'A');
  const request={operation_id:'threshold-settle',attempt_id:1,body:{kind:'commit_assets',transaction:{
    kind:'result_settlement',expected_revision:1,credits:[{asset_id:scoped,quantity:6},{asset_id:'experience',quantity:5}],
    summary:{result:{result_id:'extract',policy:'preserve'},
      rewards:[{asset_id:scoped,base_points:6,requested:6,awarded:6,balance_before:4,balance_after:10,
        adjustment:{original_requested:6,deferred_payout:5,changes:[{asset_id:'escrow',before:7,after:0},{asset_id:'uses',before:1,after:0}]}}],progression:{
      threshold_progress:{stage_index:0,total:8,current_threshold:0,next_threshold:8,balance_met:true,
        progress:{asset_id:'progress',before:2,after:8}},
      reward_earnings:{rates:[{asset_id:'earning-rate',bps:15000}],kills:[]},
    }},
  }}};
  for(const asset of ['stage','experience','progress',scoped,'earning-rate','escrow','uses']){
    const concurrent=structuredClone(admitted);
    concurrent.balances[asset]++;
    db.writeState(id,'A',concurrent);
    assert.equal(db.execute(id,'A',request).kind,'refused');
    assert.deepEqual(db.views(id)[0].balances,concurrent.balances);
    assert.equal(db.views(id)[0].settlements,0);
  }
  db.writeState(id,'A',admitted);
  const proof=db.execute(id,'A',request);
  assert.equal(proof.kind,'committed');
  assert.deepEqual(db.execute(id,'A',request),proof);
  assert.equal(db.views(id)[0].balances.stage,0);
  assert.equal(db.views(id)[0].balances.progress,8);
  assert.equal(db.views(id)[0].balances[scoped],10);
  assert.equal(db.views(id)[0].balances.escrow,0);
  assert.equal(db.views(id)[0].balances.uses,0);
  assert.equal(db.views(id)[0].settlements,1);
});
