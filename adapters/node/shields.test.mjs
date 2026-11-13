import test from 'node:test';import assert from 'node:assert/strict';import {newShieldStats,observeShields} from '../../web/shields.mjs';
const target={kind:'participant',id:'A',generation:1};
test('shield observations keep visible consumption separate from authoritative DOT total',()=>{const s=newShieldStats();observeShields(s,{kind:'shield_absorbed',target,amount:3,consumptions:[{instance_id:'own',absorbed:3,remaining:4}]},'A');observeShields(s,{kind:'periodic_tick',absorbed:9},'A');assert.equal(s.visibleAbsorbed,3);assert.equal(s.dotAbsorbed,9);assert.equal(s.ownAbsorptions,1);});
test('shield observations refuse foreign target and inconsistent consumption slice',()=>{assert.throws(()=>observeShields(newShieldStats(),{kind:'shield_absorbed',target,amount:1,consumptions:[]},'B'),/WRONG_SHIELD_RECIPIENT/);assert.throws(()=>observeShields(newShieldStats(),{kind:'shield_absorbed',target,amount:1,consumptions:[]},'A'),/INVALID_SHIELD_SLICE/);});

test('ally shield grants reach their caster and target while absorption stays with the target', () => {
  const source = {...target, id: 'B'};
  const grant = {kind: 'shield_granted', source, target, granted: 20};
  const outgoing = newShieldStats(), incoming = newShieldStats();
  observeShields(outgoing, grant, 'B');
  observeShields(incoming, grant, 'A');
  assert.equal(outgoing.outgoingAllyGrants, 1);
  assert.equal(outgoing.ownGrants, 0);
  assert.equal(incoming.ownGrants, 1);
  assert.throws(() => observeShields(newShieldStats(), grant, 'C'), /WRONG_SHIELD_RECIPIENT/);
  const hit = {kind: 'shield_absorbed', source: 'npc', target, amount: 5,
    consumptions: [{instance_id: 'shield', absorbed: 5, remaining: 15}]};
  assert.throws(() => observeShields(outgoing, hit, 'B'), /WRONG_SHIELD_RECIPIENT/);
  observeShields(incoming, hit, 'A');
  assert.equal(incoming.visibleAbsorbed, 5);
});
