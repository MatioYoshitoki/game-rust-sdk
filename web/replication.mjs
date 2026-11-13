// Pure, atomic display decoder shared by browsers and the ordinary Node consumer.
const fail=code=>{throw Object.assign(new Error(code),{code})};
const clone=v=>structuredClone(v),object=v=>v!==null&&typeof v==='object'&&!Array.isArray(v);
const canonical=v=>Array.isArray(v)?v.map(canonical):object(v)?Object.fromEntries(Object.keys(v).sort().map(k=>[k,canonical(v[k])])):v;
const same=(a,b)=>JSON.stringify(canonical(a))===JSON.stringify(canonical(b));
const uint=(v,max=Number.MAX_SAFE_INTEGER,min=0)=>Number.isSafeInteger(v)&&v>=min&&v<=max;
const id=v=>typeof v==='string'&&new TextEncoder().encode(v).length>=1&&new TextEncoder().encode(v).length<=128&&!/[\u0000-\u001f\u007f-\u009f]/.test(v);
function shape(v,required,optional=[]){if(!object(v)||required.some(k=>!Object.hasOwn(v,k))||Object.keys(v).some(k=>![...required,...optional].includes(k)))fail('INVALID_FRAME');}
function ref(v){shape(v,['kind','id','generation']);if(!['participant','monster'].includes(v.kind)||!id(v.id)||!uint(v.generation,4294967295,1))fail('INVALID_FRAME');return JSON.stringify([v.kind,v.id,v.generation]);}
function itemInstance(v) {
  shape(v, ['instance_id', 'item_id', 'quality']);
  if (!id(v.instance_id) || !id(v.item_id) || !uint(v.quality, 65535)) fail('INVALID_FRAME');
}
function validateRelics(snapshot) {
  if (snapshot.relics === undefined) return;
  if (!Array.isArray(snapshot.relics) || snapshot.relics.length > 8) fail('INVALID_FRAME');
  const ids = [], deceased = [];
  for (const row of snapshot.relics) {
    shape(row, ['id', 'position', 'deceased', 'revision', 'remaining_stacks', 'highest_quality']);
    if (!id(row.id) || !point(row.position) || !uint(row.revision, 4294967295, 1) ||
        !uint(row.remaining_stacks, 128) ||
        (row.remaining_stacks === 0 ? row.highest_quality !== null : !uint(row.highest_quality, 65535)) ||
        !snapshot.members.some(member => same(member.entity, row.deceased) &&
          ['settling', 'terminal'].includes(member.phase))) fail('INVALID_FRAME');
    ids.push(row.id);
    deceased.push(ref(row.deceased));
  }
  unique(ids);
  unique(deceased);
}
function validateBoons(own) {
  const view=own.boons;
  if(view===undefined)return;
  const shortId=value=>id(value)&&new TextEncoder().encode(value).length<=64;
  shape(view,['capacity','revision','slots','opportunities'],['pending_claim']);
  if(!uint(view.capacity,3,1)||!uint(view.revision,4294967295,1)||
    !Array.isArray(view.slots)||view.slots.length>view.capacity||
    !Array.isArray(view.opportunities)||view.opportunities.length>32||
    (own.phase!=='active'&&(view.slots.length||view.opportunities.length)))fail('INVALID_FRAME');
  if(view.pending_claim!==undefined){
    const claim=view.pending_claim;
    shape(claim,['opportunity_id','boon_id','completes_at_ms']);
    if(own.phase!=='active'||!own.ready||own.protected||!shortId(claim.opportunity_id)||!shortId(claim.boon_id)||
      !uint(claim.completes_at_ms,Number.MAX_SAFE_INTEGER,1))fail('INVALID_FRAME');
  }
  let previous=0;
  for(const slot of view.slots){
    shape(slot,['slot','boon_id','level','max_level']);
    if(!uint(slot.slot,view.capacity,previous+1)||!shortId(slot.boon_id)||
      !uint(slot.max_level,3,1)||!uint(slot.level,slot.max_level,1))fail('INVALID_FRAME');
    previous=slot.slot;
  }
  unique(view.slots.map(slot=>slot.boon_id));
  for(const source of view.opportunities){
    shape(source,['id','position','offer','available'],['shared_remaining']);
    if(!shortId(source.id)||!point(source.position)||typeof source.available!=='boolean')fail('INVALID_FRAME');
    if(source.shared_remaining!==undefined&&(!uint(source.shared_remaining,8)||source.shared_remaining===0&&source.available))fail('INVALID_FRAME');
    if(source.offer?.kind==='grant'){
      shape(source.offer,['kind','choices']);
      const choices=source.offer.choices;
      if(!Array.isArray(choices)||!uint(choices.length,8,1)||!choices.every(shortId))fail('INVALID_FRAME');
      unique(choices);
    }else{
      shape(source.offer,['kind']);
      if(source.offer.kind!=='upgrade')fail('INVALID_FRAME');
    }
  }
  unique(view.opportunities.map(source=>source.id));
}
function validateItemInstances(snapshot) {
  const own = snapshot.self.instances;
  if (own !== undefined && (!Array.isArray(own) || own.length > 128)) fail('INVALID_FRAME');
  const instances = [...(own ?? [])];
  for (const resource of snapshot.resources) {
    if (resource.instance === undefined) continue;
    itemInstance(resource.instance);
    if (resource.quantity !== 1 || resource.instance.item_id !== resource.item_id) fail('INVALID_FRAME');
    instances.push(resource.instance);
  }
  for (const instance of own ?? []) itemInstance(instance);
  unique(instances.map(instance => instance.instance_id));
}
const point=v=>Array.isArray(v)&&v.length===2&&v.every(Number.isFinite);
const phases=['preparing','deploying','active','draining','closed','faulted'];
function state(v){shape(v,['phase','active_at_ms','extraction_point','extraction_open'],['encounter_progress','extraction_pressure','contested_equipment']);if(!phases.includes(v.phase)||(v.active_at_ms!==null&&!uint(v.active_at_ms))||!point(v.extraction_point)||typeof v.extraction_open!=='boolean')fail('INVALID_FRAME');}
const ownKeys=['participant','position','hp','max_hp','spirit','max_spirit','phase','protected','foreground','ready','effective_atk','effective_def','statuses','inventory','inventory_revision','player_skills','pending_action','basic_ready_at_ms','recovery_until_ms','extraction_finishes_ms'];
function validateCharge(pending, logicalMs) {
  if (!Object.hasOwn(pending, 'charge')) return;
  const charge = pending.charge;
  shape(charge, ['action_id', 'started_at_ms', 'min_release_at_ms', 'auto_commit_at_ms'], ['aim','released_at_ms','release_windup_ms']);
  const { started_at_ms: start, min_release_at_ms: earliest, auto_commit_at_ms: automatic } = charge;
  const windup=charge.release_windup_ms??0,latest=automatic-windup;
  if (pending.skill_id === null || Object.hasOwn(pending, 'unit_target') ||
      Object.hasOwn(pending, 'ground_position') || !id(charge.action_id) ||
      !uint(start) || !uint(earliest) || !uint(automatic) || start > logicalMs ||
      start >= earliest || earliest >= latest || automatic - start > 5000 ||
      (Object.hasOwn(charge,'release_windup_ms')&&!uint(charge.release_windup_ms,5000,1)) ||
      pending.commits_at_ms < earliest+windup || pending.commits_at_ms > automatic ||
      (pending.commits_at_ms < automatic && pending.commits_at_ms > logicalMs && !Object.hasOwn(charge,'released_at_ms')) ||
      (Object.hasOwn(charge,'released_at_ms') && (!uint(charge.released_at_ms) ||
        charge.released_at_ms < start || charge.released_at_ms > logicalMs ||
        charge.released_at_ms > latest || pending.commits_at_ms !== Math.max(earliest,charge.released_at_ms)+windup))) {
    fail('INVALID_FRAME');
  }
  if (Object.hasOwn(charge, 'aim')) {
    const aim = charge.aim;
    shape(aim, ['direction', 'updated_at_ms']);
    if (!point(aim.direction) || Math.abs(Math.hypot(...aim.direction) - 1) > 1e-9 ||
        !uint(aim.updated_at_ms) || aim.updated_at_ms < start ||
        aim.updated_at_ms > Math.min(charge.released_at_ms ?? latest, logicalMs) ||
        aim.updated_at_ms >= automatic) {
      fail('INVALID_FRAME');
    }
  }
}
function skills(v,logicalMs){if(v.skill_powers!==undefined){if(!object(v.skill_powers)||Object.keys(v.skill_powers).length!==v.player_skills.length||Object.entries(v.skill_powers).some(([k,n])=>!v.player_skills.some(s=>s.skill_id===k)||!uint(n,4294967295)))fail('INVALID_FRAME');}if(v.player_skills.length>8)fail('INVALID_FRAME');const ids=[];for(const s of v.player_skills){shape(s,['skill_id','ready_at_ms']);if(!id(s.skill_id)||!uint(s.ready_at_ms))fail('INVALID_FRAME');ids.push(s.skill_id);}unique(ids);if(v.pending_action!==null){const p=v.pending_action;shape(p,['skill_id','commits_at_ms'],['action_id','resource_reservation','charge','ground_position','unit_target','ally_target','ally_target_is_self']);if(!uint(p.commits_at_ms)||(p.skill_id!==null&&!ids.includes(p.skill_id)))fail('INVALID_FRAME');
if(Object.hasOwn(p,'action_id')&&!id(p.action_id))fail('INVALID_FRAME');
if(Object.hasOwn(p,'resource_reservation')){const r=p.resource_reservation;shape(r,['spirit','hp','minimum_hp_after_cost']);if(!p.action_id||p.skill_id===null||!uint(r.spirit,4294967295)||!uint(r.hp,4294967295)||!uint(r.minimum_hp_after_cost,4294967295,1))fail('INVALID_FRAME');}
if(p.charge&&p.action_id!==undefined&&p.charge.action_id!==p.action_id)fail('INVALID_FRAME');validateCharge(p,logicalMs);}}
function unit(v){shape(v,['id','generation']);if(!id(v.id)||!uint(v.generation,4294967295,1))fail('INVALID_FRAME');}
function own(v,pid){shape(v,ownKeys,['equipment','zone_exposure','world_weapon_profile','basic_charge','basic_charge_ready_at_ms','basic_profile','basic_wave_phase','basic_projectile_progress','skill_powers','skill_loadout_revision','shield_total','active_channel','projectile_recast','followup','recovery_cancel','basic_followup','basic_pursuit','item_cooldowns','checkpoint_progress','instances','boons','mode','training']);validateBoons(v);if(v.basic_profile!==undefined&&(!id(v.basic_profile)||new TextEncoder().encode(v.basic_profile).length>64))fail('INVALID_FRAME');if(v.basic_wave_phase!==undefined&&(!['primary','alternate'].includes(v.basic_wave_phase)||!['deploying','active'].includes(v.phase)))fail('INVALID_FRAME');if(v.basic_projectile_progress!==undefined)basicProjectileProgress(v.basic_projectile_progress,v.phase);if(v.skill_loadout_revision!==undefined&&!uint(v.skill_loadout_revision,4294967295))fail('INVALID_FRAME');shape(v.participant,['entity','owner_id']);ref(v.participant.entity);if(v.participant.entity.kind!=='participant'||v.participant.entity.id!==pid||v.participant.owner_id!==pid||!point(v.position)||!['deploying','active','settling','terminal'].includes(v.phase))fail('INVALID_FRAME');for(const n of ['hp','max_hp','spirit','max_spirit','effective_atk','effective_def','inventory_revision'])if(!uint(v[n],4294967295))fail('INVALID_FRAME');for(const n of ['basic_ready_at_ms','recovery_until_ms'])if(!uint(v[n]))fail('INVALID_FRAME');for(const n of ['statuses','inventory','player_skills'])if(!Array.isArray(v[n]))fail('INVALID_FRAME');for(const n of ['protected','foreground','ready'])if(typeof v[n]!=='boolean')fail('INVALID_FRAME');if(v.extraction_finishes_ms!==null&&!uint(v.extraction_finishes_ms))fail('INVALID_FRAME');}
function itemKey(kind,v){if(kind==='members'||kind==='monsters'){if(!object(v))fail('INVALID_FRAME');return ref(v.entity);}if(!object(v)||!id(v.id))fail('INVALID_FRAME');return v.id;}
function key(kind,v){return kind==='members'||kind==='monsters'?ref(v):id(v)?v:fail('INVALID_FRAME');}
function unique(values){if(new Set(values).size!==values.length)fail('INVALID_FRAME');}
function listCheck(kind,values){if(!Array.isArray(values))fail('INVALID_FRAME');const keys=values.map(v=>itemKey(kind,v));for(const v of values){
 if(kind==='members'){shape(v,['entity','position','hp','max_hp','phase','protected'],['basic_seals','extraction_channel']);if(v.basic_seals!==undefined&&(!['deploying','active'].includes(v.phase)||!Array.isArray(v.basic_seals)||v.basic_seals.length>8||!v.basic_seals.every(point)))fail('INVALID_FRAME');if(v.entity.kind!=='participant'||!point(v.position)||!uint(v.hp,4294967295)||!uint(v.max_hp,4294967295)||!['deploying','active','settling','terminal'].includes(v.phase)||typeof v.protected!=='boolean')fail('INVALID_FRAME');}
 if(kind==='monsters'){shape(v,['entity','template_id','position','hp','max_hp','radius','returning','target'],['statuses','shield_total','summon','boss_phase','training_owner_id']);if(v.entity.kind!=='monster'||!id(v.template_id)||!point(v.position)||!uint(v.hp,4294967295)||!uint(v.max_hp,4294967295)||!Number.isFinite(v.radius)||v.radius<0||typeof v.returning!=='boolean')fail('INVALID_FRAME');if(v.target!==null)ref(v.target);bossPhase(v);}
 if(kind==='resources'){shape(v,['id','position','quantity','item_id','owner','expires_at_ms'],['instance']);if(!point(v.position)||!uint(v.quantity,4294967295)||!uint(v.expires_at_ms)||(v.item_id!==null&&!id(v.item_id)))fail('INVALID_FRAME');if(v.owner!==null)ref(v.owner);}
 if(kind==='projectiles'){shape(v,['id','source','position','radius'],['unit_target','piercing','basic_segment']);if(v.unit_target!==undefined)unit(v.unit_target);ref(v.source);if(!point(v.position)||!Number.isFinite(v.radius)||v.radius<0)fail('INVALID_FRAME');}
 }unique(keys);if(kind==='members'||kind==='monsters')unique(values.map(v=>JSON.stringify([v.entity.kind,v.entity.id])));return keys;}
