// Local reference account adapter. Execute only frozen Rust amounts, never gameplay formulas.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import { ControlledStore } from './controlled-store.mjs';

// Mirrors the SDK wallet key. Hosts can use it when loading instance balances.
export function equippedWeaponRewardAssetId(channel, instanceId) {
  assert.ok([channel,instanceId].every(id=>typeof id==='string'&&
    Buffer.byteLength(id)>0&&Buffer.byteLength(id)<=64&&!/[\u0000-\u001f\u007f-\u009f]/.test(id)));
  return '@scope/'+createHash('sha224').update(channel).update(Buffer.from([0])).update(instanceId).digest('hex');
}

export const businessKind = request => request.body.kind === 'load_assets' ? 'load_participant'
  : request.body.kind !== 'commit_assets' ? request.body.kind
  : ({ admission:'commit_admission', compensation:'compensate_admission', settlement:'commit_settlement', result_settlement:'commit_settlement' })[request.body.transaction.kind];
export const settlementEvent = kind => ['settlement_applied','asset_settlement_applied','result_settlement_applied'].includes(kind);
const compareIds = (a,b) => Buffer.compare(Buffer.from(a),Buffer.from(b));
const balance=(wallet,id)=>Object.hasOwn(wallet,id)?wallet[id]:0;
const quantity=q=>Number.isInteger(q)&&q>=0&&q<=4294967295;
const assign=(wallet,id,value)=>Object.defineProperty(wallet,id,{value,enumerable:true,writable:true,configurable:true});

function applyFrozenChanges(wallet,rows){
  const ids=rows.map(row=>row.asset_id);
  assert.deepEqual(ids,[...new Set(ids)].sort(compareIds));
  if(rows.some(row=>balance(wallet,row.asset_id)!==row.before||!quantity(row.after)))return false;
  for(const row of rows)assign(wallet,row.asset_id,row.after);
  return true;
}

function occupiedSlots(layout,wallet,instances,other){
  return Object.entries(layout.stack_sizes).reduce(
    (used,[id,size])=>used+Math.ceil(balance(wallet,id)/size),other+instances);
}

