// Display observation only. Every amount and instance comes from the Rust service.
export const newShieldStats=()=>({grants:0,granted:0,absorptions:0,visibleAbsorbed:0,ownAbsorptions:0,npcAbsorptions:0,dotAbsorbed:0,ownGrants:0,outgoingAllyGrants:0,npcGrants:0});
export function observeShields(stats,wrapped,pid){
 const e=wrapped.kind==='private'?wrapped.event:wrapped;if(!e)return;
 if(e.kind==='periodic_tick'){stats.dotAbsorbed+=e.absorbed??0;return;}
 if(!['shield_granted','shield_absorbed'].includes(e.kind))return;
 const outgoing = e.kind==='shield_granted' && e.target.kind==='participant' &&
  e.target.id!==pid && e.source.kind==='participant' && e.source.id===pid;
 if(e.target.kind==='participant'&&e.target.id!==pid&&!outgoing||
    e.kind==='shield_granted'&&e.target.kind==='monster'&&e.source.kind==='participant'&&e.source.id!==pid)throw new Error('WRONG_SHIELD_RECIPIENT');
 if(e.kind==='shield_granted'){
  stats.grants++;stats.granted+=e.granted;
  if(outgoing)stats.outgoingAllyGrants++;
  else if(e.target.kind==='participant')stats.ownGrants++;
  else stats.npcGrants++;
 }
 else {if(e.amount!==e.consumptions.reduce((a,s)=>a+s.absorbed,0))throw new Error('INVALID_SHIELD_SLICE');stats.absorptions++;stats.visibleAbsorbed+=e.amount;if(e.target.kind==='participant')stats.ownAbsorptions++;else stats.npcAbsorptions++;}
 stats.last=structuredClone(e);
}