function bossPhase(monster){
 const phase=monster.boss_phase;
 if(phase===undefined)return;
 shape(phase,['phase_id','phase_name','phase_index','phase_count','epoch','attack_interval_ms','cast_chance']);
 if(monster.summon!==undefined||phase.epoch!==monster.entity.generation||!id(phase.phase_id)||!id(phase.phase_name)
   ||!uint(phase.phase_count,8,1)||!uint(phase.phase_index,phase.phase_count,1)
   ||!uint(phase.attack_interval_ms,3600000,1)||!Number.isFinite(phase.cast_chance)||phase.cast_chance<0||phase.cast_chance>1)fail('INVALID_FRAME');
}
function bossPhaseTransition(before,after){
 for(const monster of before.monsters){
  const next=after.monsters.find(candidate=>same(candidate.entity,monster.entity));
  if(!next)continue;
  const a=monster.boss_phase,b=next.boss_phase;
  if(a===undefined&&b===undefined)continue;
  if(!a||!b||a.phase_count!==b.phase_count||b.phase_index<a.phase_index||b.phase_index===a.phase_index&&!same(a,b))fail('INVALID_FRAME');
 }
}
function reservationTransition(before,after){
 if(!same(before.participant.entity,after.participant.entity))return;
 const a=before.pending_action,b=after.pending_action;if(!a||!b)return;
 if(a.action_id!==undefined&&b.action_id===undefined&&a.skill_id===b.skill_id)fail('INVALID_FRAME');
 if(a.action_id!==undefined&&a.action_id===b.action_id&&(!same(a.skill_id,b.skill_id)||!same(a.resource_reservation,b.resource_reservation)))fail('INVALID_FRAME');
}
function channelTransition(before,after){
 if(!same(before.participant.entity,after.participant.entity))return;
 const a=before.active_channel,b=after.active_channel;
 if(!a||!b||a.action_id!==b.action_id||!a.target&&!b.target)return;
 if(!['target','skill_id','started_at_ms','ends_at_ms','total_pulses','recovery','direction'].every(k=>same(a[k],b[k]))||b.pulses<a.pulses)fail('INVALID_FRAME');
}
function loadoutTransition(before,after){
 if(!same(before.participant.entity,after.participant.entity))return;
 const a=before.skill_loadout_revision,b=after.skill_loadout_revision;
 if(a===undefined&&b===undefined)return;
 if(b===undefined||b<(a??0)||b===(a??0)&&!same(before.player_skills.map(s=>s.skill_id),after.player_skills.map(s=>s.skill_id)))fail('INVALID_FRAME');
}
function validateEquipment(own) {
 const equipment=own.equipment;
 if(equipment===undefined)return;
 shape(equipment,['revision'],['weapon']);
 const weapon=equipment.weapon;
 if(!uint(equipment.revision,4294967295)||(weapon!==undefined)!==(own.basic_profile!==undefined))fail('INVALID_FRAME');
 if(weapon===undefined){
   if(equipment.revision===0)fail('INVALID_FRAME');
   return;
 }
 shape(weapon,['instance_id','model_id'],['morph_id']);
 for(const value of Object.values(weapon)){
   if(!id(value)||new TextEncoder().encode(value).length>64)fail('INVALID_FRAME');
 }
}
function equipmentTransition(before,after) {
 const a=before.equipment,b=after.equipment;
 if(b===undefined){
   if(a!==undefined||before.basic_profile!==after.basic_profile)fail('INVALID_FRAME');
 }else if(b.revision<=(a?.revision??0)&&(!same(a,b)||before.basic_profile!==after.basic_profile)){
   fail('INVALID_FRAME');
 }
}
function validatePeriodic(p, s) {
  shape(p,['kind','interval_ms','next_tick_at_ms','ticks_done','total_ticks'],['raw_damage','raw_heal','raw_spirit','power_sampling','per_stack','first_tick_at_ms','tick_on_expiry']);
  if(!uint(p.interval_ms,Number.MAX_SAFE_INTEGER,1)||!uint(p.ticks_done,128)||!uint(p.total_ticks,128)||
      p.ticks_done>p.total_ticks||(p.next_tick_at_ms!==null&&!uint(p.next_tick_at_ms)))fail('INVALID_FRAME');
  if(p.tick_on_expiry!==undefined&&(p.tick_on_expiry!==true||p.kind!=='max_hp_heal'||p.per_stack!==undefined||
    p.power_sampling!==undefined||p.raw_spirit!==undefined))fail('INVALID_FRAME');
  if(s){
    if(!uint(s.applied_at_ms)||!uint(s.expires_at_ms)||s.expires_at_ms<=s.applied_at_ms||s.attribute!==undefined||s.shield!==undefined||s.modifier_bps!==0)fail('INVALID_FRAME');
    const layered=p.per_stack!==undefined||p.first_tick_at_ms!==undefined;
    if(layered?(p.per_stack!==true||!uint(p.first_tick_at_ms)||p.first_tick_at_ms<s.applied_at_ms||p.first_tick_at_ms>s.applied_at_ms+p.interval_ms||!uint(s.max_stacks,32,1)||!uint(s.stacks,s.max_stacks,1)):(s.stacks!==undefined||s.max_stacks!==undefined))fail('INVALID_FRAME');
    const first=layered?p.first_tick_at_ms:s.applied_at_ms+p.interval_ms;
    if(p.tick_on_expiry&&(s.expires_at_ms-s.applied_at_ms)%p.interval_ms!==0)fail('INVALID_FRAME');
    const last=s.expires_at_ms-(p.tick_on_expiry?0:1);
    const total=first<=last?Math.floor((last-first)/p.interval_ms)+1:0;
    if(!uint(first)||p.total_ticks!==total||p.next_tick_at_ms!==(p.ticks_done<total?first+p.ticks_done*p.interval_ms:null))fail('INVALID_FRAME');
  }
  if(p.power_sampling!==undefined&&!['source_snapshot','target_snapshot','source_live','target_live'].includes(p.power_sampling))fail('INVALID_FRAME');
  if(['physical_damage','elemental_damage','true_damage'].includes(p.kind)){
    if(!uint(p.raw_damage,4294967295)||Object.hasOwn(p,'raw_heal')||Object.hasOwn(p,'raw_spirit'))fail('INVALID_FRAME');
  }else if(p.kind==='max_hp_heal'){
    if(!uint(p.raw_heal,4294967295)||Object.hasOwn(p,'raw_damage')||p.raw_spirit!==undefined&&!uint(p.raw_spirit,4294967295))fail('INVALID_FRAME');
  }else fail('INVALID_FRAME');
}
function projectedStatus(status, unit, snapshot, opponent=false) {
  shape(status,['instance_id','definition_id','source','target','applied_at_ms','expires_at_ms','modifier_bps'],['periodic','shield','attribute','stacks','max_stacks','modifiers','base_modifiers','stack_counter','window','flags','cleanse']);
  const viewer = snapshot.self.participant.entity;
  const visible = opponent || same(status.source, viewer) || same(status.source, unit.entity) ||
    unit.summon && same(unit.summon.owner, viewer) &&
      snapshot.monsters.some(source => source.summon === undefined && same(status.source, source.entity));
  if (!id(status.instance_id) || !id(status.definition_id) || !same(status.target, unit.entity) ||
      !visible || !uint(status.applied_at_ms) || !uint(status.expires_at_ms) ||
      status.expires_at_ms < status.applied_at_ms || !Number.isInteger(status.modifier_bps)) fail('INVALID_FRAME');
  if (status.periodic !== undefined) validatePeriodic(status.periodic,status);
}
function validateSummons(snapshot) {
  for (const unit of snapshot.monsters) {
    if (unit.training_owner_id!==undefined&&(unit.summon!==undefined||
        !snapshot.members.some(member=>member.entity.id===unit.training_owner_id))) fail('INVALID_FRAME');
    if (unit.target?.kind === 'monster' &&
        !snapshot.monsters.some(target => target.summon !== undefined && same(target.entity,unit.target))) fail('INVALID_FRAME');
    if (unit.summon === undefined) continue;
    shape(unit.summon,['owner','expires_at_ms']);
    const {owner, expires_at_ms: expires} = unit.summon;
    ref(owner);
    if (owner.kind !== 'participant' || !snapshot.members.some(member =>
        member.hp > 0 && member.phase === 'active' && same(member.entity,owner)) ||
        unit.hp === 0 || unit.hp > unit.max_hp || unit.returning || unit.target !== null ||
        !uint(expires) || expires <= snapshot.logical_ms || expires - snapshot.logical_ms > 60000) fail('INVALID_FRAME');
  }
}
function isAllySupportStatus(status, members) {
  const hot = status.periodic?.kind === 'max_hp_heal';
  const shield = status.shield !== undefined;
  if (!members.some(member => same(member.entity, status.source))) return false;
  if (!hot && !shield && status.periodic === undefined) {
    shape(status,['instance_id','definition_id','source','target','applied_at_ms','expires_at_ms','modifier_bps'],['attribute','modifiers','base_modifiers','stack_counter','stacks','max_stacks','window','flags','cleanse']);
    const numeric=['max_hp','max_spirit','spd','attack_speed','basic_interval_delay','element_resistance','atk','def','healing_received','damage_reduction','recovery_strength','cooldown_reduction','armor_penetration','critical_chance','critical_damage_bonus','dodge_penalty','dodge_chance','accuracy','skill_power_bonus','life_steal','control_duration_reduction','harmful_duration_reduction','movement_slow','movement_haste','control_immunity','damage_immunity','first_control_immunity'];
    if (!uint(status.applied_at_ms)||!uint(status.expires_at_ms)||status.expires_at_ms<=status.applied_at_ms||!Number.isInteger(status.modifier_bps)||status.modifier_bps < -10000||status.modifier_bps>100000||
        (status.attribute===undefined ? status.modifiers===undefined&&status.base_modifiers===undefined&&status.flags===undefined||status.modifier_bps!==0 : !numeric.includes(status.attribute))||
        (status.stacks===undefined)!==(status.max_stacks===undefined)||status.stacks!==undefined&&(!uint(status.max_stacks,128,1)||!uint(status.stacks,status.max_stacks,status.stack_counter===true?0:1)))fail('INVALID_FRAME');
    statusModifiers(status);
    statusFlags(status);
    if(status.flags?.some(f=>['silence','root','stun'].includes(f)))fail('INVALID_FRAME');
    return true;
  }
  if (hot === shield) return false;
  shape(status, ['instance_id','definition_id','source','target','applied_at_ms','expires_at_ms',
    'modifier_bps', hot ? 'periodic' : 'shield'], ['modifiers','base_modifiers','stack_counter','window','flags','cleanse',...(hot?['stacks','max_stacks']:[])]);
  if (!uint(status.applied_at_ms) || !uint(status.expires_at_ms) ||
      status.expires_at_ms <= status.applied_at_ms || status.modifier_bps !== 0) fail('INVALID_FRAME');
  if (hot) validatePeriodic(status.periodic,status);
  return true;
}
function statusFlags(s){
 if(s.attribute==='life_steal'){
  shape(s,['instance_id','definition_id','source','target','applied_at_ms','expires_at_ms','modifier_bps','attribute'],['stacks','max_stacks','window','cleanse']);
  if(!uint(s.applied_at_ms)||!uint(s.expires_at_ms)||s.expires_at_ms<=s.applied_at_ms||!Number.isInteger(s.modifier_bps)||s.modifier_bps < -10000||s.modifier_bps>100000)fail('INVALID_FRAME');
 }

 if(s.attribute==='movement_slow'&&(!Number.isInteger(s.modifier_bps)||s.modifier_bps < -9999||s.modifier_bps >= 0||(s.stacks===undefined)!==(s.max_stacks===undefined)||s.stacks!==undefined&&(!uint(s.max_stacks,32,1)||!uint(s.stacks,s.max_stacks,1))||['modifiers','flags','periodic','shield'].some(k=>s[k]!==undefined)))fail('INVALID_FRAME');
 if(s.attribute==='movement_haste'&&(!uint(s.modifier_bps,10000,1)||['modifiers','flags','periodic','shield','stacks','max_stacks'].some(k=>s[k]!==undefined)))fail('INVALID_FRAME');
 if(s.attribute==='basic_interval_delay'&&(!uint(s.modifier_bps,100000,1)||['modifiers','flags','periodic','shield','stacks','max_stacks'].some(k=>s[k]!==undefined)))fail('INVALID_FRAME');
 if(['silence','root','stun','control_immunity','damage_immunity','first_control_immunity'].includes(s.attribute)&&
    (s.modifier_bps!==0||['modifiers','flags','window','periodic','shield','stacks','max_stacks'].some(k=>s[k]!==undefined)))fail('INVALID_FRAME');
 if(s.attribute==='spirit_recovery_lock'&&(s.modifier_bps!==-10000||['modifiers','flags','window','periodic','shield','stacks','max_stacks'].some(k=>s[k]!==undefined)))fail('INVALID_FRAME');
 if(s.flags===undefined)return;
 const controls=['silence','root','stun'], immunities=['control_immunity','damage_immunity','first_control_immunity'];
 if(!Array.isArray(s.flags)||s.flags.length<1||s.flags.length>3||s.flags.some(f=>![...controls,...immunities].includes(f))||
    s.flags.some(f=>controls.includes(f))&&s.flags.some(f=>immunities.includes(f))||s.attribute!==undefined||s.modifier_bps!==0||
    ['shield','stacks','max_stacks'].some(k=>s[k]!==undefined)||s.window!==undefined&&s.periodic?.kind!=='max_hp_heal'||s.periodic!==undefined&&s.periodic.kind!=='max_hp_heal'||s.flags.some(f=>controls.includes(f))&&s.periodic!==undefined)fail('INVALID_FRAME');
 unique(s.flags);
}
function statusModifiers(s){
 if(s.stack_counter!==undefined||s.base_modifiers!==undefined||s.modifiers!==undefined)shape(s,['instance_id','definition_id','source','target','applied_at_ms','expires_at_ms','modifier_bps'],['attribute','periodic','shield','modifiers','base_modifiers','stack_counter','stacks','max_stacks','window','flags','cleanse']);
 if(s.stack_counter!==undefined&&(s.stack_counter!==true||s.stacks===undefined||s.max_stacks===undefined||s.attribute!==undefined||s.periodic!==undefined||s.shield!==undefined||s.flags!==undefined||s.modifiers===undefined&&s.base_modifiers===undefined||[...(s.modifiers??[]),...(s.base_modifiers??[])].some(m=>m.attribute==='movement_slow')&&s.max_stacks>32))fail('INVALID_FRAME');
 if(s.base_modifiers!==undefined&&(s.attribute!==undefined||s.periodic!==undefined||s.shield!==undefined||s.modifier_bps!==0))fail('INVALID_FRAME');
 if((s.modifiers?.length??0)+(s.base_modifiers?.length??0)>8)fail('INVALID_FRAME');
 if(s.modifiers===undefined&&s.base_modifiers===undefined)return;
 const attributes=['movement_haste','movement_slow','max_hp','max_spirit','spd','attack_speed','basic_interval_delay','element_resistance','atk','def','healing_received','damage_reduction','recovery_strength','cooldown_reduction','armor_penetration','critical_chance','critical_damage_bonus','dodge_penalty','dodge_chance','accuracy','skill_power_bonus','life_steal','control_duration_reduction','harmful_duration_reduction'];
 for(const list of [s.modifiers,s.base_modifiers]) {
 if(list===undefined)continue;
 if(!Array.isArray(list)||list.length<1||list.length>8)fail('INVALID_FRAME');
 const ids=[];
 for(const m of list){shape(m,['attribute','modifier_bps']);if(!attributes.includes(m.attribute)||!Number.isInteger(m.modifier_bps)||m.modifier_bps< -10000||m.modifier_bps>100000||m.modifier_bps<0&&!['movement_slow','max_hp','max_spirit','spd','attack_speed','element_resistance','atk','def','healing_received','recovery_strength','life_steal'].includes(m.attribute))fail('INVALID_FRAME');if(m.attribute==='movement_slow'&&(m.modifier_bps>=0||m.modifier_bps < -9999))fail('INVALID_FRAME');if(m.attribute==='movement_haste'&&!uint(m.modifier_bps,10000,1))fail('INVALID_FRAME');ids.push(m.attribute);}
 unique(ids);
 }
}
function statusCleanse(s){
 const v=s.cleanse;if(v===undefined)return;
 shape(v,['on_tick','on_expired']);
 if(typeof v.on_tick!=='boolean'||typeof v.on_expired!=='boolean'||!v.on_tick&&!v.on_expired||s.shield!==undefined||
    s.periodic!==undefined&&s.periodic.kind!=='max_hp_heal'||v.on_tick&&s.periodic?.kind!=='max_hp_heal'||
    (s.stacks!==undefined||s.max_stacks!==undefined)&&s.stack_counter!==true)fail('INVALID_FRAME');
}
function windowRecord(s){
 const w=s.window;if(w===undefined)return;
 shape(w,['basis','recorded','cap','contacts','maximum_sampling','max_hp_at_apply','max_spirit_at_apply']);
 if(!['none','damage_dealt','damage_taken','shield_absorbed'].includes(w.basis)||!['snapshot','live'].includes(w.maximum_sampling)||!uint(w.cap)||!uint(w.recorded,w.cap)||!uint(w.contacts,4294967295)||!uint(w.max_hp_at_apply,4294967295,1)||!uint(w.max_spirit_at_apply,4294967295)||(w.recorded===0)!==(w.contacts===0)||
     (w.basis==='none' ? w.cap!==0||w.recorded!==0||w.contacts!==0 : w.cap===0)||(s.stacks!==undefined||s.max_stacks!==undefined)&&s.stack_counter!==true||s.shield!==undefined||s.periodic!==undefined&&s.periodic.kind!=='max_hp_heal'||['silence','stun','root','control_immunity','damage_immunity','first_control_immunity','spirit_recovery_lock'].includes(s.attribute))fail('INVALID_FRAME');
}
function retainedSourceStatus(s) {
  if (s.retained_source === undefined) return;
  shape(s,['instance_id','definition_id','source','target','applied_at_ms','expires_at_ms','modifier_bps','attribute','retained_source']);
  ref(s.source);
  ref(s.target);
  if (s.retained_source !== true || s.source.kind !== 'monster' || s.target.kind !== 'participant' ||
      s.attribute !== 'movement_slow' || !Number.isInteger(s.modifier_bps) || s.modifier_bps < -10000 || s.modifier_bps >= 0 ||
      !uint(s.applied_at_ms) || !uint(s.expires_at_ms) || s.expires_at_ms <= s.applied_at_ms) fail('INVALID_FRAME');
}
function shieldView(entries,total,max,exact){
 let sum=0;
 for(const s of entries){
  retainedSourceStatus(s);
  if((s.stacks===undefined)!==(s.max_stacks===undefined)||s.stacks!==undefined&&(!uint(s.max_stacks,128,1)||!uint(s.stacks,s.max_stacks,s.stack_counter===true?0:1)))fail('INVALID_FRAME');
  if(s.modifiers!==undefined&&(s.attribute!==undefined||s.modifier_bps!==0))fail('INVALID_FRAME');
  if(s.periodic!==undefined)validatePeriodic(s.periodic,s);
  statusModifiers(s);
  statusFlags(s);
  statusCleanse(s);
  windowRecord(s);
  if(s.shield===undefined)continue;
  shape(s.shield,['granted','remaining'],['record']);
  if(s.attribute!==undefined||s.stacks!==undefined||s.max_stacks!==undefined||s.modifier_bps!==0||s.window!==undefined||s.periodic!==undefined||!uint(s.shield.granted,4294967295)||!uint(s.shield.remaining,s.shield.granted)||!uint(s.applied_at_ms)||!uint(s.expires_at_ms)||s.expires_at_ms<s.applied_at_ms)fail('INVALID_FRAME');
  if(s.shield.record!==undefined){const r=s.shield.record;shape(r,['basis','recorded','cap']);if(!['incoming','own_absorbed'].includes(r.basis)||!uint(r.cap,4294967295)||!uint(r.recorded,r.cap))fail('INVALID_FRAME');}
  if(s.modifiers!==undefined&&(s.shield.record===undefined||s.modifiers.length!==1||s.modifiers[0].attribute!=='damage_reduction'||s.modifiers[0].modifier_bps<1||s.modifiers[0].modifier_bps>10000))fail('INVALID_FRAME');
  sum+=s.shield.remaining;
 }
 // Grant budgets are frozen. A later maxHP debuff may leave a larger old balance.
 if(total===undefined){if(entries.some(s=>s.shield!==undefined))fail('INVALID_FRAME');}
 else if(!uint(total,4294967295)||(exact?sum!==total:sum>total))fail('INVALID_FRAME');
}
function piercing(r,s){const p=r.piercing;if(p===undefined)return;
shape(p,['skill_id','direction','speed','remaining_range','created_at_ms','expires_at_ms','max_hits','used_hits','remaining_hits','repeat_interval_ms','contacts']);
if(r.unit_target!==undefined||r.source.kind!=='participant'||!s.members.some(m=>same(m.entity,r.source))||!id(p.skill_id)||r.radius>128||!point(p.direction)||Math.abs(Math.hypot(...p.direction)-1)>1e-9||!Number.isFinite(p.speed)||p.speed<.1||p.speed>10000||!Number.isFinite(p.remaining_range)||p.remaining_range<=0||p.remaining_range>10000)fail('INVALID_FRAME');
if(!uint(p.created_at_ms)||!uint(p.expires_at_ms)||p.created_at_ms>s.logical_ms||p.expires_at_ms<=s.logical_ms||p.expires_at_ms<=p.created_at_ms||p.expires_at_ms-p.created_at_ms>60000||!uint(p.max_hits,128,1)||!uint(p.used_hits,p.max_hits-1)||p.remaining_hits!==p.max_hits-p.used_hits||!uint(p.repeat_interval_ms,60000)||!Array.isArray(p.contacts)||p.contacts.length>p.used_hits||(p.used_hits===0)!==(p.contacts.length===0)||(p.repeat_interval_ms===0&&p.contacts.length!==p.used_hits))fail('INVALID_FRAME');
const keys=[];for(const c of p.contacts){shape(c,['target','hit_at_ms','next_eligible_at_ms']);unit(c.target);if(!s.monsters.some(n=>n.entity.id===c.target.id&&n.entity.generation>=c.target.generation)||!uint(c.hit_at_ms)||c.hit_at_ms<=p.created_at_ms||c.hit_at_ms>s.logical_ms||c.hit_at_ms>=p.expires_at_ms||(c.next_eligible_at_ms!==null&&!uint(c.next_eligible_at_ms))||c.next_eligible_at_ms!==(p.repeat_interval_ms>0?c.hit_at_ms+p.repeat_interval_ms:null))fail('INVALID_FRAME');keys.push(JSON.stringify([c.target.id,c.target.generation]));}unique(keys);
}
function movingEnterArea(a,s){
  const m=a.movement,p=a.contact_policy;
  shape(m,['origin','direction','speed','distance','committed']);
  shape(p,['max_targets_per_tick','max_total_hits','repeat_interval_ms','used_hits','remaining_hits','contacts']);
  if(!id(a.id)||!id(a.skill_id)||a.source.kind!=='monster'||!s.monsters.some(n=>n.entity.id===a.source.id&&n.entity.generation>=a.source.generation)||
    Object.hasOwn(a,'line')||Object.hasOwn(a,'basic_action_id')||!point(a.center)||!point(m.origin)||!point(m.direction)||
    Math.abs(Math.hypot(...m.direction)-1)>1e-9||!Number.isFinite(a.radius)||a.radius<.1||a.radius>128||
    !Number.isFinite(m.speed)||m.speed<.1||m.speed>10000||!Number.isFinite(m.distance)||m.distance<0||m.distance>10000||
    !uint(a.created_at_ms)||!uint(a.activates_at_ms)||!uint(a.expires_at_ms)||a.created_at_ms>s.logical_ms||
    !uint(a.activates_at_ms-a.created_at_ms,60000,1)||!uint(a.expires_at_ms-a.activates_at_ms,60000,1)||
    a.expires_at_ms<=s.logical_ms||m.committed!==(s.logical_ms>=a.activates_at_ms)||
    a.interval_ms!==0||a.next_tick_at_ms!==null||a.completed_ticks!==0||a.total_ticks!==0)fail('INVALID_FRAME');
  const travel=m.speed*Math.max(0,s.logical_ms-a.activates_at_ms)/1000;
  if((m.committed&&travel>=m.distance)||a.center.some((x,i)=>Math.abs(x-(m.origin[i]+m.direction[i]*travel))>1e-7)||
    !uint(p.max_targets_per_tick,8,1)||!uint(p.max_total_hits,128,1)||!uint(p.repeat_interval_ms,60000)||
    !uint(p.used_hits,p.max_total_hits-1)||p.remaining_hits!==p.max_total_hits-p.used_hits||
    !Array.isArray(p.contacts)||p.contacts.length>p.used_hits||(p.used_hits===0)!==(p.contacts.length===0)||
    (!m.committed&&p.used_hits!==0))fail('INVALID_FRAME');
  const keys=[];
  for(const c of p.contacts){
    shape(c,['target','last_hit_at_ms','next_eligible_at_ms']);unit(c.target);
    if(!s.members.some(member=>member.entity.id===c.target.id&&member.entity.generation===c.target.generation)||
      !uint(c.last_hit_at_ms)||c.last_hit_at_ms<a.activates_at_ms||c.last_hit_at_ms>s.logical_ms||
      c.next_eligible_at_ms!==(p.repeat_interval_ms>0?c.last_hit_at_ms+p.repeat_interval_ms:null))fail('INVALID_FRAME');
    keys.push(JSON.stringify([c.target.id,c.target.generation]));
  }
  unique(keys);
}
function npcGroundField(a,s){
  for(const k of ['created_at_ms','activates_at_ms','expires_at_ms','interval_ms','completed_ticks','total_ticks']){
    if(!uint(a[k]))fail('INVALID_FRAME');
  }
  const pending=s.logical_ms<a.activates_at_ms, duration=a.expires_at_ms-a.activates_at_ms;
  if(!id(a.id)||!id(a.skill_id)||a.source.kind!=='monster'||
    !s.monsters.some(n=>n.entity.id===a.source.id&&(pending?n.entity.generation===a.source.generation&&n.hp>0:n.entity.generation>=a.source.generation))||
    ['line','basic_action_id','movement'].some(k=>Object.hasOwn(a,k))||!point(a.center)||
    !Number.isFinite(a.radius)||a.radius<.1||a.radius>512||a.created_at_ms>s.logical_ms||
    !uint(a.activates_at_ms-a.created_at_ms,60000,1)||a.expires_at_ms<=s.logical_ms||
    !uint(a.interval_ms,60000,1)||!uint(duration,60000,a.interval_ms)||duration%a.interval_ms!==0||
    !uint(a.total_ticks,128,1)||a.total_ticks!==duration/a.interval_ms||a.completed_ticks>=a.total_ticks||
    a.completed_ticks!==Math.floor(Math.max(0,s.logical_ms-a.activates_at_ms)/a.interval_ms)||
    a.next_tick_at_ms!==a.activates_at_ms+(a.completed_ticks+1)*a.interval_ms)fail('INVALID_FRAME');
  const p=a.contact_policy;
  shape(p,['max_targets_per_tick','max_total_hits','repeat_interval_ms','used_hits','remaining_hits','contacts']);
  if(!uint(p.max_targets_per_tick,16,1)||!uint(p.max_total_hits,256,1)||
    p.repeat_interval_ms!==a.interval_ms||!uint(p.used_hits,p.max_total_hits-1)||
    p.used_hits>a.completed_ticks*p.max_targets_per_tick||p.remaining_hits!==p.max_total_hits-p.used_hits||
    !Array.isArray(p.contacts)||p.contacts.length>p.used_hits||(p.used_hits===0)!==(p.contacts.length===0))fail('INVALID_FRAME');
  const keys=[];
  for(const c of p.contacts){
    shape(c,['target','last_hit_at_ms','next_eligible_at_ms']);unit(c.target);
    if(!s.members.some(m=>m.entity.id===c.target.id&&m.entity.generation===c.target.generation)||
      !uint(c.last_hit_at_ms,s.logical_ms,a.activates_at_ms+a.interval_ms)||
      (c.last_hit_at_ms-a.activates_at_ms)%a.interval_ms!==0||
      c.next_eligible_at_ms!==c.last_hit_at_ms+a.interval_ms)fail('INVALID_FRAME');
    keys.push(JSON.stringify([c.target.id,c.target.generation]));
  }
  unique(keys);
}
function npcGroundStrike(a,s){
  if(!id(a.id)||!id(a.skill_id)||a.source.kind!=='monster'||
    !s.monsters.some(n=>same(n.entity,a.source)&&n.hp>0)||
    ['line','basic_action_id','movement'].some(k=>Object.hasOwn(a,k))||
    !point(a.center)||!Number.isFinite(a.radius)||a.radius<.1||a.radius>512||
    !uint(a.created_at_ms,s.logical_ms)||!uint(a.activates_at_ms)||a.activates_at_ms<=s.logical_ms||
    !uint(a.activates_at_ms-a.created_at_ms,60000,1)||a.expires_at_ms!==a.activates_at_ms||
    a.interval_ms!==0||a.next_tick_at_ms!==a.activates_at_ms||
    a.completed_ticks!==0||a.total_ticks!==1||a.contact_policy!==null)fail('INVALID_FRAME');
}
function memberAreaHistory(a,s){
  const p=a.contact_policy;
  shape(p,['max_targets_per_tick','max_total_hits','repeat_interval_ms','used_hits','remaining_hits','contacts']);
  if(!uint(p.max_targets_per_tick,32,1)||!uint(p.max_total_hits,128,1)||!uint(p.repeat_interval_ms,60000)||
    !uint(p.used_hits,p.max_total_hits-1)||p.remaining_hits!==p.max_total_hits-p.used_hits||
    !Array.isArray(p.contacts)||p.contacts.length>p.used_hits||(p.used_hits===0)!==(p.contacts.length===0))fail('INVALID_FRAME');
  const keys=[];
  for(const c of p.contacts){
    shape(c,['target','last_hit_at_ms','next_eligible_at_ms']);unit(c.target);
    if(!s.monsters.some(n=>n.entity.id===c.target.id&&n.entity.generation>=c.target.generation)||
      !uint(c.last_hit_at_ms)||c.last_hit_at_ms<a.activates_at_ms+a.interval_ms||
      c.last_hit_at_ms>=a.expires_at_ms||c.last_hit_at_ms>s.logical_ms||
      (c.next_eligible_at_ms!==null&&!uint(c.next_eligible_at_ms))||
      c.next_eligible_at_ms!==(p.repeat_interval_ms>0?c.last_hit_at_ms+p.repeat_interval_ms:null))fail('INVALID_FRAME');
    keys.push(JSON.stringify([c.target.id,c.target.generation]));
  }
  unique(keys);
}
function memberGroundTrap(a,s){
  if(!id(a.id)||!id(a.skill_id)||a.source.kind!=='participant'||
    !s.members.some(m=>same(m.entity,a.source)&&m.phase==='active'&&m.hp>0)||
    ['line','basic_action_id','movement'].some(k=>Object.hasOwn(a,k))||
    !point(a.center)||!Number.isFinite(a.radius)||a.radius<.1||a.radius>128||
    !uint(a.created_at_ms,s.logical_ms)||!uint(a.activates_at_ms)||
    !uint(a.activates_at_ms-a.created_at_ms,60000)||!uint(a.expires_at_ms)||
    !uint(a.expires_at_ms-a.activates_at_ms,60000,1)||a.expires_at_ms<=s.logical_ms||
    a.interval_ms!==0||a.next_tick_at_ms!==null||a.completed_ticks!==0||a.total_ticks!==0)fail('INVALID_FRAME');
  memberAreaHistory(a,s);
  const p=a.contact_policy;
  if(p.repeat_interval_ms===0&&p.contacts.length!==p.used_hits)fail('INVALID_FRAME');
}
function memberCastAnchor(a,s){
  if(!id(a.id)||!id(a.skill_id)||a.source.kind!=='participant'||
    !s.members.some(m=>same(m.entity,a.source)&&m.phase==='active'&&m.hp>0)||
    ['line','basic_action_id','movement'].some(k=>Object.hasOwn(a,k))||
    !point(a.center)||a.radius!==0||!uint(a.created_at_ms,s.logical_ms)||
    a.activates_at_ms!==a.created_at_ms||!uint(a.expires_at_ms)||a.expires_at_ms<=s.logical_ms||
    a.expires_at_ms-a.created_at_ms>60000||a.interval_ms!==0||a.next_tick_at_ms!==null||
    a.completed_ticks!==0||a.total_ticks!==0||a.contact_policy!==null)fail('INVALID_FRAME');
  if(Object.hasOwn(a,'anchor_recast')){
    const r=a.anchor_recast;
    shape(r,['action_id','expires_at_ms','max_count','used_count'],['behaviors']);
    if(!id(r.action_id)||!uint(r.expires_at_ms,a.expires_at_ms,a.created_at_ms+1)||
      r.expires_at_ms-a.created_at_ms>30000||!uint(r.max_count,8,1)||
      !uint(r.used_count,r.max_count))fail('INVALID_FRAME');
    if(Object.hasOwn(r,'behaviors')){
      const b=r.behaviors;
      if(!Array.isArray(b)||b.length<1||b.length>3||
        !b.every(v=>['relocate','return_to_source','end'].includes(v))||
        (r.used_count>0&&!b.some(v=>v!=='end')))fail('INVALID_FRAME');
      unique(b);
    }
  }
}
function anchorTransition(previous,next){
  const isAnchor=a=>a.mechanism==='member_cast_anchor';
  const before=previous.areas??[], after=next.areas??[];
  for(const old of before.filter(isAnchor)){
    const current=after.find(a=>a.id===old.id);
    if(!current||same(old,current))continue;
    const a=old.anchor_recast,b=current.anchor_recast;
    if(!isAnchor(current)||!a||!b||!same(old.source,current.source)||
      old.skill_id!==current.skill_id||old.created_at_ms!==current.created_at_ms||
      old.expires_at_ms!==current.expires_at_ms||a.action_id!==b.action_id||
      a.expires_at_ms!==b.expires_at_ms||a.max_count!==b.max_count||!same(a.behaviors,b.behaviors)||
      previous.logical_ms>=a.expires_at_ms||b.used_count<=a.used_count||
      (b.used_count===a.used_count+1&&same(old.center,current.center)))fail('INVALID_FRAME');
  }
  for(const current of after.filter(isAnchor)){
    const old=before.find(a=>a.id===current.id);
    if(old&&!isAnchor(old))fail('INVALID_FRAME');
  }
}
function anchorFieldTransition(previous,next){
  const before=previous.areas??[],after=next.areas??[];
  for(const old of before){
    if(!old.attached_anchor_id)continue;
    const a=after.find(a=>a.id===old.id);
    if(a&&(a.attached_anchor_id!==old.attached_anchor_id||!same(a.source,old.source)||
      ['skill_id','radius','created_at_ms','activates_at_ms','expires_at_ms','interval_ms'].some(k=>a[k]!==old[k])||
      a.completed_ticks<old.completed_ticks))fail('INVALID_FRAME');
  }
  for(const a of after){
    if(!a.attached_anchor_id)continue;
    const old=before.find(b=>b.id===a.id);
    if(old&&!old.attached_anchor_id)fail('INVALID_FRAME');
  }
}
function areas(values,s){if(!Array.isArray(values)||values.length>20)fail('INVALID_FRAME');const ids=[];let basicCount=0,skillCount=0;for(const a of values){shape(a,['id','source','skill_id','mechanism','center','radius','created_at_ms','activates_at_ms','expires_at_ms','interval_ms','next_tick_at_ms','completed_ticks','total_ticks','contact_policy'],['line','basic_action_id','movement','anchor_recast','attached_anchor_id']);const basic=a.mechanism==='basic_field';if(basic?++basicCount>12:++skillCount>8)fail('INVALID_FRAME');ref(a.source);if(Object.hasOwn(a,'attached_anchor_id')&&(a.mechanism!=='field'||!id(a.attached_anchor_id)||a.expires_at_ms<=s.logical_ms))fail('INVALID_FRAME');if(a.mechanism==='member_cast_anchor'){ids.push(a.id);memberCastAnchor(a,s);continue;}if(Object.hasOwn(a,'anchor_recast'))fail('INVALID_FRAME');if(a.mechanism==='moving_enter'){ids.push(a.id);movingEnterArea(a,s);continue;}if(a.mechanism==='npc_ground_field'){ids.push(a.id);npcGroundField(a,s);continue;}
if(a.mechanism==='npc_ground_strike'){ids.push(a.id);npcGroundStrike(a,s);continue;}
if(a.mechanism==='member_ground_trap'){ids.push(a.id);memberGroundTrap(a,s);continue;}
if(Object.hasOwn(a,'movement'))fail('INVALID_FRAME');if(!id(a.id)||!id(a.skill_id)||a.source.kind!=='participant'||!s.members.some(m=>same(m.entity,a.source))||!point(a.center)||!Number.isFinite(a.radius)||a.radius<.1||a.radius>(basic?512:128))fail('INVALID_FRAME');for(const k of ['created_at_ms','activates_at_ms','expires_at_ms','interval_ms','completed_ticks','total_ticks'])if(!uint(a[k]))fail('INVALID_FRAME');if(a.created_at_ms>s.logical_ms||a.activates_at_ms<a.created_at_ms||a.activates_at_ms-a.created_at_ms>60000||a.expires_at_ms<a.activates_at_ms||!uint(a.total_ticks,128,1)||a.completed_ticks>a.total_ticks)fail('INVALID_FRAME');ids.push(a.id);
if(basic){
  const duration=a.expires_at_ms-a.activates_at_ms;
  if(!id(a.basic_action_id)||new TextEncoder().encode(a.skill_id).length>64||
      Object.hasOwn(a,'line')||a.contact_policy!==null||a.created_at_ms!==a.activates_at_ms||
      !uint(a.interval_ms,60000,1)||duration>60000||duration%a.interval_ms!==0||
      duration/a.interval_ms!==a.total_ticks||a.completed_ticks>=a.total_ticks||
      a.next_tick_at_ms!==a.activates_at_ms+(a.completed_ticks+1)*a.interval_ms||
      a.next_tick_at_ms<=s.logical_ms||
      !s.members.some(member=>same(member.entity,a.source)&&member.phase==='active'&&member.hp>0))fail('INVALID_FRAME');
  continue;
}
if(Object.hasOwn(a,'basic_action_id'))fail('INVALID_FRAME');
if(a.mechanism==='line_field'){const l=a.line;shape(l,['start','end','width']);if(!point(l.start)||!point(l.end)||!same(l.start,a.center)||l.width!==a.radius*2||Math.hypot(l.end[0]-l.start[0],l.end[1]-l.start[1])>10000+1e-9)fail('INVALID_FRAME');}else if(Object.hasOwn(a,'line'))fail('INVALID_FRAME');
if(a.mechanism==='circle'){if(a.interval_ms!==0||a.expires_at_ms!==a.activates_at_ms||a.completed_ticks!==0||a.total_ticks!==1||a.next_tick_at_ms!==a.activates_at_ms||a.contact_policy!==null)fail('INVALID_FRAME');continue;}
if(!['field','line_field'].includes(a.mechanism)||!uint(a.interval_ms,60000,1)||a.expires_at_ms-a.activates_at_ms>60000||a.expires_at_ms-a.activates_at_ms<=a.interval_ms||a.total_ticks!==Math.floor((a.expires_at_ms-a.activates_at_ms-1)/a.interval_ms)||a.next_tick_at_ms!==(a.completed_ticks<a.total_ticks?a.activates_at_ms+(a.completed_ticks+1)*a.interval_ms:null))fail('INVALID_FRAME');memberAreaHistory(a,s);}unique(ids);
unique(values.filter(a=>a.mechanism==='member_cast_anchor').map(a=>JSON.stringify([ref(a.source),a.skill_id])));
for(const a of values){
  if(!a.attached_anchor_id)continue;
  const anchor=values.find(b=>b.id===a.attached_anchor_id&&b.mechanism==='member_cast_anchor');
  if(!anchor||!same(anchor.source,a.source)||!same(anchor.center,a.center)||anchor.created_at_ms>a.created_at_ms)fail('INVALID_FRAME');
}
if(values.reduce((sum,a)=>sum+(a.mechanism==='npc_ground_field'?0:a.contact_policy?.max_total_hits??0),0)>128)fail('INVALID_FRAME');}
function basicBeams(beams, snapshot) {
  if (!Array.isArray(beams) || beams.length > 8) fail('INVALID_FRAME');
  const ids = [], sources = [];
  for (const beam of beams) {
    shape(beam, ['action_id','plan_id','source','direction','range','half_width','interval_ms',
      'started_at_ms','next_tick_at_ms','ticks','released']);
    const source = ref(beam.source);
    const own = snapshot.self;
    if (!id(beam.action_id) || !id(beam.plan_id) || new TextEncoder().encode(beam.plan_id).length > 64 ||
        beam.source.kind !== 'participant' ||
        !snapshot.members.some(member => same(member.entity, beam.source) && member.phase === 'active' &&
          member.hp > 0 && !member.protected) ||
        (snapshot.basic_waves ?? []).some(wave => same(wave.source, beam.source)) ||
        (snapshot.basic_impacts ?? []).some(plan => same(plan.source, beam.source)) ||
        !point(beam.direction) || Math.abs(Math.hypot(...beam.direction) - 1) > 1e-9 ||
        !Number.isFinite(beam.range) || beam.range < 0.1 || beam.range > 10000 ||
        !Number.isFinite(beam.half_width) || beam.half_width < 0.1 || beam.half_width > 128 ||
        !uint(beam.interval_ms, 60000, 1) || !uint(beam.started_at_ms, snapshot.logical_ms) ||
        !uint(beam.next_tick_at_ms) || beam.next_tick_at_ms <= snapshot.logical_ms ||
        !uint(beam.ticks, 4294967295) ||
        beam.next_tick_at_ms - beam.started_at_ms !== (beam.ticks + 1) * beam.interval_ms ||
        typeof beam.released !== 'boolean' || (beam.released && beam.ticks !== 0) ||
        (same(beam.source, own.participant.entity) && (!own.ready || own.protected || own.pending_action !== null ||
          own.active_channel !== undefined || own.basic_pursuit !== undefined || own.extraction_finishes_ms !== null))) fail('INVALID_FRAME');
    ids.push(beam.action_id);
    sources.push(source);
  }
  unique(ids);
  unique(sources);
}

