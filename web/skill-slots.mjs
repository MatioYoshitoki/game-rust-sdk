// Typed input intentions and observation of private Rust commits only.
export const newSkillStats=()=>({commits:{},total:0});
export function observeSkills(stats,wrapped){const e=wrapped.kind==='private'?wrapped.event:null;if(e?.kind==='action_committed'&&e.skill_id){stats.commits[e.skill_id]=(stats.commits[e.skill_id]??0)+1;stats.total++;}}
export function skillAction(skill,snapshot,point=null,ally=null){
 if(skill.mechanism==='self_status')return {kind:'cast_self',skill_id:skill.id};
 if(['ally_heal','ally_shield'].includes(skill.mechanism)){
   if(!ally)throw Object.assign(new Error('INVALID_TARGET'),{code:'INVALID_TARGET'});
   return {kind:'cast_unit',skill_id:skill.id,target:{id:ally.id,generation:ally.generation}};
 }
 const origin=snapshot.self.position,aim=point??origin;
 const targets=snapshot.monsters.filter(n=>n.hp>0&&n.summon===undefined&&
   (n.training_owner_id===undefined||n.training_owner_id===snapshot.self.participant?.entity.id));
 if(point&&snapshot.member_conflict===true){
   const self=snapshot.self.participant.entity.id;
   targets.push(...snapshot.members.filter(m=>m.entity.id!==self&&m.hp>0&&m.phase==='active'&&!m.protected));
 }
 targets.sort((a,b)=>Math.hypot(...a.position.map((v,i)=>v-aim[i]))-Math.hypot(...b.position.map((v,i)=>v-aim[i])));
 const target=targets[0];
 if(['unit_projectile','unit_effect'].includes(skill.mechanism)){if(!target)throw Object.assign(new Error('INVALID_TARGET'),{code:'INVALID_TARGET'});return {kind:'cast_unit',skill_id:skill.id,target:{id:target.entity.id,generation:target.entity.generation}};}
 const position=point??target?.position;if(!position)throw Object.assign(new Error('INVALID_TARGET'),{code:'INVALID_TARGET'});
 if(['ground_circle','ground_field'].includes(skill.mechanism))return {kind:'cast_ground',skill_id:skill.id,position:[...position]};
 if(['projectile','line_field','piercing_projectile'].includes(skill.mechanism))return {kind:'cast',skill_id:skill.id,direction:position.map((v,i)=>v-origin[i])};
 throw Object.assign(new Error('UNSUPPORTED'),{code:'UNSUPPORTED'});
}
