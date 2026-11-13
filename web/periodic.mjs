// Observation only: all amounts, identities and scheduled times come from Rust events.
export const newPeriodicStats=()=>({dotTicks:0,hotTicks:0,outgoingDotTicks:0,incomingDotTicks:0,selfHotTicks:0,outgoingAllyHotTicks:0,incomingAllyHotTicks:0,npcHotTicks:0,hpDamage:0,hpRestored:0,statusApplied:0,statusRefreshed:0,statusRemoved:0});
export function observePeriodic(stats,wrapped,pid){
 const e=wrapped.kind==='private'?wrapped.event:wrapped;if(!e)return;
 if(['status_applied','status_refreshed'].includes(e.kind)&&e.status.source.id===pid&&e.status.target.id===pid&&e.status.periodic?.kind==='max_hp_heal')stats.selfHotIdentity=structuredClone(e.status);
 if(e.kind==='status_applied')stats.statusApplied++;
 if(e.kind==='status_refreshed')stats.statusRefreshed++;
 if(e.kind==='status_removed')stats.statusRemoved+=e.instance_ids.length;
 if(!['periodic_tick','periodic_heal_tick'].includes(e.kind))return;
 const own=e.source.kind==='participant'&&e.source.id===pid||e.target.kind==='participant'&&e.target.id===pid;
 const npcSelf=e.source.kind==='monster'&&e.target.kind==='monster'&&e.source.id===e.target.id&&e.source.generation===e.target.generation;
 if(!own&&!npcSelf)throw new Error('WRONG_PERIODIC_RECIPIENT');
 if(e.kind==='periodic_tick'){stats.dotTicks++;stats.hpDamage+=e.hp_damage;if(e.source.kind==='participant')stats.outgoingDotTicks++;else stats.incomingDotTicks++;}
 else{
  stats.hotTicks++;stats.hpRestored+=e.hp_restored;
  if(npcSelf)stats.npcHotTicks++;
  else if(e.source.kind==='participant'&&e.target.kind==='participant'&&e.source.id!==e.target.id){
   if(e.source.id===pid)stats.outgoingAllyHotTicks++;
   else stats.incomingAllyHotTicks++;
  }else stats.selfHotTicks++;
 }
 stats.last=structuredClone(e);
}
