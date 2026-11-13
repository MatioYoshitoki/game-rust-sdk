// Observation of actual private Rust costs. No fee calculation or gameplay mutation.
export const newHpCostStats=()=>({commits:0,total:0,bySkill:{}});
export function observeHpCosts(stats,wrapped){
 const e=wrapped.kind==='private'?wrapped.event:null;
 if(e?.kind!=='action_committed'||!Object.hasOwn(e,'hp_cost'))return;
 if(!e.skill||typeof e.skill_id!=='string'||!e.skill_id||!Number.isInteger(e.hp_cost)||e.hp_cost<0||e.hp_cost>4294967295)throw Object.assign(new Error('INVALID_COST_FACT'),{code:'INVALID_COST_FACT'});
 const row=stats.bySkill[e.skill_id]??{commits:0,total:0,last:0};
 stats.commits++;stats.total+=e.hp_cost;row.commits++;row.total+=e.hp_cost;row.last=e.hp_cost;stats.bySkill[e.skill_id]=row;
}
