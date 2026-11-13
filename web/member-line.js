import {newAreaStats,observeAreas,observeAreaSnapshot} from './areas.mjs';
import {newCombatStats,observeCombat} from './combat.mjs';
import {newShieldStats,observeShields} from './shields.mjs';
import {newPeriodicStats,observePeriodic} from './periodic.mjs';
import {ReplicaV6} from './replication.mjs';
const $=id=>document.getElementById(id),q=new URLSearchParams(location.search),room=q.get('room'),seat=q.get('seat'),token=q.get('token');
let replica=null,resyncBusy=false,statistics={full:0,delta:0,bytes:0,fullEquivalentBytes:0,resyncs:0,stale:0},snapshot=null,configuration=null,stream=null,readyBusy=false,dirty=true,aim=null,connecting=false;
const periodic=newPeriodicStats(),shields=newShieldStats(),combat=newCombatStats(),areaStats=newAreaStats();let selectingGround=false;
const labels={preparing:'等待准入',deploying:'等待共同就绪',active:'活动中',settling:'结算核对中',terminal:'成员已结束',closed:'房间已关闭',faulted:'房间已封存'};
const auth=new URLSearchParams({room:room??'',seat:seat??'',token:token??''});
async function api(path,body){const r=await fetch(path,{method:body?'POST':'GET',headers:body?{'content-type':'application/json'}:{},body:body?JSON.stringify(body):undefined});const data=await r.json();if(!r.ok)throw new Error(data.error);return data;}
function problem(e){$('problem').textContent=e.message??String(e);}
async function input(action,{quiet=false}={}){try{const result=await api('/api/input?'+auth,action);if(!quiet)$('problem').textContent='';return result;}catch(e){if(!quiet)problem(e);throw e;}}
function showPair(data){$('launcher').hidden=false;$('pairTitle').hidden=false;$('closeRoom').hidden=false;$('links').replaceChildren();$('pair').replaceChildren();
  for(const link of data.links){const a=document.createElement('a');a.href=link.url;a.target='_blank';a.rel='noopener';a.textContent=`单独打开成员 ${link.participant_id}`;$('links').append(a,document.createElement('br'));const frame=document.createElement('iframe');frame.src=link.url;frame.title=`成员 ${link.participant_id}`;$('pair').append(frame);}
  $('closeRoom').onclick=()=>api(`/api/close?room=${encodeURIComponent(data.room_id??room)}&owner=${encodeURIComponent(data.owner_token??q.get('owner'))}`,{}).catch(problem);
}
async function launch(){
  const data=await api('/api/config');$('version').textContent=`SDK ${data.sdk.crateVersion} · 配置化成员`;
  if(!seat){$('launcher').hidden=false;for(const s of data.scenarios){const o=document.createElement('option');o.value=s.id;o.textContent=s.name;$('scenario').append(o);}
    $('create').onclick=async()=>{try{$('create').disabled=true;const result=await api('/api/rooms',{scenario:$('scenario').value});showPair(result);}catch(e){problem(e)}finally{$('create').disabled=false;}};
    if(room&&q.get('owner')){const links=await api(`/api/links?room=${room}&owner=${q.get('owner')}`);showPair({...links,room_id:room});}return;
  }
  $('game').hidden=false;$('identity').textContent=`成员 ${seat} · 房间 ${room.slice(0,8)}`;
  const view=await api('/api/view?'+auth);replica=new ReplicaV6(view.match_key,seat);configuration=view.configuration;bakeTerrain();updateLedger(view.ledger);$('phase').textContent=labels[view.phase]??view.phase;if(view.snapshot){snapshot=view.snapshot;renderText();}
  connect();setInterval(async()=>{try{const view=await api('/api/ledger?'+auth);updateLedger(view.ledger);if(view.errors.length)problem(view.errors.at(-1));}catch{}},800);
}
function connect(){
  connecting=true;if(snapshot)renderText();stream?.close();stream=new EventSource('/api/events?'+auth);
  stream.addEventListener('snapshot',async e=>{
    const frame=JSON.parse(e.data);statistics[frame.payload.kind]++;statistics.bytes+=new TextEncoder().encode(e.data).length;
    try {snapshot=replica.apply(frame);}
    catch(error){if(error.code==='STALE_FRAME'){statistics.stale++;window.__replicationStats=structuredClone(statistics);return;}if(error.code==='BASELINE_MISSING'){connecting=true;if(snapshot)renderText();if(!resyncBusy){resyncBusy=true;statistics.resyncs++;try{await input({kind:'request_full',stream_epoch:frame.stream_epoch},{quiet:true});}catch(e){if(e.message!=='STALE_PUBLICATION')problem(e);}finally{resyncBusy=false;}}return;}problem(error);return;}
    statistics.fullEquivalentBytes+=new TextEncoder().encode(JSON.stringify({...frame,payload:{kind:'full',snapshot,reason:'initial'}})).length;
    window.__replicationStats=structuredClone(statistics);window.__lastFrame=structuredClone(frame);
    connecting=false;window.__lastConnection=structuredClone(frame.connection);window.__lastSnapshot=structuredClone(snapshot);window.__lastPublication=frame.publication_id;renderText();dirty=true;
    if($('replicationStats'))$('replicationStats').textContent=`全量 ${statistics.full} · 增量 ${statistics.delta} · JSON ${(statistics.bytes/1024).toFixed(1)} KiB · 重同步 ${statistics.resyncs}`;
    if(!snapshot.self.ready&&['active','deploying'].includes(snapshot.phase)&&['active','deploying'].includes(snapshot.self.phase)&&document.visibilityState==='visible'&&!readyBusy){
      readyBusy=true;try{if(!snapshot.self.foreground)await input({kind:'foreground'},{quiet:true});await input({kind:'combat_ready',publication_id:replica.lastFullId},{quiet:true});}catch{}finally{readyBusy=false;}
    }
  });
  stream.addEventListener('gameplay',e=>{const event=JSON.parse(e.data);observePeriodic(periodic,event,seat);observeShields(shields,event,seat);observeCombat(combat,event,seat);observeAreas(areaStats,event,seat);window.__areaStats=structuredClone(areaStats);window.__combatStats=structuredClone(combat);window.__shieldStats=structuredClone(shields);window.__periodicStats=structuredClone(periodic);if($('periodicStats'))$('periodicStats').textContent=`DOT ${periodic.dotTicks} 跳（发出 ${periodic.outgoingDotTicks} / 受击 ${periodic.incomingDotTicks}） · HOT ${periodic.hotTicks} 跳（自身 ${periodic.selfHotTicks} / NPC ${periodic.npcHotTicks}） · 实际治疗 ${periodic.hpRestored}`;window.__lastEvent=event;if(event.kind==='problem')problem(event.code);if(event.kind==='publication_expired'){statistics.expired=(statistics.expired??0)+1;connecting=true;$('resetCache').disabled=true;problem('显示同步已到期，结算以本人成员账户为准');}if(event.kind==='closed')$('phase').textContent='房间已关闭';});
  stream.onerror=()=>{$('phase').textContent='连接中断，正在恢复';};
}
function updateLedger(v){if(!v)return;$('ledger').textContent=`本人成员账户\n准入 ${v.admissions} 次 · 结算 ${v.settlements} 次\n补偿 ${v.compensations} 次\n`+Object.entries(v.balances??{}).map(([k,v])=>`${k}: ${v}`).join('\n');}
function renderText(){const s=snapshot.self;observeAreaSnapshot(areaStats,snapshot);$('phase').textContent=labels[snapshot.phase]??snapshot.phase;$('own').textContent=`${seat} · ${labels[s.phase]??s.phase}\nHP ${s.hp} / ${s.max_hp}\n灵力 ${s.spirit} / ${s.max_spirit}\n攻击 ${s.effective_atk} · 防御 ${s.effective_def}\n逻辑时间 ${(snapshot.logical_ms/1000).toFixed(2)}s\n${s.protected?'保护中':'可操作'} · ${s.ready?'已就绪':'未就绪'}\n状态 ${s.statuses.length}`;
  $('own').textContent+=`\n准备技能：${s.pending_action?.skill_id??'无'}`;
  $('own').textContent+=`\n范围提交 ${areaStats.ownStarted} · 实际接触 ${areaStats.ownContacts}\n`+(snapshot.areas??[]).map(a=>`${a.source.id} ${a.mechanism==='circle'?'单次':'持续'} · ${a.completed_ticks}/${a.total_ticks} 跳 · 剩余接触 ${a.contact_policy?.remaining_hits??'单次'}`).join('\n');
  $('members').textContent='共享成员\n'+snapshot.members.map(m=>`${m.entity.id}: HP ${m.hp} · ${labels[m.phase]??m.phase}`).join('\n');
  $('own').textContent+=`\n本人护盾 ${s.shield_total??0} · 可见吸收 ${shields.visibleAbsorbed} · DOT 实际吸收 ${shields.dotAbsorbed}\nNPC 总盾：${snapshot.monsters.map(n=>n.entity.id+': '+n.shield_total).join(' / ')}`;
  $('own').textContent+='\n本人周期\n'+s.statuses.filter(v=>v.periodic).map(v=>`${v.definition_id}: ${v.source.id} → ${v.target.id} · ${v.periodic.ticks_done}/${v.periodic.total_ticks}`).join('\n')+'\nNPC 上本人成员来源 / NPC 自身状态\n'+snapshot.monsters.flatMap(n=>(n.statuses??[]).filter(v=>v.periodic).map(v=>`${n.entity.id}: ${v.source.id} · ${v.definition_id} ${v.periodic.ticks_done}/${v.periodic.total_ticks}`)).join('\n');
  $('inventory').textContent='本人背包\n'+s.inventory.map(i=>`${i.item_id}: ${i.quantity}`).join('\n');
  $('resetCache').disabled=s.phase==='terminal'||snapshot.phase==='closed'||!!statistics.expired;
  $('reconnect').disabled=s.phase==='terminal'||snapshot.phase==='closed'||!!statistics.expired;
  const active=!connecting&&snapshot.phase==='active'&&s.phase==='active'&&s.ready&&!s.protected;for(const id of ['attack','cast','selfCast','cancel','pickup','toExit','extract','stop','leave'])$(id).disabled=!active;
  $('cast').disabled=!active||!configuration.skills.some(s=>['line_field'].includes(s.mechanism));$('selfCast').disabled=!active||!configuration.skills.some(s=>s.mechanism==='self_status');
}
function nearest(){return snapshot?.monsters.filter(n=>n.hp>0).sort((a,b)=>Math.hypot(...a.position.map((v,i)=>v-snapshot.self.position[i]))-Math.hypot(...b.position.map((v,i)=>v-snapshot.self.position[i])))[0];}
$('attack').onclick=()=>input({kind:'attack'}).catch(()=>{});
$('selectGround').onclick=()=>{selectingGround=true;$('groundPoint').textContent='请在地图上选择方向点';};
$('cast').onclick=()=>{const position=aim??nearest()?.position;if(position)input({kind:'cast',skill_id:configuration.skills.find(s=>s.mechanism==='line_field').id,direction:position.map((v,i)=>v-snapshot.self.position[i])}).catch(()=>{});};
$('selfCast').onclick=()=>input({kind:'cast_self',skill_id:configuration.skills.find(s=>s.mechanism==='self_status').id}).catch(()=>{});
$('cancel').onclick=()=>input({kind:'cancel_action'}).catch(()=>{});$('stop').onclick=()=>input({kind:'stop'}).catch(()=>{});
$('toExit').onclick=()=>input({kind:'navigate',destination:snapshot.extraction_point}).catch(()=>{});
$('resetCache').onclick=()=>{replica.forgetBaseline();connecting=true;if(snapshot)renderText();};
$('extract').onclick=()=>input({kind:'extract_start'}).catch(()=>{});$('leave').onclick=()=>input({kind:'leave'}).catch(()=>{});$('reconnect').onclick=connect;
$('pickup').onclick=()=>{const r=snapshot.resources.filter(r=>!r.owner||r.owner.id===seat).sort((a,b)=>Math.hypot(...a.position.map((v,i)=>v-snapshot.self.position[i]))-Math.hypot(...b.position.map((v,i)=>v-snapshot.self.position[i])))[0];if(r)input({kind:'pickup',resource_id:r.id,expected_revision:snapshot.self.inventory_revision}).catch(()=>{});};
const keys=new Set(),directions={w:[-1,-1],s:[1,1],a:[-1,1],d:[1,-1],ArrowUp:[-1,-1],ArrowDown:[1,1],ArrowLeft:[-1,1],ArrowRight:[1,-1]};
function movement(){if(connecting||!snapshot?.self.ready)return;let direction=[0,0];for(const k of keys){direction[0]+=directions[k][0];direction[1]+=directions[k][1];}input({kind:'move',direction}).catch(()=>{});}
addEventListener('keydown',e=>{const k=directions[e.key]?e.key:e.key.toLowerCase();if(!directions[k]||keys.has(k))return;e.preventDefault();keys.add(k);movement();});
addEventListener('keyup',e=>{const k=directions[e.key]?e.key:e.key.toLowerCase();if(keys.delete(k)){e.preventDefault();movement();}});
document.addEventListener('visibilitychange',()=>{keys.clear();if(!snapshot||!['active','deploying'].includes(snapshot.phase))return;input({kind:document.hidden?'background':'foreground'},{quiet:true}).catch(()=>{});});