export class AssetStore extends ControlledStore {
  balances = null;
  instances = [];
  appendedFacts = [];
  location = null;
  savedPosition = null;
  warehouseOtherSlots = null;
  constructor(initialStats, options = {}) { super(initialStats, options.config?.asset_contract ? undefined : options.account); this.accountOptions = options; }
  execute(request, repair = false) {
    const kind = request.body.kind;
    if (!['load_assets','commit_assets','append_facts'].includes(kind)) {
      const original=kind==='reconcile_operation' ? this.requests.get(request.body.operation_id) : undefined;
      if (original && ['commit_assets','append_facts'].includes(original.body.kind)) {
        this.remember(request);
        const previous=this.journal.get(request.operation_id);if(previous)return previous;
        const proof=this.execute(original,true);
        if(!['committed','facts_appended'].includes(proof.kind))return {kind:'unknown'};
        this.reconciliations++;
        const result=proof.kind==='committed'?{...proof,kind:'reconciled'}:proof;
        this.journal.set(request.operation_id,result);return result;
      }
      return super.execute(request,repair);
    }
    this.remember(request);
    const previous=this.journal.get(request.operation_id);
    if(previous && (previous.kind!=='refused'||!repair))return previous;
    if(!repair && this.fenced.has(`${request.operation_id}/${request.attempt_id}`))return {kind:'not_committed'};
    const { account, config } = this.accountOptions;
    assert.ok(account && config?.asset_contract,'EXPLICIT_ASSET_ACCOUNT_REQUIRED');
    if (kind === 'append_facts') {
      const {settlement_operation_id,result_id,fact_ids}=request.body;
      const settlement=this.requests.get(settlement_operation_id)?.body.transaction;
      const routes=config.settlement_facts??{};
      assert.equal(this.journal.get(settlement_operation_id)?.kind,'committed','MISSING_CORE_PROOF');
      assert.equal(settlement?.kind,'result_settlement');
      assert.equal(settlement.summary.result.result_id,result_id);
      assert.ok(Object.hasOwn(routes,result_id));
      assert.deepEqual(fact_ids,[...routes[result_id]].sort(compareIds));
      assert.ok(!this.appendedFacts.some(row=>row.settlement_operation_id===settlement_operation_id),
        'DUPLICATE_SETTLEMENT_FACTS');
      const proof={kind:'facts_appended',commit_id:`facts-${request.operation_id}`};
      this.appendedFacts.push({operation_id:request.operation_id,...structuredClone(request.body),
        commit_id:proof.commit_id});
      this.journal.set(request.operation_id,proof);
      return proof;
    }
    if(this.balances===null) {
      assert.equal(this.revision,0,'MISSING_ASSET_BALANCES');
      this.balances=structuredClone(account.assets);
      this.revision=account.revision;
      this.location=structuredClone(account.location??null);
      if(config.settlement_progression?.warehouse_slots){
        this.instances=structuredClone(account.instances??[]);
        this.warehouseOtherSlots=account.warehouse_other_slots??null;
      }
    }
    let result;
    if(kind==='load_assets') {
      result={kind:'assets_loaded',facts:{principal:account.principal,revision:this.revision,...account.stats,
        assets:Object.entries(this.balances).sort(([a],[b])=>compareIds(a,b)).map(([asset_id,quantity])=>({asset_id,quantity})),
        ...(config.settlement_progression?.instance_capacity!==undefined||config.settlement_progression?.warehouse_slots
          ?{instance_count:this.instances.length}:{}),
        ...(this.warehouseOtherSlots!==null?{warehouse_other_slots:this.warehouseOtherSlots}:{}),
        ...(this.location?{location:this.location}:{}),
      }};
    } else {
      const tx=request.body.transaction;
      const admission=tx.kind==='admission', compensation=tx.kind==='compensation';
      assert.ok(['admission','compensation','settlement','result_settlement'].includes(tx.kind));
      const progression=tx.summary?.progression;
      const expectsClock=config.params?.entry_years!==0&&Boolean(config.settlement_progression?.age_asset_id);
      if((admission||compensation)
        ? Boolean(tx.clock)!==expectsClock
        : Boolean(progression)!==Boolean(config.settlement_progression))return this.refuse(request);
      let revision=tx.expected_revision;
      if(compensation) {
        const original=this.requests.get(tx.original_operation_id);
        const proof=this.journal.get(tx.original_operation_id);
        assert.equal(original?.body.transaction?.kind,'admission');assert.equal(proof?.kind,'committed');
        assert.deepEqual(tx.credits,original.body.transaction.debits);revision=proof.revision;
        const clock=original.body.transaction.clock;
        if(clock){
          const reverse=row=>({...row,before:row.after,after:row.before});
          assert.deepEqual(tx.clock,{remaining:reverse(clock.remaining),elapsed:reverse(clock.elapsed)});
        }else assert.equal(tx.clock,undefined);
      }
      if(revision!==this.revision || this.revision===4294967295)return this.refuse(request);
      const rows=admission?tx.debits:tx.credits;
      assert.ok(rows.length<=65);
      assert.deepEqual(rows.map(r=>r.asset_id),[...new Set(rows.map(r=>r.asset_id))].sort(compareIds));
      const next={...this.balances};
      let newInstances=tx.summary?.instances??[];
      assert.ok(Array.isArray(newInstances)&&newInstances.length<=128);
      const instanceIds=new Set(this.instances.map(item=>item.instance_id));
      for(const item of newInstances) {
        const definition=Object.hasOwn(config.world_loot?.instances??{},item.item_id)
          ? config.world_loot.instances[item.item_id] : undefined;
        assert.ok(definition&&Number.isInteger(item.quality)&&item.quality===definition.quality,'INVALID_ITEM_INSTANCE');
        assert.ok(typeof item.instance_id==='string'&&/^[a-f0-9]{64}-[a-f0-9]{16}$/.test(item.instance_id));
        assert.ok(!instanceIds.has(item.instance_id),'DUPLICATE_ITEM_INSTANCE');
        instanceIds.add(item.instance_id);
      }
      for(const row of rows) {
        assert.ok(Number.isInteger(row.quantity)&&row.quantity>0&&row.quantity<=4294967295);
        assign(next,row.asset_id,balance(next,row.asset_id)+(admission?-1:1)*row.quantity);
      }
      if(tx.clock){
        const {remaining,elapsed}=tx.clock;
        if(balance(this.balances,remaining.asset_id)!==remaining.before||balance(next,remaining.asset_id)!==remaining.after||
          !applyFrozenChanges(next,[elapsed]))return this.refuse(request);
      }
      const rewards=tx.summary?.rewards??[];
      if(Object.values(next).some(q=>!quantity(q))
        || rewards.some(row=>balance(this.balances,row.asset_id)!==row.balance_before
          || balance(next,row.asset_id)!==row.balance_after))return this.refuse(request);
      for(const row of rewards){
        if(row.adjustment&&!applyFrozenChanges(next,row.adjustment.changes))return this.refuse(request);
      }
      if(progression?.reward_earnings?.rates.some(row=>
        !Object.hasOwn(this.balances,row.asset_id)||this.balances[row.asset_id]!==row.bps))
        return this.refuse(request);
      const storage=progression?.storage??[];
      if(storage.some(row=>balance(this.balances,row.asset_id)!==row.before||
        balance(next,row.asset_id)!==row.after||row.after-row.before!==row.stored||
        !quantity(row.overflow)||
        row.stored+row.overflow!==tx.summary.inventory.find(item=>item.item_id===row.asset_id)?.kept))return this.refuse(request);
      const instances=progression?.instance_storage;
      if(instances){
        if(instances.before!==this.instances.length || instances.after!==instances.before+instances.stored_ids.length)
          return this.refuse(request);
        assert.deepEqual(newInstances.map(i=>i.instance_id),[...instances.stored_ids,...instances.overflow_ids]);
        newInstances=newInstances.slice(0,instances.stored_ids.length);
      }
      if(!admission&&!compensation){
        const layout=config.settlement_progression?.warehouse_slots, slots=progression?.warehouse_slots;
        if(Boolean(layout)!==Boolean(slots))return this.refuse(request);
        if(layout&&(!instances||!quantity(this.warehouseOtherSlots)||
          !quantity(slots.before)||!quantity(slots.after)||
          occupiedSlots(layout,this.balances,this.instances.length,this.warehouseOtherSlots)!==slots.before||
          occupiedSlots(layout,next,instances.after,this.warehouseOtherSlots)!==slots.after||
          slots.after>Math.max(layout.capacity,slots.before)))return this.refuse(request);
      }
      if(progression?.conversion && !applyFrozenChanges(next,progression.conversion.changes))return this.refuse(request);
      if(!admission&&!compensation){
        const ordered=config.settlement_progression?.conversion_mode==='ordered', steps=progression?.conversion_steps;
        if(ordered){
          if(!Array.isArray(steps)||progression.conversion||
            steps.some(step=>!applyFrozenChanges(next,step.changes)))return this.refuse(request);
        }else if(steps!==undefined)return this.refuse(request);
        const rule=config.settlement_progression?.threshold_progress, stage=progression?.threshold_progress;
        if(Boolean(rule)!==Boolean(stage))return this.refuse(request);
        if(rule&&(this.balances[rule.stage_asset_id]!==stage.stage_index||
          balance(next,rule.total_asset_id)!==stage.total||stage.progress?.asset_id!==rule.progress_asset_id||
          !applyFrozenChanges(next,[stage.progress])))return this.refuse(request);
      }
      const region=progression?.region;
      if(region&&(this.location?.region_id!==region.before.region_id||this.location?.revision!==region.before.revision||
        region.after.revision!==region.before.revision+1))return this.refuse(request);
      this.balances=next;this.revision++;
      if(region){
        this.location=structuredClone(region.after);
        this.savedPosition=null;
      }
      if(newInstances.length) this.instances.push(...structuredClone(newInstances));
      if(admission)this.admissions++;else if(compensation)this.compensations++;else this.settlements++;
      // Display projections of committed balances. No reward or loss calculation.
      this.warehouse=Object.hasOwn(next,config.item_id)?next[config.item_id]:0;
      this.cultivation=Object.hasOwn(next,config.asset_contract.reward_asset_id)?next[config.asset_contract.reward_asset_id]:0;
      this.items=Object.fromEntries(Object.entries(next).filter(([id])=>id!==config.item_id));
      if(config.settlement_progression){
        const fee=rows.find(row=>row.asset_id===config.asset_contract.entry_cost_asset_id)?.quantity??0;
        this.years+=admission?fee:compensation?-fee:0;
      }else this.years=(account.assets[config.asset_contract.entry_cost_asset_id]??0)-(next[config.asset_contract.entry_cost_asset_id]??0);
      result={kind:'committed',revision:this.revision,commit_id:`tx-${this.revision}`};
    }
    this.journal.set(request.operation_id,result);return result;
  }
}