function basicImpacts(plans, snapshot) {
  if (!Array.isArray(plans) || plans.length > 8) fail('INVALID_FRAME');
  const ids = [], sources = [];
  for (const plan of plans) {
    const ground = plan.kind === 'ground_field';
    shape(plan, ['id','plan_id','source','kind','origin',ground ? 'position' : 'target','released_at_ms','impact_at_ms']);
    const source = ref(plan.source);
    if (ground) {
      if (!point(plan.position)) fail('INVALID_FRAME');
    } else {
      unit(plan.target);
    }
    if ((!ground && plan.kind !== 'fixed_delay') || !id(plan.id) || !id(plan.plan_id) ||
        new TextEncoder().encode(plan.plan_id).length > 64 ||
        plan.source.kind !== 'participant' ||
        !snapshot.members.some(member => same(member.entity, plan.source) && member.phase === 'active' && member.hp > 0) ||
        (snapshot.basic_waves ?? []).some(wave => same(wave.source, plan.source)) ||
        !point(plan.origin) || !uint(plan.released_at_ms, snapshot.logical_ms) ||
        !uint(plan.impact_at_ms) || plan.impact_at_ms <= snapshot.logical_ms ||
        plan.impact_at_ms - plan.released_at_ms > 60000 ||
        (!ground && !snapshot.monsters.some(monster => monster.entity.id === plan.target.id && monster.entity.generation >= plan.target.generation))) fail('INVALID_FRAME');
    ids.push(plan.id);
    sources.push(source);
  }
  unique(ids);
  unique(sources);
}

