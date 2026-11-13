// Observe authoritative Rust flight facts. No collision, clock or damage decisions.
export const newFlightStats=()=>({ownStarted:0,ownContacts:0,ownEnded:0,once:0,repeating:0,owners:{},endedReasons:{}});
const remember=(s,p)=>{if(!p.piercing)return;s.owners[p.id]=p.source.id;while(Object.keys(s.owners).length>128)delete s.owners[Object.keys(s.owners)[0]];};
export function observeFlightSnapshot(s,snapshot){for(const p of snapshot.projectiles)remember(s,p);}
export function observeFlights(s,e,pid){
 if(e.kind==='projectile_started'&&e.projectile.piercing){const p=e.projectile;remember(s,p);if(p.source.id===pid){s.ownStarted++;s[p.piercing.repeat_interval_ms===0?'once':'repeating']++;s.lastPlan=structuredClone(p);}}
 else if(e.kind==='projectile_contact'&&e.source.id===pid){s.ownContacts++;s.lastContact=structuredClone(e);}
 else if(e.kind==='projectile_ended'&&e.reason){if(s.owners[e.projectile_id]===pid)s.ownEnded++;delete s.owners[e.projectile_id];s.endedReasons[e.projectile_id]=e.reason;while(Object.keys(s.endedReasons).length>128)delete s.endedReasons[Object.keys(s.endedReasons)[0]];}
}
