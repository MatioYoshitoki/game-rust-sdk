// Ordinary consumer: exercise configured slots through the public input and publication path.
import assert from 'node:assert/strict';
import {skillAction} from '../../web/skill-slots.mjs';
import {delay} from './two-member-host.mjs';
export async function probeSkillSlots(host,{skip=[]}={}){
 const skills=[host.record.config.skill,...host.record.config.additional_skills];
 for(const skill of skills.filter(s=>!skip.includes(s.id))){
  if(['ground_circle','ground_field','line_field'].includes(skill.mechanism))await host.until(()=>host.members.find(m=>m.snapshot.self.phase==='active').snapshot.areas.length===0);
  const baseline=new Map(host.members.map(m=>[m.account.participant_id,m.skillStats.commits[skill.id]??0]));const end=Date.now()+15000;
  while(host.members.some(m=>(m.skillStats.commits[skill.id]??0)===baseline.get(m.account.participant_id))){
   assert.ok(Date.now()<end,'SLOT_PROBE_DEADLINE '+skill.id);
   for(const m of host.members){const s=m.snapshot;if((m.skillStats.commits[skill.id]??0)>baseline.get(m.account.participant_id)||s.self.pending_action||s.logical_ms<Math.max(s.self.recovery_until_ms,s.self.player_skills.find(x=>x.skill_id===skill.id).ready_at_ms))continue;
    const p=s.self.position,t=s.monsters.find(n=>n.hp>0)?.position??[p[0]-1,p[1]],dx=p[0]-t[0],dy=p[1]-t[1],length=Math.hypot(dx,dy)||1;
    const point=['ground_circle','ground_field'].includes(skill.mechanism)?p:[p[0]+dx/length*100,p[1]+dy/length*100];
    try{await host.input(m.account.participant_id,skillAction(skill,s,point));}catch(e){if(!['INVALID_STATE','CAPACITY'].includes(e.code))throw e;}
   }
   await host.pump();await delay(30);
  }
  await host.until(()=>host.members.every(m=>m.snapshot.self.pending_action===null&&m.snapshot.logical_ms>=m.snapshot.self.recovery_until_ms));
 }
}