function basicWaves(waves, snapshot) {
  if (!Array.isArray(waves) || waves.length > 8) fail('INVALID_FRAME');
  const ids = [], sources = [];
  const inRange = (value, max) => Number.isFinite(value) && value >= 0.1 && value <= max;
  for (const wave of waves) {
    shape(wave, ['id','plan_id','source','origin','geometry','range','speed','released_at_ms','pending']);
    const source = ref(wave.source);
    if (!id(wave.id) || !id(wave.plan_id) || new TextEncoder().encode(wave.plan_id).length > 64 ||
        wave.source.kind !== 'participant' ||
        !snapshot.members.some(member => same(member.entity, wave.source) && member.phase === 'active' && member.hp > 0) ||
        !point(wave.origin) || !inRange(wave.range, 10000) || !inRange(wave.speed, 10000) ||
        !uint(wave.released_at_ms, snapshot.logical_ms) ||
        !Array.isArray(wave.pending) || wave.pending.length < 1 || wave.pending.length > 16) fail('INVALID_FRAME');
    const { geometry } = wave;
    if (geometry?.kind === 'directional') {
      shape(geometry, ['kind','direction','cone_degrees']);
      if (!point(geometry.direction) || Math.abs(Math.hypot(...geometry.direction) - 1) > 1e-9 ||
          !inRange(geometry.cone_degrees, 180)) fail('INVALID_FRAME');
    } else if (geometry?.kind === 'landing') {
      shape(geometry, ['kind','from']);
      if (!point(geometry.from) || wave.range > 512) fail('INVALID_FRAME');
    } else {
      shape(geometry, ['kind']);
      if (geometry.kind !== 'radial') fail('INVALID_FRAME');
    }
    const landing = geometry.kind === 'landing';
    const travelDistance = landing ? Math.hypot(geometry.from[0]-wave.origin[0], geometry.from[1]-wave.origin[1]) : wave.range;
    const travel = travelDistance * 1000 / wave.speed;
    if (travel > 60000) fail('INVALID_FRAME');
    const lastArrival = wave.released_at_ms + Math.max(1, Math.ceil(travel));
    let previous = snapshot.logical_ms;
    const targets = [];
    for (const contact of wave.pending) {
      shape(contact, ['target','at_ms']);
      unit(contact.target);
      if (!uint(contact.at_ms) || contact.at_ms <= snapshot.logical_ms || contact.at_ms < previous ||
          (landing ? (Math.abs(contact.at_ms-lastArrival)>1 || previous>snapshot.logical_ms&&contact.at_ms!==previous) : contact.at_ms > lastArrival) || !snapshot.monsters.some(monster =>
            monster.entity.id === contact.target.id && monster.entity.generation >= contact.target.generation)) fail('INVALID_FRAME');
      previous = contact.at_ms;
      targets.push(contact.target.id);
    }
    unique(targets);
    ids.push(wave.id);
    sources.push(source);
  }
  unique(ids);
  unique(sources);
}

