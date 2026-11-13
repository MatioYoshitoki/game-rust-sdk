// Observation of published Rust plans only; no timer, geometry or damage decisions.
export const newAreaStats=()=>({ownStarted:0,ownTicks:0,ownContacts:0,ownEnded:0,circles:0,fields:0,owners:{},endedReasons:{}});
const remember=(s,p)=>{s.owners[p.id]=p.source.id;while(Object.keys(s.owners).length>16)delete s.owners[Object.keys(s.owners)[0]];};
export function observeAreaSnapshot(s,snapshot){for(const p of snapshot.areas??[])remember(s,p);}
export function observeAreas(s,e,pid){if(e.kind==='area_started'){remember(s,e.area);if(e.area.source.id===pid){s.ownStarted++;if(e.area.mechanism==='line_field')s.lines=(s.lines??0)+1;else s[e.area.mechanism==='circle'?'circles':'fields']++;}s.last=structuredClone(e);}
else if(e.kind==='area_tick'&&s.owners[e.area_id]===pid){s.ownTicks++;s.ownContacts+=e.targets.length;s.last=structuredClone(e);}
else if(e.kind==='area_ended'){if(s.owners[e.area_id]===pid)s.ownEnded++;delete s.owners[e.area_id];s.endedReasons[e.area_id]=e.reason;while(Object.keys(s.endedReasons).length>16)delete s.endedReasons[Object.keys(s.endedReasons)[0]];}}
