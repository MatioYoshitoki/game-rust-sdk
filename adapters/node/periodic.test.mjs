import test from 'node:test';import assert from 'node:assert/strict';import {newPeriodicStats,observePeriodic} from '../../web/periodic.mjs';
const ref=(kind,id)=>({kind,id,generation:1});
test('periodic metrics count authoritative amounts per recipient without reconstructing gameplay',()=>{const s=newPeriodicStats();observePeriodic(s,{kind:'private',event:{kind:'periodic_tick',source:ref('participant','A'),target:ref('monster','n'),hp_damage:7}},'A');observePeriodic(s,{kind:'private',event:{kind:'periodic_heal_tick',source:ref('participant','A'),target:ref('participant','A'),hp_restored:3}},'A');assert.equal(s.outgoingDotTicks,1);assert.equal(s.selfHotTicks,1);assert.equal(s.hpDamage,7);assert.equal(s.hpRestored,3);});
test('periodic metrics refuse foreign member details while allowing NPC self HOT',()=>{const s=newPeriodicStats();assert.throws(()=>observePeriodic(s,{kind:'periodic_tick',source:ref('participant','B'),target:ref('monster','n'),hp_damage:7},'A'),/WRONG_PERIODIC_RECIPIENT/);observePeriodic(s,{kind:'periodic_heal_tick',source:ref('monster','n'),target:ref('monster','n'),hp_restored:0},'A');assert.equal(s.dotTicks,0);assert.equal(s.npcHotTicks,1);});

test('ally HOT metrics retain outgoing and incoming roles without counting them as self HOT', () => {
  const cast = newPeriodicStats(), target = newPeriodicStats();
  const event = {kind:'private',event:{kind:'periodic_heal_tick',
    source:ref('participant','A'),target:ref('participant','B'),hp_restored:30}};
  observePeriodic(cast,event,'A');
  observePeriodic(target,event,'B');
  assert.equal(cast.outgoingAllyHotTicks,1);
  assert.equal(target.incomingAllyHotTicks,1);
  assert.equal(cast.selfHotTicks + target.selfHotTicks,0);
  assert.throws(() => observePeriodic(newPeriodicStats(),event,'C'),/WRONG_PERIODIC_RECIPIENT/);
});