function validateChannel(snapshot) {
  const own = snapshot.self;
  if (!Object.hasOwn(own, 'active_channel')) return;
  const channel = own.active_channel;
  shape(channel, ['action_id', 'skill_id', 'direction', 'started_at_ms', 'ends_at_ms', 'next_pulse_at_ms', 'pulses'], ['total_pulses','recovery','target']);
  if (Object.hasOwn(channel,'target')) {
    ref(channel.target);
    const t=channel.target;
    if(!Object.hasOwn(channel,'total_pulses') || (channel.recovery===true ? t.kind!=='participant' || same(t,own.participant.entity) || !snapshot.members.some(m=>same(m.entity,t)) : t.kind!=='monster' || !snapshot.monsters.some(n=>same(n.entity,t)))) fail('INVALID_FRAME');
  }
  const { started_at_ms: start, ends_at_ms: end, next_pulse_at_ms: next, pulses } = channel;
  if (!id(channel.action_id) || !own.player_skills.some(skill => skill.skill_id === channel.skill_id) ||
      snapshot.phase !== 'active' || own.phase !== 'active' || own.protected || !own.ready || own.hp === 0 ||
      own.pending_action !== null || own.extraction_finishes_ms !== null || own.recovery_until_ms > start ||
      !point(channel.direction) || (Object.hasOwn(channel,'recovery') ?
        channel.recovery !== true || !Object.hasOwn(channel,'total_pulses') || channel.direction.some(v=>v!==0) :
        Math.abs(Math.hypot(...channel.direction) - 1) > 1e-9 && !(channel.target && channel.direction.every(v=>v===0))) ||
      !uint(start) || !uint(end) || start > snapshot.logical_ms || end <= snapshot.logical_ms || end - start > 30000 ||
      !uint(pulses, 128) || (next === null ? pulses === 0 : !uint(next) || next <= snapshot.logical_ms || next > end || (next === end && !Object.hasOwn(channel,'total_pulses'))) ||
      (Object.hasOwn(channel,'total_pulses') && (!uint(channel.total_pulses,128,1) || pulses >= channel.total_pulses ||
        (end-start)%channel.total_pulses !== 0 || next !== start+(pulses+1)*((end-start)/channel.total_pulses)))) {
    fail('INVALID_FRAME');
  }
}
function validateRecast(snapshot) {
  const own = snapshot.self;
  if (!Object.hasOwn(own, 'projectile_recast')) return;
  const recast = own.projectile_recast;
  shape(recast, ['action_id', 'skill_id', 'projectile_id', 'expires_at_ms', 'next_index']);
  if (!id(recast.action_id) || !id(recast.projectile_id) || recast.next_index !== 1 ||
      snapshot.phase !== 'active' || own.phase !== 'active' || own.hp === 0 ||
      !uint(recast.expires_at_ms) || recast.expires_at_ms <= snapshot.logical_ms ||
      recast.expires_at_ms - snapshot.logical_ms > 30000 ||
      !own.player_skills.some(skill => skill.skill_id === recast.skill_id) ||
      !snapshot.projectiles.some(shot => shot.id === recast.projectile_id &&
        same(shot.source, own.participant.entity) && !shot.unit_target && !shot.piercing)) fail('INVALID_FRAME');
}
function validateFollowup(snapshot) {
  const own = snapshot.self;
  if (!Object.hasOwn(own, 'followup')) return;
  const followup = own.followup;
  shape(followup, ['source_skill_id', 'promotes_at_ms', 'queued']);
  const { source_skill_id: source, promotes_at_ms: release, queued } = followup;
  if (snapshot.phase !== 'active' || own.phase !== 'active' || own.protected || !own.ready || own.hp === 0 ||
      own.pending_action !== null || own.active_channel || own.extraction_finishes_ms !== null ||
      own.recovery_until_ms <= snapshot.logical_ms || !uint(release) ||
      release < own.recovery_until_ms || release - own.recovery_until_ms >= 100 ||
      !own.player_skills.some(slot => slot.skill_id === source)) fail('INVALID_FRAME');
  if (queued === null) return;
  shape(queued, ['action_id', 'skill_id', 'direction', 'expires_at_ms']);
  if (!id(queued.action_id) || queued.skill_id === source ||
      !own.player_skills.some(slot => slot.skill_id === queued.skill_id && slot.ready_at_ms <= release) ||
      !uint(queued.expires_at_ms) || queued.expires_at_ms < release || queued.expires_at_ms - snapshot.logical_ms > 1000 ||
      !point(queued.direction) || Math.abs(Math.hypot(...queued.direction) - 1) > 1e-9) fail('INVALID_FRAME');
}
function validateRecoveryCancel(snapshot) {
  const own = snapshot.self;
  if (!Object.hasOwn(own, 'recovery_cancel')) return;
  const cancel = own.recovery_cancel;
  shape(cancel, ['skill_id', 'cancel_at_ms']);
  if (snapshot.phase !== 'active' || own.phase !== 'active' || own.protected || !own.ready || own.hp === 0 ||
      own.pending_action !== null || own.active_channel || own.extraction_finishes_ms !== null ||
      own.recovery_until_ms <= snapshot.logical_ms ||
      !uint(cancel.cancel_at_ms) || cancel.cancel_at_ms >= own.recovery_until_ms ||
      !own.player_skills.some(slot => slot.skill_id === cancel.skill_id) ||
      (own.followup && own.followup.source_skill_id !== cancel.skill_id)) fail('INVALID_FRAME');
}
function validateBasicFollowup(snapshot) {
  const own = snapshot.self;
  if (!Object.hasOwn(own, 'basic_followup')) return;
  const queued = own.basic_followup;
  shape(queued, ['action_id', 'target', 'promotes_at_ms', 'expires_at_ms']);
  unit(queued.target);
  const { promotes_at_ms: release, expires_at_ms: end } = queued;
  if (snapshot.phase !== 'active' || own.phase !== 'active' || own.protected || !own.ready || own.hp === 0 ||
      own.pending_action !== null || own.active_channel || own.followup || own.recovery_cancel ||
      own.extraction_finishes_ms !== null || own.recovery_until_ms > snapshot.logical_ms ||
      !id(queued.action_id) || own.basic_ready_at_ms === 0 || !uint(release) ||
      release < own.basic_ready_at_ms || release - own.basic_ready_at_ms >= 100 ||
      !uint(end) || end <= snapshot.logical_ms || end < release || end - release > 1000) fail('INVALID_FRAME');
}
function validateBasicPursuit(snapshot) {
  const own = snapshot.self;
  if (!Object.hasOwn(own, 'basic_pursuit')) return;
  const plan = own.basic_pursuit;
  shape(plan, ['action_id', 'target', 'started_at_ms', 'expires_at_ms']);
  unit(plan.target);
  const { started_at_ms: start, expires_at_ms: end } = plan;
  if (snapshot.phase !== 'active' || own.phase !== 'active' || own.protected || !own.ready || own.hp === 0 ||
      own.pending_action !== null || own.active_channel || own.basic_followup ||
      own.extraction_finishes_ms !== null || own.recovery_until_ms > snapshot.logical_ms ||
      own.basic_ready_at_ms > snapshot.logical_ms || !id(plan.action_id) ||
      !uint(start) || !uint(end) || start > snapshot.logical_ms || end <= snapshot.logical_ms || end - start > 1800) {
    fail('INVALID_FRAME');
  }
}
function validateItemCooldowns(snapshot) {
  const cooldowns = snapshot.self.item_cooldowns;
  if (cooldowns === undefined) return;
  if (!Array.isArray(cooldowns) || cooldowns.length > 16) fail('INVALID_FRAME');
  const ids = [];
  const encoder = new TextEncoder();
  for (const entry of cooldowns) {
    shape(entry, ['item_id', 'ready_at_ms']);
    if (!id(entry.item_id) || encoder.encode(entry.item_id).length > 64 ||
        !uint(entry.ready_at_ms) || entry.ready_at_ms <= snapshot.logical_ms ||
        entry.ready_at_ms - snapshot.logical_ms > 60000) fail('INVALID_FRAME');
    ids.push(entry.item_id);
  }
  unique(ids);
}
function basicProjectileProgress(progress, phase) {
  shape(progress, ['stored','maximum','attack_speed_bonus_bps']);
  const { stored, maximum, attack_speed_bonus_bps: bonus } = progress;
  if (!['deploying','active'].includes(phase) || !uint(maximum, 8, 2) ||
      !uint(stored, maximum) || !uint(bonus, 10000) ||
      (stored === 0 ? bonus !== 0 :
        bonus === 0 || bonus % stored !== 0 || bonus / stored * maximum > 10000)) fail('INVALID_FRAME');
}
function validateBasicProjectileSegments(snapshot) {
  const groups = new Map();
  for (const {id: projectileId, basic_segment: segment, source, piercing, unit_target, radius} of snapshot.projectiles) {
    if (segment === undefined) continue;
    shape(segment, ['action_id','plan_id','index','launched_at_ms','aim_point'], ['role','tier']);
    const { action_id, plan_id, index, launched_at_ms, aim_point, role, tier } = segment;
    if (!id(action_id) || !id(plan_id) || new TextEncoder().encode(plan_id).length > 64 ||
        !uint(index, 8) || !uint(launched_at_ms, snapshot.logical_ms) ||
        snapshot.logical_ms - launched_at_ms >= 60000 || !point(aim_point) ||
        piercing !== undefined || source.kind !== 'participant' ||
        !snapshot.members.some(member => same(member.entity, source) && member.phase === 'active' &&
          (!['seal_outbound','seal_recall'].includes(role) || member.basic_seals !== undefined))) fail('INVALID_FRAME');
    if (role === 'charged_flight' ? !uint(tier,3,1) || index !== 0 || unit_target !== undefined ||
        radius < 0.1 || radius > 128 || snapshot.projectiles.some(other =>
          other.id !== projectileId && other.basic_segment !== undefined && same(other.source,source)) :
        tier !== undefined) fail('INVALID_FRAME');
    if (role !== undefined && (!['seal_outbound','seal_recall','ricochet','return_to_owner','charged_flight'].includes(role) ||
        (role === 'seal_outbound' && index !== 0) || (role === 'ricochet' && index === 8) ||
        (['seal_recall','return_to_owner'].includes(role) && (index === 0 || unit_target !== undefined)))) fail('INVALID_FRAME');
    const key = ref(source);
    if (!groups.has(key)) groups.set(key, {action_id, plan_id, indexes: new Set()});
    const group = groups.get(key);
    if (group.action_id !== action_id || group.plan_id !== plan_id || group.indexes.has(index)) fail('INVALID_FRAME');
    group.indexes.add(index);
  }
}
function validateBasicCharge(snapshot) {
  const own = snapshot.self;
  const ready = own.basic_charge_ready_at_ms;
  const charge = own.basic_charge;
  if (ready !== undefined && (own.phase !== 'active' || !uint(ready) || ready - snapshot.logical_ms > 60000)) {
    fail('INVALID_FRAME');
  }
  if (charge === undefined) return;
  shape(charge, ['session_id','kind','started_at_ms','max_charge_at_ms','tier']);
  if (!id(charge.session_id) || own.phase !== 'active' || own.protected || !own.ready ||
      own.pending_action !== null || own.basic_pursuit != null || own.active_channel != null ||
      own.extraction_finishes_ms !== null || !uint(charge.started_at_ms, snapshot.logical_ms) ||
      !uint(charge.max_charge_at_ms) ||
      !uint(charge.max_charge_at_ms - charge.started_at_ms, 30000, 1) ||
      !['staged_wave','guard','dash','flight'].includes(charge.kind) ||
      !uint(charge.tier, ['staged_wave','flight'].includes(charge.kind) ? 3 : 1, 1) ||
      (charge.kind === 'dash' ? ready === undefined || ready > charge.started_at_ms : ready !== undefined) ||
      (charge.kind === 'guard' && snapshot.logical_ms >= charge.max_charge_at_ms)) fail('INVALID_FRAME');
}

