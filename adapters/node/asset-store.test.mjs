import { test } from 'node:test';
import assert from 'node:assert/strict';
import { AssetStore } from './asset-store.mjs';
const config={item_id:'ore',asset_contract:{entry_cost_asset_id:'ticket',reward_asset_id:'xp'}};
const account={principal:'principal-A',revision:0,stats:{hp_points:100,spirit_points:50,atk_points:10,def_points:10,spd_points:8},assets:{ore:10,ticket:1,xp:0}};
const request=(id,body)=>({operation_id:id,attempt_id:1,body});
const transaction=(id,value)=>request(id,{kind:'commit_assets',transaction:value});
function store(){const s=new AssetStore(account.stats,{config,account});s.execute(request('load',{kind:'load_assets'}));return s;}
test('asset transaction is atomic on insufficient balance and repeats original proof',()=>{
  const s=store();const before={...s.balances};
  assert.equal(s.execute(transaction('bad',{kind:'admission',expected_revision:0,debits:[{asset_id:'ore',quantity:2},{asset_id:'ticket',quantity:2}]})).kind,'refused');assert.deepEqual(s.balances,before);
  const r=transaction('entry',{kind:'admission',expected_revision:0,debits:[{asset_id:'ore',quantity:2},{asset_id:'ticket',quantity:1}]});const proof=s.execute(r);assert.deepEqual(s.execute(r),proof);assert.equal(s.admissions,1);assert.deepEqual(s.balances,{ore:8,ticket:0,xp:0});
});
test('asset compensation matches the committed frozen debits exactly',()=>{
  const s=store();s.execute(transaction('entry',{kind:'admission',expected_revision:0,debits:[{asset_id:'ore',quantity:2},{asset_id:'ticket',quantity:1}]}));
  assert.throws(()=>s.execute(transaction('bad-refund',{kind:'compensation',original_operation_id:'entry',credits:[{asset_id:'ore',quantity:3}]})));
  const r=transaction('refund',{kind:'compensation',original_operation_id:'entry',credits:[{asset_id:'ore',quantity:2},{asset_id:'ticket',quantity:1}]});s.execute(r);s.execute(r);assert.deepEqual(s.balances,account.assets);assert.equal(s.compensations,1);
});
test('asset result settlement and authorized reconciliation use frozen credits once',()=>{
  const s=store();const r=transaction('result',{kind:'result_settlement',expected_revision:0,credits:[{asset_id:'ore',quantity:3},{asset_id:'xp',quantity:7}],summary:{result:{result_id:'done',policy:'preserve'}}});s.refuse(r);
  const query=request('query',{kind:'query_operation',operation_id:'result'});assert.equal(s.execute(query).kind,'refused');
  const repair=request('review',{kind:'reconcile_operation',operation_id:'result'});const proof=s.execute(repair);assert.equal(proof.kind,'reconciled');assert.deepEqual(s.execute(repair),proof);assert.equal(s.settlements,1);assert.deepEqual(s.balances,{ore:13,ticket:1,xp:7});
});
test('missing commit is unknown unless explicitly fenced; UTF-8 account keys are canonical',()=>{
  const s=store();assert.equal(s.execute(request('query',{kind:'query_operation',operation_id:'absent'})).kind,'unknown');
  s.balances={'\ue000':1,'𐀀':1,'A':1};const facts=s.execute(request('load-2',{kind:'load_assets'})).facts;assert.deepEqual(facts.assets.map(r=>r.asset_id),['A','\ue000','𐀀']);
});
test('legal asset IDs that match JavaScript object properties are stored as own data',()=>{
  const c={item_id:'toString',asset_contract:{entry_cost_asset_id:'constructor',reward_asset_id:'__proto__'}};
  const a={...account,assets:{constructor:1,toString:10}};
  const s=new AssetStore(a.stats,{config:c,account:a});s.execute(request('load',{kind:'load_assets'}));
  s.execute(transaction('entry',{kind:'admission',expected_revision:0,debits:[{asset_id:'constructor',quantity:1},{asset_id:'toString',quantity:2}]}));
  s.execute(transaction('reward',{kind:'result_settlement',expected_revision:1,credits:[{asset_id:'__proto__',quantity:7},{asset_id:'toString',quantity:3}],summary:{result:{result_id:'done',policy:'preserve'}}}));
  assert.equal(Object.getPrototypeOf(s.balances),Object.prototype);assert.ok(Object.hasOwn(s.balances,'__proto__'));
  assert.equal(s.balances.__proto__,7);assert.equal(s.cultivation,7);assert.equal(s.warehouse,11);
});