const canvas=$('map'),ctx=canvas.getContext('2d');let terrain=null,origin=[0,0];const project=([x,y])=>[(x-y)/2,(x+y)/4];
function bakeTerrain(){const p=configuration.pve,s=p.cell_size;terrain=document.createElement('canvas');terrain.width=(p.width+p.height)*s/2+64;terrain.height=(p.width+p.height)*s/4+64;origin=[p.height*s/2+32,32];const t=terrain.getContext('2d');let at=0;
  for(const [index,count] of p.cells_rle)for(let n=0;n<count;n++,at++){const x=at%p.width*s,y=Math.floor(at/p.width)*s,[a,b]=project([x,y]);t.beginPath();t.moveTo(a+origin[0],b+origin[1]);t.lineTo(a+s/2+origin[0],b+s/4+origin[1]);t.lineTo(a+origin[0],b+s/2+origin[1]);t.lineTo(a-s/2+origin[0],b+s/4+origin[1]);t.closePath();t.fillStyle=p.palette[index].walkable?'#243d3e':'#344854';t.fill();t.strokeStyle='#2e4b48';t.lineWidth=.6;t.stroke();}
  dirty=true;
}
function geometry(){const r=canvas.getBoundingClientRect(),center=project(snapshot?.self.position??configuration?.spawn??[0,0]);return {w:r.width,h:r.height,ox:r.width/2-center[0],oy:r.height/2-center[1]};}
function draw(){requestAnimationFrame(draw);if(!dirty||!snapshot||!terrain)return;dirty=false;const g=geometry(),d=devicePixelRatio||1;if(canvas.width!==Math.round(g.w*d)||canvas.height!==Math.round(g.h*d)){canvas.width=Math.round(g.w*d);canvas.height=Math.round(g.h*d);}ctx.setTransform(d,0,0,d,0,0);ctx.clearRect(0,0,g.w,g.h);ctx.drawImage(terrain,g.ox-origin[0],g.oy-origin[1]);
  const point=(position,color,r,label)=>{const [x,y]=project(position);ctx.beginPath();ctx.ellipse(x+g.ox,y+g.oy,r,r*.55,0,0,Math.PI*2);ctx.fillStyle=color;ctx.fill();if(label){ctx.font='12px system-ui';ctx.fillStyle='#e7f6f6';ctx.textAlign='center';ctx.fillText(label,x+g.ox,y+g.oy-14);}};
  for(const a of snapshot.areas??[]){const warning=snapshot.logical_ms<a.activates_at_ms;ctx.beginPath();
    if(a.line){const {start,end,width}=a.line,angle=Math.atan2(end[1]-start[1],end[0]-start[0]);let first=true;for(const [center,from]of[[end,angle-Math.PI/2],[start,angle+Math.PI/2]])for(let k=0;k<=16;k++){const t=from+k*Math.PI/16,[x,y]=project([center[0]+width/2*Math.cos(t),center[1]+width/2*Math.sin(t)]);if(first){ctx.moveTo(x+g.ox,y+g.oy);first=false;}else ctx.lineTo(x+g.ox,y+g.oy);}ctx.closePath();}
    else{const [x,y]=project(a.center);ctx.ellipse(x+g.ox,y+g.oy,a.radius/Math.SQRT2,a.radius/(2*Math.SQRT2),0,0,Math.PI*2);}
    ctx.fillStyle=warning?'#efbb5030':'#65dac530';ctx.fill();ctx.strokeStyle=warning?'#efbb50':'#65dac5';ctx.stroke();window.__areaRendered={id:a.id,line:structuredClone(a.line),warning};}

  point(snapshot.extraction_point,'#68bdab',20,'撤离点');for(const r of snapshot.resources)point(r.position,r.item_id?'#e9c967':'#93d9f2',5,r.item_id?`${r.quantity}`:'光球');
  const actors=[...snapshot.monsters.filter(m=>m.hp>0).map(m=>({p:m.position,color:'#dd806c',label:`${m.entity.id} ${m.hp}`})),...snapshot.members.map(m=>({p:m.position,color:m.entity.id===seat?'#84d4cf':'#80acd7',label:`${m.entity.id} ${m.hp}`}))];actors.sort((a,b)=>a.p[0]+a.p[1]-b.p[0]-b.p[1]);for(const a of actors)point(a.p,a.color,10,a.label);for(const p of snapshot.projectiles)point(p.position,p.source.kind==='participant'?'#e5d197':'#e29ea9',4);
}
canvas.addEventListener('pointerdown',e=>{if(connecting||!snapshot?.self.ready)return;const r=canvas.getBoundingClientRect(),g=geometry(),dx=e.clientX-r.left-g.ox,dy=e.clientY-r.top-g.oy;const destination=[dx+2*dy,2*dy-dx];if(selectingGround){aim=destination;selectingGround=false;$('groundPoint').textContent=`方向点 ${aim.map(n=>n.toFixed(0)).join(', ')}`;return;}input({kind:'navigate',destination}).catch(()=>{});});
new ResizeObserver(()=>dirty=true).observe(canvas);requestAnimationFrame(draw);launch().catch(problem);