function memberCombat(snapshot) {
  return snapshot.member_conflict === true || snapshot.self.mode?.objective === 'contest';
}

function validateUnitTargets(snapshot) {
  const matches = (entity, target) => entity.id === target.id && entity.generation === target.generation;
  const validateHostile = (target, source) => {
    unit(target);
    if (!snapshot.monsters.some(npc => npc.summon === undefined && matches(npc.entity, target) &&
        (npc.training_owner_id === undefined || npc.training_owner_id === source.id)) &&
        !(memberCombat(snapshot) && snapshot.members.some(member => matches(member.entity, target) && !same(member.entity, source)))) fail('INVALID_FRAME');
  };
  const pending = snapshot.self.pending_action;
  if (pending?.unit_target !== undefined) {
    if (pending.ground_position !== undefined) fail('INVALID_FRAME');
    validateHostile(pending.unit_target, snapshot.self.participant.entity);
  }
  if (pending?.ally_target !== undefined) {
    const target = pending.ally_target;
    unit(target);
    const marker = target.id === snapshot.self.participant.entity.id ? true : undefined;
    if (pending.skill_id === null || pending.ground_position !== undefined ||
        pending.unit_target !== undefined || pending.charge !== undefined ||
        pending.ally_target_is_self !== marker || (memberCombat(snapshot) && marker !== true) ||
        !snapshot.members.some(member => matches(member.entity, target))) fail('INVALID_FRAME');
  } else if (pending?.ally_target_is_self !== undefined) fail('INVALID_FRAME');
  for (const shot of snapshot.projectiles) {
    if (shot.unit_target === undefined) continue;
    validateHostile(shot.unit_target, shot.source);
    if (shot.source.kind !== 'participant' ||
        !snapshot.members.some(member => same(member.entity, shot.source))) fail('INVALID_FRAME');
  }
}

function isMemberCombatStatus(status, snapshot) {
  if (!memberCombat(snapshot) || !snapshot.members.some(member => same(member.entity, status.source))) return false;
  projectedStatus(status, {entity:snapshot.self.participant.entity}, snapshot, true);
  statusModifiers(status);
  statusFlags(status);
  return true;
}

const modeObjectives=['wait','checkpoint','clear_targets','survive','waves','kill_quota','contest'];
function validateModeWave(stage,now) {
  const wave=stage.wave;
  if ((wave!==undefined)!==(stage.objective==='waves')) fail('INVALID_FRAME');
  if (wave===undefined) return;
  shape(wave,['index','total','opens_at_ms','opened']);
  if (!uint(wave.total,8,1)||!uint(wave.index,wave.total,1)||
      !uint(wave.opens_at_ms,stage.expires_at_ms,stage.opens_at_ms)||
      typeof wave.opened!=='boolean'||
      wave.opened&&(wave.opens_at_ms>now||wave.opens_at_ms===stage.expires_at_ms)) fail('INVALID_FRAME');
}

function waveTransition(a,b) {
  if (a===undefined||b===undefined) return a===b;
  return a.total===b.total&&b.index>=a.index&&
    (a.index===b.index?a.opens_at_ms===b.opens_at_ms&&(!a.opened||b.opened):b.opens_at_ms>=a.opens_at_ms);
}

function validateModeProgress(stage) {
  const progress=stage.progress;
  if ((progress!==undefined)!==(stage.objective==='kill_quota')) fail('INVALID_FRAME');
  if (progress===undefined) return;
  shape(progress,['current','required']);
  if (!uint(progress.required,4294967295,1)||!uint(progress.current,progress.required)) fail('INVALID_FRAME');
}

function quotaTransition(a,b) {
  if (a===undefined||b===undefined) return a===b;
  return a.required===b.required&&a.current<=b.current;
}

function extractionChannel(channel,activeAt) {
  shape(channel,['position','started_at_ms','ends_at_ms']);
  if (!point(channel.position)||!uint(activeAt)||!uint(channel.started_at_ms,Number.MAX_SAFE_INTEGER,activeAt)||
      !uint(channel.ends_at_ms,Number.MAX_SAFE_INTEGER,channel.started_at_ms+1)) fail('INVALID_FRAME');
}

function validateExtraction(snapshot) {
  const pressure=snapshot.extraction_pressure, now=snapshot.logical_ms, active=snapshot.active_at_ms;
  if (pressure!==undefined) {
    shape(pressure,['grace_at_ms','ends_at_ms','phase'],['warning']);
    const {grace_at_ms:grace,ends_at_ms:end,phase,warning}=pressure;
    const expected=now>=end?'expired':now>=grace?'grace':'exploring';
    const frozen=['settling','terminal'].includes(snapshot.self?.phase);
    const phases=['exploring','grace','expired'];
    if (!uint(active)||!uint(grace,Number.MAX_SAFE_INTEGER,active+1)||!uint(end,Number.MAX_SAFE_INTEGER,grace+1)||
        !phases.includes(phase)||(frozen?phases.indexOf(phase)>phases.indexOf(expected):phase!==expected)||
        snapshot.self?.mode!==undefined||snapshot.self?.training!==undefined) fail('INVALID_FRAME');
    if (warning!==undefined) {
      shape(warning,['id','index']);
      if (phase!=='exploring'||!id(warning.id)||new TextEncoder().encode(warning.id).length>64||
          !uint(warning.index,8,1)) fail('INVALID_FRAME');
    }
  }
  for (const member of snapshot.members) {
    const channel=member.extraction_channel;
    if (channel===undefined) continue;
    extractionChannel(channel,active);
    if (member.phase!=='active'||member.hp===0||member.protected===true||
        channel.started_at_ms>now||channel.ends_at_ms<=now||
        snapshot.self!==undefined&&(!snapshot.extraction_open||
          same(member.entity,snapshot.self.participant.entity)&&snapshot.self.extraction_finishes_ms!==channel.ends_at_ms)) fail('INVALID_FRAME');
  }
}

function extractionTransition(before,after) {
  const a=before.extraction_pressure, b=after.extraction_pressure;
  if (a===undefined) {
    if (b!==undefined&&before.active_at_ms!==null) fail('INVALID_FRAME');
    return;
  }
  const phases=['exploring','grace','expired'];
  if (b===undefined||a.grace_at_ms!==b.grace_at_ms||a.ends_at_ms!==b.ends_at_ms||
      phases.indexOf(a.phase)>phases.indexOf(b.phase)||
      b.phase==='exploring'&&a.warning!==undefined&&(b.warning===undefined||
        b.warning.index<a.warning.index||b.warning.index===a.warning.index&&b.warning.id!==a.warning.id)) fail('INVALID_FRAME');
}

function validateEncounter(snapshot) {
  const p=snapshot.encounter_progress;
  if (p===undefined) return;
  shape(p,['initial','remaining','stats_bps','surge_started','surge_ends_at_ms']);
  const end=p.surge_ends_at_ms;
  if (snapshot.self?.training!==undefined||!uint(p.initial,4294967295,1)||!uint(p.remaining,p.initial)||
      !uint(p.stats_bps,100000,1)||typeof p.surge_started!=='boolean'||
      p.surge_started&&p.remaining!==0||end!==null&&(!p.surge_started||!uint(end,Number.MAX_SAFE_INTEGER,1)||
        (snapshot.self?.phase??snapshot.phase)==='active'&&end<=snapshot.logical_ms)) fail('INVALID_FRAME');
}

function encounterTransition(a,b,now) {
  if ((a===undefined)!==(b===undefined)||a!==undefined&&
      (a.initial!==b.initial||a.remaining<b.remaining||
       a.surge_started&&(!b.surge_started||b.surge_ends_at_ms!==null&&a.surge_ends_at_ms!==b.surge_ends_at_ms)||
       b.surge_ends_at_ms===null&&a.surge_ends_at_ms!==null&&now<a.surge_ends_at_ms||
       a.remaining===b.remaining&&a.surge_ends_at_ms===b.surge_ends_at_ms&&a.stats_bps!==b.stats_bps)) fail('INVALID_FRAME');
}

function validateMode(snapshot) {
  const mode=snapshot.self.mode;
  if (mode===undefined) return;
  shape(mode,['stage_id','stage_index','objective','opens_at_ms','expires_at_ms','submitted','respawns_left','respawn_at_ms','outcome'],['forfeit_at_ms','wave','progress']);
  if (!id(mode.stage_id) || new TextEncoder().encode(mode.stage_id).length>64 || !uint(mode.stage_index,8,1) ||
      !modeObjectives.includes(mode.objective) ||
      snapshot.active_at_ms===null || !uint(mode.opens_at_ms,snapshot.logical_ms,snapshot.active_at_ms) ||
      !uint(mode.expires_at_ms,Number.MAX_SAFE_INTEGER,mode.opens_at_ms+1) ||
      typeof mode.submitted!=='boolean' || mode.submitted&&mode.objective!=='checkpoint' ||
      !uint(mode.respawns_left,8) || snapshot.extraction_open ||
      mode.respawn_at_ms!==null&&(!uint(mode.respawn_at_ms,Number.MAX_SAFE_INTEGER,snapshot.logical_ms+1)||snapshot.self.hp!==0||mode.outcome!==null) ||
      mode.forfeit_at_ms!==undefined&&(!uint(mode.forfeit_at_ms,Number.MAX_SAFE_INTEGER,snapshot.logical_ms+1)||snapshot.self.ready||mode.outcome!==null) ||
      (mode.outcome===null?snapshot.self.phase!=='active'||snapshot.logical_ms>=mode.expires_at_ms||snapshot.self.hp===0&&mode.respawn_at_ms===null:
        !['win','loss','draw','death'].includes(mode.outcome)||!['settling','terminal'].includes(snapshot.self.phase))) fail('INVALID_FRAME');
  validateModeWave(mode,snapshot.logical_ms);
  validateModeProgress(mode);
}

