// Pure observation of Rust plans. No target eligibility or damage decisions live here.
export const newCombatStats=()=>({unitCommits:0,unitProjectiles:0,targets:[]});
export function observeCombat(stats,wrapped,pid){
 const e=wrapped.kind==='private'?wrapped.event:wrapped;if(!e)return;
 if(e.kind==='action_committed'&&e.unit_target){stats.unitCommits++;return;}
 if(e.kind!=='projectile_started'||!e.projectile.unit_target||e.projectile.source.kind!=='participant'||e.projectile.source.id!==pid)return;
 const p=e.projectile,t=p.unit_target;if(typeof t.id!=='string'||!Number.isInteger(t.generation)||t.generation<1)throw new Error('INVALID_UNIT_PLAN');
 stats.unitProjectiles++;if(!stats.targets.some(v=>v.id===t.id&&v.generation===t.generation))stats.targets.push(structuredClone(t));stats.lastPlan=structuredClone(p);
}
