import test from 'node:test';import assert from 'node:assert/strict';import{newSkillStats,observeSkills,skillAction}from'../../web/skill-slots.mjs';
const mechanisms=['projectile','self_status','piercing_projectile','unit_projectile','ground_circle','ground_field','piercing_projectile','self_status'];
const skills=mechanisms.map((mechanism,index)=>({mechanism,id:`test-slot-${index}`}));
const snapshot={self:{position:[400,400]},monsters:[{entity:{kind:'monster',id:'actual-npc',generation:7},hp:100,position:[460,400]}]};
test('configured slot input helper covers all registered forms with intentions only',()=>{const actions=skills.map(s=>skillAction(s,snapshot,[460,400]));assert.deepEqual(actions.map(a=>a.kind),['cast','cast_self','cast','cast_unit','cast_ground','cast_ground','cast','cast_self']);assert.deepEqual(actions[3].target,{id:'actual-npc',generation:7});assert.deepEqual(actions[0].direction,[60,0]);for(const a of actions)for(const k of ['damage','cost','cooldown_ms','radius','max_hits'])assert.ok(!Object.hasOwn(a,k));});
test('configured slot input helper uses actual target identities and refuses missing targets or unknown forms',()=>{assert.throws(()=>skillAction(skills[3],{...snapshot,monsters:[]}),e=>e.code==='INVALID_TARGET');assert.throws(()=>skillAction({...skills[0],mechanism:'patch_world'},snapshot,[460,400]),e=>e.code==='UNSUPPORTED');assert.deepEqual(skillAction(skills[1],{...snapshot,monsters:[]}),{kind:'cast_self',skill_id:skills[1].id});});
test('slot observation counts only private actual skill commits and ignores public plans and basic attacks',()=>{const s=newSkillStats();observeSkills(s,{kind:'projectile_started',projectile:{}});observeSkills(s,{kind:'private',event:{kind:'action_committed',skill_id:null}});assert.equal(s.total,0);observeSkills(s,{kind:'private',event:{kind:'action_committed',skill_id:skills[7].id}});assert.equal(s.commits[skills[7].id],1);assert.equal(s.total,1);});
test('slot observation keeps independent counters for all configured identities',()=>{const s=newSkillStats();for(let i=0;i<100;i++)for(const skill of skills)observeSkills(s,{kind:'private',event:{kind:'action_committed',skill_id:skill.id}});assert.equal(Object.keys(s.commits).length,8);assert.ok(Object.values(s.commits).every(n=>n===100));assert.equal(s.total,800);});
test('strict unit effect emits only target identity and refuses absent targets',()=>{const skill={id:'strict-unit',mechanism:'unit_effect'};assert.deepEqual(skillAction(skill,snapshot),{kind:'cast_unit',skill_id:'strict-unit',target:{id:'actual-npc',generation:7}});assert.throws(()=>skillAction(skill,{...snapshot,monsters:[]}),e=>e.code==='INVALID_TARGET');});
test('hostile unit targeting skips friendly summons on the same board',()=>{
  const ally={entity:{kind:'monster',id:'ally',generation:1},hp:10,position:[400,400],summon:{owner:{kind:'participant',id:'A',generation:1},expires_at_ms:300}};
  assert.deepEqual(skillAction(skills[3],{...snapshot,monsters:[ally,...snapshot.monsters]}).target,{id:'actual-npc',generation:7});
  assert.throws(()=>skillAction(skills[3],{...snapshot,monsters:[ally]}),e=>e.code==='INVALID_TARGET');
});
test('member conflict allows an aimed member while implicit targeting still picks an NPC',()=>{
  const a={kind:'participant',id:'A',generation:1},b={kind:'participant',id:'B',generation:2};
  const view={...snapshot,member_conflict:true,self:{...snapshot.self,participant:{entity:a}},
    members:[{entity:b,position:[420,400],hp:1,phase:'active',protected:false}]};
  assert.deepEqual(skillAction(skills[3],view,[420,400]).target,{id:'B',generation:2});
  assert.deepEqual(skillAction(skills[3],view).target,{id:'actual-npc',generation:7});
});
test('training unit intent skips another seat target even when it is closer',()=>{
  const self={...snapshot.self,participant:{entity:{kind:'participant',id:'A',generation:1}}};
  const own={...snapshot.monsters[0],training_owner_id:'A'};
  const foreign={...own,position:[410,400],entity:{...own.entity,id:'foreign'},training_owner_id:'B'};
  assert.deepEqual(skillAction(skills[3],{self,monsters:[foreign,own]},[410,400]).target,
    {id:'actual-npc',generation:7});
});