function modeTransition(previous,next) {
  const a=previous.self.mode, b=next.self.mode;
  if (a===undefined) {
    if (b!==undefined&&previous.active_at_ms!==null) fail('INVALID_FRAME');
    return;
  }
  const old=previous.self.participant.entity, entity=next.self.participant.entity;
  if (b===undefined||entity.generation<old.generation||b.respawns_left>a.respawns_left||
      entity.generation-old.generation>a.respawns_left-b.respawns_left+Number(a.respawn_at_ms!==null)||b.stage_index<a.stage_index||
      a.outcome!==null&&(!same(a,b)||!same(old,entity))||
      (a.stage_index===b.stage_index?
        a.stage_id!==b.stage_id||a.objective!==b.objective||a.opens_at_ms!==b.opens_at_ms||
        a.expires_at_ms!==b.expires_at_ms||a.submitted&&!b.submitted||!waveTransition(a.wave,b.wave)||!quotaTransition(a.progress,b.progress):
        b.opens_at_ms<a.opens_at_ms)) fail('INVALID_FRAME');
}
function validateTraining(snapshot) {
  const own=snapshot.self, training=own.training;
  if (training===undefined) return;
  shape(training,['respawn_at_ms'],['metrics']);
  const at=training.respawn_at_ms;
  if (snapshot.member_conflict===true||own.mode!==undefined||snapshot.extraction_open||
      (at!==null)!==(own.phase==='active'&&own.hp===0)||
      at!==null&&!uint(at,Number.MAX_SAFE_INTEGER,snapshot.logical_ms+1)) fail('INVALID_FRAME');
  const m=training.metrics;
  if (m===undefined) return;
  shape(m,['sampled_at_ms','damage','hits','critical_hits','highest_hit','rolling_dps','average_dps','by_source']);
  const sources=['basic','skill','periodic','summon','other'];
  shape(m.by_source,sources);
  if (!uint(m.sampled_at_ms,snapshot.logical_ms)||![m.damage,m.rolling_dps,m.average_dps].every(v=>uint(v))||
      !uint(m.hits,m.damage)||!uint(m.critical_hits,m.hits)||
      !uint(m.highest_hit,Math.min(m.damage,4294967295))||
      (m.damage===0)!==(m.highest_hit===0)||(m.damage===0)!==(m.hits===0)||
      m.damage===0&&(m.rolling_dps!==0||m.average_dps!==0)||
      !sources.every(key=>uint(m.by_source[key]))||
      sources.reduce((sum,key)=>sum+m.by_source[key],0)!==m.damage) fail('INVALID_FRAME');
}

function trainingTransition(before,after) {
  const a=before.training, b=after.training;
  if ((a===undefined)!==(b===undefined)||a!==undefined&&
      (before.participant.entity.generation>after.participant.entity.generation||
       (a.metrics===undefined)!==(b.metrics===undefined)||
       a.metrics?.sampled_at_ms>b.metrics?.sampled_at_ms)) fail('INVALID_FRAME');
}

function validateCheckpoints(snapshot){
  const hasZones=Object.hasOwn(snapshot,'checkpoint_zones');
  const hasProgress=Object.hasOwn(snapshot.self,'checkpoint_progress');
  if(!hasZones&&!hasProgress)return;
  const zones=snapshot.checkpoint_zones, progress=snapshot.self.checkpoint_progress;
  if(!Array.isArray(zones)||!Array.isArray(progress)||progress.length<1||progress.length>4||
    zones.length>progress.length||zones.length>0&&(snapshot.phase!=='active'||snapshot.self.phase!=='active'))fail('INVALID_FRAME');
  const schedules=new Map();
  for(const row of progress){
    shape(row,['schedule_id','collected','required','total_windows','last_window_index'],['counter']);
    if(!id(row.schedule_id)||schedules.has(row.schedule_id)||!uint(row.total_windows,32,1)||
      !uint(row.required,row.total_windows,1)||!uint(row.collected,row.total_windows)||
      !uint(row.last_window_index,row.total_windows,row.collected)||
      (row.collected===0)!==(row.last_window_index===0))fail('INVALID_FRAME');
    if(row.counter!==undefined){
      const counter=row.counter;
      shape(counter,['window_index','target','opens_at_ms','expires_at_ms'],['on_contact']);
      unit(counter.target);
      if(snapshot.phase!=='active'||snapshot.self.phase!=='active'||snapshot.active_at_ms===null||
        !uint(counter.window_index,row.last_window_index,row.last_window_index)||counter.window_index===0||
        !uint(counter.opens_at_ms,snapshot.logical_ms,snapshot.active_at_ms)||
        !uint(counter.expires_at_ms,Number.MAX_SAFE_INTEGER,snapshot.logical_ms+1)||
        counter.on_contact!==undefined&&counter.on_contact!==true||
        counter.expires_at_ms-counter.opens_at_ms>60000||
        !snapshot.monsters.some(n=>!n.summon&&n.hp>0&&n.entity.id===counter.target.id&&n.entity.generation===counter.target.generation))fail('INVALID_FRAME');
    }
    schedules.set(row.schedule_id,row);
  }
  const ids=[];
  for(const zone of zones){
    shape(zone,['schedule_id','window_index','center','radius','opens_at_ms','expires_at_ms'],['avoid']);
    const row=schedules.get(zone.schedule_id);
    if(!row||!uint(zone.window_index,row.total_windows,Math.max(1,row.last_window_index))||
      !point(zone.center)||!Number.isFinite(zone.radius)||zone.radius<.1||zone.radius>512||
      snapshot.active_at_ms===null||!uint(zone.opens_at_ms,snapshot.logical_ms,snapshot.active_at_ms)||
      !uint(zone.expires_at_ms,Number.MAX_SAFE_INTEGER,snapshot.logical_ms+1)||
      zone.avoid!==undefined&&zone.avoid!==true||
      zone.expires_at_ms-zone.opens_at_ms>60000)fail('INVALID_FRAME');
    ids.push(zone.schedule_id);
  }
  unique(ids);
}
function checkpointTransition(previous,next){
  const a=previous.self.checkpoint_progress, b=next.self.checkpoint_progress;
  if(a===undefined&&b===undefined)return;
  if(!a||!b||a.length!==b.length)fail('INVALID_FRAME');
  for(let i=0;i<a.length;i++){
    const old=a[i], current=b[i], added=current.collected-old.collected;
    if(old.schedule_id!==current.schedule_id||old.required!==current.required||
      old.total_windows!==current.total_windows||added<0||
      current.last_window_index===old.last_window_index&&current.counter!==undefined&&!same(current.counter,old.counter)||
      (added===0?old.last_window_index!==current.last_window_index:
        old.last_window_index+added>current.last_window_index))fail('INVALID_FRAME');
  }
  for(const old of previous.checkpoint_zones){
    const current=next.checkpoint_zones.find(z=>z.schedule_id===old.schedule_id);
    if(current&&current.window_index<=old.window_index&&!same(current,old))fail('INVALID_FRAME');
  }
}
function validateActiveBlockers(ids){
  if(ids===undefined)return;
  if(!Array.isArray(ids)||ids.length>64||!ids.every(id))fail('INVALID_FRAME');
  unique(ids);
}
function validateContainers(rows){
 if(rows===undefined)return;
 if(!Array.isArray(rows)||rows.length>16)fail('INVALID_FRAME');
 for(const row of rows){shape(row,['id','generation','position','state']);if(!id(row.id)||!uint(row.generation,4294967295,1)||!point(row.position)||!['ready','reserved','cooldown'].includes(row.state))fail('INVALID_FRAME');}
 unique(rows.map(row=>row.id));
}
function snapshot(v,pid){shape(v,['phase','logical_ms','active_at_ms','self','members','monsters','resources','projectiles','extraction_point','extraction_open'],['areas','basic_waves','basic_impacts','basic_beams','checkpoint_zones','active_blockers','containers','relics','member_conflict','encounter_progress','extraction_pressure','contested_equipment']);if(v.member_conflict!==undefined&&(v.member_conflict!==true||v.self?.mode!==undefined))fail('INVALID_FRAME');state({phase:v.phase,active_at_ms:v.active_at_ms,extraction_point:v.extraction_point,extraction_open:v.extraction_open});if(!uint(v.logical_ms))fail('INVALID_FRAME');own(v.self,pid);validateActiveBlockers(v.active_blockers);validateContainers(v.containers);validateCheckpoints(v);validateMode(v);validateTraining(v);validateEncounter(v);validateEquipment(v.self);skills(v.self,v.logical_ms);validateItemCooldowns(v);validateChannel(v);validateFollowup(v);validateRecoveryCancel(v);validateBasicFollowup(v);validateBasicPursuit(v);validateUnitTargets(v);validateBasicCharge(v);shieldView(v.self.statuses,v.self.shield_total,v.self.max_hp,true);if(v.self.shield_total!==undefined||v.self.statuses.some(s=>s.attribute==='life_steal'||[...(s.modifiers??[]),...(s.base_modifiers??[])].some(m=>m.attribute==='life_steal'))){const ids=[];if(v.self.statuses.length>32)fail('INVALID_FRAME');for(const s of v.self.statuses){if(!id(s.instance_id)||!id(s.definition_id)||!same(s.target,v.self.participant.entity)||(!same(s.source,s.target)&&!v.monsters.some(n=>same(n.entity,s.source)||s.retained_source===true&&n.entity.id===s.source.id&&n.entity.generation>s.source.generation)&&!isMemberCombatStatus(s,v)&&!isAllySupportStatus(s,v.members)))fail('INVALID_FRAME');ids.push(s.instance_id);}unique(ids);}for(const kind of ['members','monsters','resources','projectiles'])listCheck(kind,v[kind]);validateExtraction(v);validateRelics(v);validateItemInstances(v);if(v.projectiles.length>64||v.projectiles.reduce((sum,r)=>sum+(r.piercing?.max_hits??0),0)>128)fail('INVALID_FRAME');for(const r of v.projectiles)piercing(r,v);validateBasicProjectileSegments(v);if(v.areas!==undefined)areas(v.areas,v);if(v.basic_waves!==undefined)basicWaves(v.basic_waves,v);else if(v.self.basic_wave_phase!==undefined)fail('INVALID_FRAME');if(v.basic_impacts!==undefined)basicImpacts(v.basic_impacts,v);if(v.basic_beams!==undefined)basicBeams(v.basic_beams,v);for(const n of v.monsters){shieldView(n.statuses??[],n.shield_total,n.max_hp,false);if(n.shield_total!==undefined&&n.statuses===undefined)fail('INVALID_FRAME');if(n.statuses!==undefined){if(!Array.isArray(n.statuses)||n.statuses.length>32)fail('INVALID_FRAME');const ids=[];for(const s of n.statuses){projectedStatus(s,n,v);ids.push(s.instance_id);}unique(ids);}}validateRecast(v);validateSummons(v);if(v.members.filter(m=>same(m.entity,v.self.participant.entity)).length!==1)fail('INVALID_FRAME');}
function applyList(kind,old,d){shape(d,['upsert','removed','order']);if(![d.upsert,d.removed,d.order].every(Array.isArray))fail('INVALID_FRAME');const up=d.upsert.map(v=>itemKey(kind,v)),removed=d.removed.map(v=>key(kind,v)),order=d.order.map(v=>key(kind,v));unique(up);unique(removed);unique(order);const values=new Map(old.map(v=>[itemKey(kind,v),v]));for(const k of removed){if(!values.has(k)||up.includes(k))fail('INVALID_FRAME');values.delete(k);}for(let i=0;i<up.length;i++)values.set(up[i],d.upsert[i]);if(values.size!==order.length||order.some(k=>!values.has(k)))fail('INVALID_FRAME');return order.map(k=>clone(values.get(k)));}
export class ReplicaV6 {
 constructor(keyValue,recipient){shape(keyValue,['id','incarnation']);if(!id(keyValue.id)||!uint(keyValue.incarnation,4294967295,1)||!id(recipient))fail('INVALID_FRAME');this.key=clone(keyValue);this.recipient=recipient;this._last=null;this._snapshot=null;this.lastFullId=null;this._memberConflict=null;}
 get snapshot(){return this._snapshot?clone(this._snapshot):null;}
 get epoch(){return this._last?.stream_epoch??null;}
 get revision(){return this._last?.revision??null;}
 forgetBaseline(){this._snapshot=null;this.lastFullId=null;}
 apply(e){
  shape(e,['key','publication_id','recipient_id','connection','stream_epoch','revision','payload']);if(!same(e.key,this.key)||e.recipient_id!==this.recipient)fail('WRONG_RECIPIENT');shape(e.connection,['id','generation']);if(!id(e.publication_id)||!id(e.connection.id)||!uint(e.connection.generation,4294967295,1)||!uint(e.stream_epoch,4294967295,1)||!uint(e.revision,Number.MAX_SAFE_INTEGER,1))fail('INVALID_FRAME');
  const p=e.payload;if(!object(p))fail('INVALID_FRAME');const full=p.kind==='full';if(full){shape(p,['kind','snapshot','reason']);if(!['initial','periodic','expired','failure','timeout','resync','lifecycle','terminal','size'].includes(p.reason))fail('INVALID_FRAME');}else if(p.kind==='delta'){shape(p,['kind','base_publication_id','base_revision','delta']);if(!id(p.base_publication_id)||!uint(p.base_revision,Number.MAX_SAFE_INTEGER,1))fail('INVALID_FRAME');}else fail('INVALID_FRAME');
  const old=this._last;if(old){if(e.connection.generation<old.connection.generation||(e.connection.generation===old.connection.generation&&(!same(e.connection,old.connection)||e.stream_epoch<old.stream_epoch))||e.revision<old.revision)fail('STALE_FRAME');if(e.revision===old.revision){if(!same(e,old))fail('CONFLICT');if(!this._snapshot)fail('BASELINE_MISSING');return this.snapshot;}if(e.publication_id===old.publication_id)fail('CONFLICT');if((!same(e.connection,old.connection)||e.stream_epoch!==old.stream_epoch)&&!full)fail('BASELINE_MISSING');}else if(!full)fail('BASELINE_MISSING');
  let next;if(full)next=clone(p.snapshot);else{if(!old||!this._snapshot||p.base_publication_id!==old.publication_id||p.base_revision!==old.revision)fail('BASELINE_MISSING');const d=p.delta;shape(d,['logical_ms'],['state','own','members','monsters','resources','projectiles','areas','basic_waves','basic_impacts','basic_beams','checkpoint_zones','active_blockers','containers','relics']);if(!uint(d.logical_ms)||Object.keys(d).some(k=>d[k]===null))fail('INVALID_FRAME');next=this.snapshot;next.logical_ms=d.logical_ms;if(d.areas!==undefined)next.areas=clone(d.areas);if(d.checkpoint_zones!==undefined)next.checkpoint_zones=clone(d.checkpoint_zones);if(d.active_blockers!==undefined)next.active_blockers=clone(d.active_blockers);if(d.containers!==undefined)next.containers=clone(d.containers);if(d.relics!==undefined)next.relics=clone(d.relics);if(d.basic_waves!==undefined)next.basic_waves=clone(d.basic_waves);if(d.basic_impacts!==undefined)next.basic_impacts=clone(d.basic_impacts);if(d.basic_beams!==undefined)next.basic_beams=clone(d.basic_beams);if(d.state!==undefined){state(d.state);Object.assign(next,clone(d.state));if(d.state.encounter_progress===undefined)delete next.encounter_progress;if(d.state.extraction_pressure===undefined)delete next.extraction_pressure;if(d.state.contested_equipment===undefined)delete next.contested_equipment;}if(d.own!==undefined){own(d.own,this.recipient);next.self=clone(d.own);}for(const kind of ['members','monsters','resources','projectiles'])if(d[kind]!==undefined)next[kind]=applyList(kind,next[kind],d[kind]);}
  snapshot(next,this.recipient);validateZoneExposure(next);validateContestedEquipment(next,next.self);const conflict=next.member_conflict===true;if(this._memberConflict!==null&&conflict!==this._memberConflict)fail('INVALID_FRAME');if(this._snapshot){contestedTransition(this._snapshot,next);equipmentTransition(this._snapshot.self,next.self);loadoutTransition(this._snapshot.self,next.self);reservationTransition(this._snapshot.self,next.self);channelTransition(this._snapshot.self,next.self);checkpointTransition(this._snapshot,next);modeTransition(this._snapshot,next);extractionTransition(this._snapshot,next);encounterTransition(this._snapshot.encounter_progress,next.encounter_progress,next.logical_ms);trainingTransition(this._snapshot.self,next.self);anchorTransition(this._snapshot,next);anchorFieldTransition(this._snapshot,next);bossPhaseTransition(this._snapshot,next);}if(this._snapshot&&next.logical_ms<this._snapshot.logical_ms)fail('STALE_FRAME');this._snapshot=next;this._memberConflict=conflict;this._last=clone(e);if(full)this.lastFullId=e.publication_id;return this.snapshot;
 }
}
export function envelopeFromOutput(o){const b=o.body;if(b.kind!=='publication_v6')fail('INVALID_FRAME');return {key:o.key,publication_id:o.id,recipient_id:b.recipient_id,connection:b.connection,stream_epoch:b.stream_epoch,revision:b.revision,payload:b.payload};}

