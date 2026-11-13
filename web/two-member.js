const $=id=>document.getElementById(id),q=new URLSearchParams(location.search),room=q.get('room'),seat=q.get('seat'),token=q.get('token');
let snapshot=null,configuration=null,stream=null,readyBusy=false,dirty=true,aim=null,connecting=false;
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
  const data=await api('/api/config');$('version').textContent=`SDK ${data.sdk.crateVersion} · 协议 4`;
  if(!seat){$('launcher').hidden=false;for(const s of data.scenarios){const o=document.createElement('option');o.value=s.id;o.textContent=s.name;$('scenario').append(o);}
    $('create').onclick=async()=>{try{$('create').disabled=true;const result=await api('/api/rooms',{scenario:$('scenario').value});showPair(result);}catch(e){problem(e)}finally{$('create').disabled=false;}};
    if(room&&q.get('owner')){const links=await api(`/api/links?room=${room}&owner=${q.get('owner')}`);showPair({...links,room_id:room});}return;
  }
  $('game').hidden=false;$('identity').textContent=`成员 ${seat} · 房间 ${room.slice(0,8)}`;
  const view=await api('/api/view?'+auth);configuration=view.configuration;bakeTerrain();updateLedger(view.ledger);$('phase').textContent=labels[view.phase]??view.phase;if(view.snapshot){snapshot=view.snapshot;renderText();}
  connect();setInterval(async()=>{try{const view=await api('/api/view?'+auth);updateLedger(view.ledger);if(view.errors.length)problem(view.errors.at(-1));}catch{}},800);
}
function connect(){
  connecting=true;if(snapshot)renderText();stream?.close();stream=new EventSource('/api/events?'+auth);
  stream.addEventListener('snapshot',async e=>{
    const frame=JSON.parse(e.data);if(snapshot&&frame.snapshot.logical_ms<snapshot.logical_ms)return;
    connecting=false;snapshot=frame.snapshot;window.__lastConnection=structuredClone(frame.connection);window.__lastSnapshot=structuredClone(snapshot);window.__lastPublication=frame.publication_id;renderText();dirty=true;
    if(!snapshot.self.ready&&['active','deploying'].includes(snapshot.phase)&&['active','deploying'].includes(snapshot.self.phase)&&document.visibilityState==='visible'&&!readyBusy){
      readyBusy=true;try{if(!snapshot.self.foreground)await input({kind:'foreground'},{quiet:true});await input({kind:'combat_ready',publication_id:frame.publication_id},{quiet:true});}catch{}finally{readyBusy=false;}
    }
  });
  stream.addEventListener('gameplay',e=>{const event=JSON.parse(e.data);window.__lastEvent=event;if(event.kind==='problem')problem(event.code);if(event.kind==='closed')$('phase').textContent='房间已关闭';});
  stream.onerror=()=>{$('phase').textContent='连接中断，正在恢复';};
}
function updateLedger(v){if(!v)return;$('ledger').textContent=`本人成员账户\n准入 ${v.admissions} 次 · 结算 ${v.settlements} 次\n补偿 ${v.compensations} 次\n`+Object.entries(v.balances??{}).map(([k,v])=>`${k}: ${v}`).join('\n');}
function renderText(){const s=snapshot.self;$('phase').textContent=labels[snapshot.phase]??snapshot.phase;$('own').textContent=`${seat} · ${labels[s.phase]??s.phase}\nHP ${s.hp} / ${s.max_hp}\n灵力 ${s.spirit} / ${s.max_spirit}\n攻击 ${s.effective_atk} · 防御 ${s.effective_def}\n逻辑时间 ${(snapshot.logical_ms/1000).toFixed(2)}s\n${s.protected?'保护中':'可操作'} · ${s.ready?'已就绪':'未就绪'}\n状态 ${s.statuses.length}`;
  $('members').textContent='共享成员\n'+snapshot.members.map(m=>`${m.entity.id}: HP ${m.hp} · ${labels[m.phase]??m.phase}`).join('\n');
  $('inventory').textContent='本人背包\n'+s.inventory.map(i=>`${i.item_id}: ${i.quantity}`).join('\n');
  const active=!connecting&&snapshot.phase==='active'&&s.phase==='active'&&s.ready&&!s.protected;for(const id of ['attack','cast','selfCast','cancel','pickup','toExit','extract','stop','leave'])$(id).disabled=!active;
  $('cast').disabled=!active||!configuration.skills.some(s=>s.mechanism==='projectile');$('selfCast').disabled=!active||!configuration.skills.some(s=>s.mechanism==='self_status');
}
function nearest(){return snapshot?.monsters.filter(n=>n.hp>0).sort((a,b)=>Math.hypot(...a.position.map((v,i)=>v-snapshot.self.position[i]))-Math.hypot(...b.position.map((v,i)=>v-snapshot.self.position[i])))[0];}
$('attack').onclick=()=>input({kind:'attack'}).catch(()=>{});
$('cast').onclick=()=>{const n=nearest(),p=aim??n?.position??[snapshot.self.position[0]+100,snapshot.self.position[1]];input({kind:'cast',skill_id:configuration.skills.find(s=>s.mechanism==='projectile').id,direction:p.map((v,i)=>v-snapshot.self.position[i])}).catch(()=>{});};
$('selfCast').onclick=()=>input({kind:'cast_self',skill_id:configuration.skills.find(s=>s.mechanism==='self_status').id}).catch(()=>{});
$('cancel').onclick=()=>input({kind:'cancel_action'}).catch(()=>{});$('stop').onclick=()=>input({kind:'stop'}).catch(()=>{});
$('toExit').onclick=()=>input({kind:'navigate',destination:snapshot.extraction_point}).catch(()=>{});
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
  point(snapshot.extraction_point,'#68bdab',20,'撤离点');for(const r of snapshot.resources)point(r.position,r.item_id?'#e9c967':'#93d9f2',5,r.item_id?`${r.quantity}`:'光球');
  const actors=[...snapshot.monsters.filter(m=>m.hp>0).map(m=>({p:m.position,color:'#dd806c',label:`${m.entity.id} ${m.hp}`})),...snapshot.members.map(m=>({p:m.position,color:m.entity.id===seat?'#84d4cf':'#80acd7',label:`${m.entity.id} ${m.hp}`}))];actors.sort((a,b)=>a.p[0]+a.p[1]-b.p[0]-b.p[1]);for(const a of actors)point(a.p,a.color,10,a.label);for(const p of snapshot.projectiles)point(p.position,p.source.kind==='participant'?'#e5d197':'#e29ea9',4);
}
canvas.addEventListener('pointerdown',e=>{if(connecting||!snapshot?.self.ready)return;const r=canvas.getBoundingClientRect(),g=geometry(),dx=e.clientX-r.left-g.ox,dy=e.clientY-r.top-g.oy;const destination=[dx+2*dy,2*dy-dx];aim=destination;input({kind:'navigate',destination}).catch(()=>{});});
new ResizeObserver(()=>dirty=true).observe(canvas);requestAnimationFrame(draw);launch().catch(problem);