function validateZoneExposure(snapshot) {
  const own=snapshot.self, view=own.zone_exposure, now=snapshot.logical_ms;
  if (view===undefined) return;
  shape(view,['band_id','stacks','max_stacks','nodes'],['boost_ends_at_ms']);
  const shortId=value=>id(value)&&new TextEncoder().encode(value).length<=64;
  const end=view.boost_ends_at_ms;
  if (own.phase!=='active'||own.hp===0||own.mode||own.training||!shortId(view.band_id)||
      !uint(view.max_stacks,64,1)||!uint(view.stacks,view.max_stacks)||
      end!==undefined&&(!uint(end)||end<=now||end-now>3600000)||
      !Array.isArray(view.nodes)||view.nodes.length>16) fail('INVALID_FRAME');
  for (const node of view.nodes) {
    shape(node,['id','position','revision','ready_at_ms']);
    if (!shortId(node.id)||!point(node.position)||!uint(node.revision,4294967295,1)||
        !uint(node.ready_at_ms)||(node.revision===1)!==(node.ready_at_ms===0)||
        node.ready_at_ms-now>3600000) fail('INVALID_FRAME');
  }
  unique(view.nodes.map(node=>node.id));
}

function validateContestedEquipment(snapshot, own) {
  const rows=snapshot.contested_equipment;
  const profile=own?.world_weapon_profile;
  const shortId=value=>id(value)&&new TextEncoder().encode(value).length<=64;
  if (rows===undefined) {
    if (profile!==undefined) fail('INVALID_FRAME');
    return;
  }
  if (!Array.isArray(rows)||!uint(rows.length,8,1)||own?.mode||own?.training||
      profile!==undefined&&!shortId(profile)) fail('INVALID_FRAME');
  for (const row of rows) {
    shape(row,['id','position','revision','state'],['holder']);
    if (!shortId(row.id)||!point(row.position)||!uint(row.revision,4294967295,1)||
        !['available','held','dropped','secured','lost'].includes(row.state)||
        (row.state==='available')!==(row.revision===1)||
        (row.state==='held')!==(row.holder!==undefined)) fail('INVALID_FRAME');
    if (row.holder!==undefined) {
      ref(row.holder);
      if (!snapshot.members.some(m=>same(m.entity,row.holder)&&m.phase==='active'&&m.hp>0)) fail('INVALID_FRAME');
    }
  }
  unique(rows.map(row=>row.id));
  if (own && (profile!==undefined)!==rows.some(row=>same(row.holder,own.participant.entity))) fail('INVALID_FRAME');
}

function contestedTransition(before, after) {
  const a=before.contested_equipment,b=after.contested_equipment;
  if (a===undefined&&b===undefined) return;
  if (a===undefined||b===undefined||a.length!==b.length) fail('INVALID_FRAME');
  for (let i=0;i<a.length;i++) {
    const x=a[i],y=b[i];
    if (!same(x,y)&&(x.id!==y.id||x.revision>=y.revision||['secured','lost'].includes(x.state))) fail('INVALID_FRAME');
  }
}

const spectatorLists=['members','monsters','projectiles','resources','areas'];
function spectatorSnapshot(s) {
  shape(s,['phase','logical_ms','active_at_ms','events',...spectatorLists],['stage','encounter_progress','extraction_pressure','contested_equipment']);
  if (!['active','closed'].includes(s.phase) || !uint(s.logical_ms) || !uint(s.active_at_ms,s.logical_ms)) fail('INVALID_FRAME');
  const memberPhases=['active','settling','terminal'];
  const hp=v=>uint(v.max_hp,4294967295,1)&&uint(v.hp,v.max_hp);
  const radius=v=>Number.isFinite(v.radius)&&v.radius>=0;
  for (const list of spectatorLists) {
    if (!Array.isArray(s[list])) fail('INVALID_FRAME');
    unique(s[list].map(v=>itemKey(list,v)));
  }
  if (!uint(s.members.length,8,1)) fail('INVALID_FRAME');
  for (const m of s.members) {
    shape(m,['entity','position','hp','max_hp','phase'],['outcome','extraction_channel']);
    if (m.entity.kind!=='participant'||!point(m.position)||!hp(m)||!memberPhases.includes(m.phase)||
        m.outcome!==undefined&&(s.stage?.objective!=='contest'||m.phase==='active'||!['win','loss','draw'].includes(m.outcome))) fail('INVALID_FRAME');
  }
  validateContestedEquipment(s);
  for (const m of s.monsters) {
    shape(m,['entity','template_id','position','hp','max_hp','radius'],['summon_owner']);
    if (m.entity.kind!=='monster'||!id(m.template_id)||!point(m.position)||!hp(m)||!radius(m)) fail('INVALID_FRAME');
    if (m.summon_owner!==undefined) {
      ref(m.summon_owner);
      if (!s.members.some(p=>same(p.entity,m.summon_owner))) fail('INVALID_FRAME');
    }
  }
  for (const p of s.projectiles) {
    shape(p,['id','source','position','radius']);
    ref(p.source);
    if (!point(p.position)||!radius(p)) fail('INVALID_FRAME');
  }
  for (const r of s.resources) {
    shape(r,['id','position','expires_at_ms']);
    if (!point(r.position)||!uint(r.expires_at_ms,Number.MAX_SAFE_INTEGER,s.logical_ms+1)) fail('INVALID_FRAME');
  }
  for (const a of s.areas) {
    shape(a,['id','center','radius']);
    if (!point(a.center)||!radius(a)) fail('INVALID_FRAME');
  }
  if (s.stage!==undefined) {
    const stage=s.stage;
    shape(stage,['id','index','objective','opens_at_ms','expires_at_ms'],['wave','progress']);
    if (!id(stage.id)||!uint(stage.index,8,1)||!modeObjectives.includes(stage.objective)||
        !uint(stage.opens_at_ms,s.logical_ms,s.active_at_ms)||!uint(stage.expires_at_ms,Number.MAX_SAFE_INTEGER,stage.opens_at_ms+1)) fail('INVALID_FRAME');
    validateModeWave(stage,s.logical_ms);
    validateModeProgress(stage);
  }
  validateEncounter(s);
  validateExtraction(s);
  if (!Array.isArray(s.events)||s.events.length>32) fail('INVALID_FRAME');
  let sequence=0, time=s.active_at_ms;
  for (const fact of s.events) {
    shape(fact,['sequence','logical_ms','event']);
    if (!uint(fact.sequence,Number.MAX_SAFE_INTEGER,sequence+1)||!uint(fact.logical_ms,s.logical_ms,time)) fail('INVALID_FRAME');
    sequence=fact.sequence; time=fact.logical_ms;
    const e=fact.event;
    if (e?.kind==='damage') {
      shape(e,['kind','source','target','amount','hp']);
      ref(e.source);ref(e.target);
      if (!uint(e.amount,4294967295)||!uint(e.hp,4294967295)) fail('INVALID_FRAME');
    } else if (e?.kind==='extraction_channel_changed') {
      shape(e,['kind','entity','channel','state']);
      ref(e.entity);
      extractionChannel(e.channel,s.active_at_ms);
      if (e.entity.kind!=='participant'||!['started','interrupted','completed'].includes(e.state)||
          fact.logical_ms<e.channel.started_at_ms||
          e.state==='started'&&fact.logical_ms!==e.channel.started_at_ms||
          e.state==='completed'&&fact.logical_ms<e.channel.ends_at_ms) fail('INVALID_FRAME');
    } else if (e?.kind==='member_changed'||e?.kind==='respawned') {
      shape(e,e.kind==='member_changed'?['kind','entity','phase']:['kind','entity']);
      ref(e.entity);
      if (e.entity.kind!=='participant'||e.phase!==undefined&&!memberPhases.includes(e.phase)) fail('INVALID_FRAME');
    } else fail('INVALID_FRAME');
  }
}

export class SpectatorReplicaV6 {
  #last=null;
  #snapshot=null;
  constructor(key,slot) { this.key=clone(key); this.slot=slot; }
  get snapshot(){return this.#snapshot&&clone(this.#snapshot);}
  forgetBaseline(){this.#snapshot=null;}
  apply(frame) {
    shape(frame,['key','publication_id','slot','connection','stream_epoch','revision','payload']);
    if (!same(frame.key,this.key)||frame.slot!==this.slot) fail('WRONG_RECIPIENT');
    shape(frame.connection,['id','generation']);
    if (!uint(frame.slot,8,1)||!id(frame.publication_id)||!id(frame.connection.id)||!uint(frame.connection.generation,4294967295,1)||
        !uint(frame.stream_epoch,4294967295,1)||!uint(frame.revision,Number.MAX_SAFE_INTEGER,1)) fail('INVALID_FRAME');
    const p=frame.payload, full=p?.kind==='full', old=this.#last;
    if (full) {
      shape(p,['kind','snapshot','reason']);
      if (!['initial','periodic','expired','failure','timeout','resync','lifecycle','terminal','size'].includes(p.reason)) fail('INVALID_FRAME');
    } else {
      shape(p,['kind','base_publication_id','base_revision','delta']);
      if (p.kind!=='delta') fail('INVALID_FRAME');
    }
    if (old) {
      if (frame.connection.generation<old.connection.generation||frame.revision<old.revision||
          frame.connection.generation===old.connection.generation&&(!same(frame.connection,old.connection)||frame.stream_epoch<old.stream_epoch)) fail('STALE_FRAME');
      if (frame.revision===old.revision) {
        if (!same(frame,old)) fail('CONFLICT');
        if (!this.#snapshot) fail('BASELINE_MISSING');
        return this.snapshot;
      }
      if (frame.publication_id===old.publication_id) fail('CONFLICT');
    }
    let next;
    if (full) next=clone(p.snapshot);
    else {
      if (!old||!this.#snapshot||!same(frame.connection,old.connection)||frame.stream_epoch!==old.stream_epoch||
          p.base_publication_id!==old.publication_id||p.base_revision!==old.revision) fail('BASELINE_MISSING');
      const d=p.delta;
      shape(d,['logical_ms'],[...spectatorLists,'stage','events','encounter_progress','extraction_pressure','contested_equipment']);
      next=this.snapshot;
      next.logical_ms=d.logical_ms;
      for (const list of spectatorLists) if (Object.hasOwn(d,list)) next[list]=applyList(list,next[list],d[list]);
      for (const field of ['stage','events','encounter_progress','extraction_pressure','contested_equipment']) if (Object.hasOwn(d,field)) next[field]=clone(d[field]);
    }
    spectatorSnapshot(next);
    const before=this.#snapshot;
    if (before) {
      if (next.logical_ms<before.logical_ms) fail('STALE_FRAME');
      if (next.active_at_ms!==before.active_at_ms||before.phase==='closed'&&!same(before,next)) fail('INVALID_FRAME');
      encounterTransition(before.encounter_progress,next.encounter_progress,next.logical_ms);
      contestedTransition(before,next);
      extractionTransition(before,next);
      const a=before.stage,b=next.stage;
      if ((a===undefined)!==(b===undefined)||a&&(b.index<a.index||b.opens_at_ms<a.opens_at_ms||
          b.index===a.index&&(a.id!==b.id||a.objective!==b.objective||a.opens_at_ms!==b.opens_at_ms||
            a.expires_at_ms!==b.expires_at_ms||(!waveTransition(a.wave,b.wave)||!quotaTransition(a.progress,b.progress))))) fail('INVALID_FRAME');
      if ((next.events.at(-1)?.sequence??0)<(before.events.at(-1)?.sequence??0)||
          next.events.some(e=>before.events.some(o=>e.sequence===o.sequence&&!same(e,o)))) fail('INVALID_FRAME');
    }
    this.#snapshot=next;
    this.#last=clone(frame);
    return this.snapshot;
  }
}
export function spectatorEnvelopeFromOutput(o) {
  const b=o.body;
  if (b.kind!=='spectator_publication_v6') fail('INVALID_FRAME');
  return {key:o.key,publication_id:o.id,slot:b.slot,connection:b.connection,stream_epoch:b.stream_epoch,revision:b.revision,payload:b.payload};
}
