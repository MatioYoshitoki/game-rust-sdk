import http from 'node:http';
import {createInterface} from 'node:readline';
import {readFile} from 'node:fs/promises';
import {readFileSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {randomUUID} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {TwoMemberDatabase,TwoMemberHost,readCatalog,newRecord} from '../node/two-member-host.mjs';

const ROOT=fileURLToPath(new URL('../../',import.meta.url));
const arg=(name,fallback)=>{const i=process.argv.indexOf(name);return i<0?fallback:process.argv[i+1]};
const pagePrefix=arg('--page-prefix','two-member');if(!['two-member','member','delta','multiplayer-periodic','multiplayer-shield','multiplayer-combat','member-area','member-line','member-piercing','member-skills','skill-hp-cost'].includes(pagePrefix))throw new Error('INVALID_PAGE_PREFIX');
const configPath=arg('--configs',process.env.MATCH_CONFIG_MANIFEST);
const hostConfigPath=arg('--host-config');
if(!configPath||configPath.startsWith('--'))throw new Error('Supply --configs <catalog.json> or MATCH_CONFIG_MANIFEST');
if(!hostConfigPath||hostConfigPath.startsWith('--'))throw new Error('Supply --host-config <host.json>');
const binary=resolve(arg('--binary',resolve(ROOT,'target/debug/kernel-host'))),manifest=resolve(configPath);
const hostPath=resolve(hostConfigPath),hostConfig=JSON.parse(readFileSync(hostPath,'utf8'));
const catalog=readCatalog(binary,manifest),info=JSON.parse(execFileSync(binary,['--sdk-info'],{encoding:'utf8'}));
const database=new TwoMemberDatabase(arg('--database',resolve(ROOT,'.local/sdk-5-18-demo.sqlite'))),rooms=new Map();
const port=Number(arg('--port','4190'));
const allowed={join:[],combat_ready:['publication_id'],move:['direction'],navigate:['destination'],stop:[],attack:[],cast:['skill_id','direction'],cast_self:['skill_id'],cancel_action:[],pickup:['resource_id','expected_revision'],open_chest:['chest_id','expected_generation'],cancel_chest_unlock:['chest_id','expected_generation'],extract_start:[],extract_cancel:[],background:[],foreground:[],leave:[],request_full:['stream_epoch']};
const fail=(code,status=400)=>Object.assign(new Error(code),{status});
const json=(res,status,data)=>{res.writeHead(status,{'content-type':'application/json; charset=utf-8','cache-control':'no-store'});res.end(JSON.stringify(data));};
const versionedInputs = {
  set_owned_targets: { minimum: 105, fields: ['preset_id'] },
  clear_owned_targets: { minimum: 105, fields: [] },
  reset_training_metrics: { minimum: 105, fields: [] },
  drop_item: { minimum: 57, fields: ['item_id', 'quantity', 'expected_revision'] },
  use_item: { minimum: 58, fields: ['item_id', 'expected_revision'] },
  attack_unit: { minimum: 54, fields: ['target'] },
  attack_ground: { minimum: 105, fields: ['position'] },
  start_basic_charge: { minimum: 105, fields: [] },
  release_basic_charge: { minimum: 105, fields: ['session_id', 'target', 'direction', 'position'] },
  cancel_basic_charge: { minimum: 105, fields: ['session_id'] },
  cast_unit: { minimum: 40, fields: ['skill_id', 'target'] },
  cast_ground: { minimum: 41, fields: ['skill_id', 'position'] },
  cast_at_anchor: { minimum: 105, fields: ['skill_id', 'anchor_id', 'direction'] },
  recast_anchor: { minimum: 105, fields: ['anchor_id', 'action_id', 'recast_index', 'position'] },
  return_anchor: { minimum: 105, fields: ['anchor_id', 'action_id', 'recast_index'] },
  end_anchor: { minimum: 105, fields: ['anchor_id', 'action_id', 'recast_index'] },
  start_charge: { minimum: 47, fields: ['skill_id', 'direction'] },
  release_charge: { minimum: 47, fields: ['action_id'] },
  update_charge_aim: { minimum: 48, fields: ['action_id', 'direction'] },
  recast_projectile: { minimum: 50, fields: ['action_id', 'recast_index', 'direction'] },
  start_attack_beam: { minimum: 105, fields: ['direction'] },
  aim_attack_beam: { minimum: 105, fields: ['action_id', 'direction'] },
  end_attack_beam: { minimum: 105, fields: ['action_id'] },
  open_relic: { minimum: 105, fields: ['relic_id'] },
  use_exposure_node: { minimum: 105, fields: ['node_id','expected_revision','choice'] },
  claim_world_equipment: { minimum: 105, fields: ['node_id','expected_revision'] },
  claim_relic: { minimum: 105, fields: ['relic_id','expected_revision','expected_inventory_revision','entry_id'] },
  claim_boon: { minimum: 105, fields: ['opportunity_id','boon_id','expected_revision','expected_inventory_revision','replace_slot'] },
  counter_checkpoint: { minimum: 105, fields: ['schedule_id','window_index','target'] },
};
function allowedFields(kind, mechanism) {
  if (kind === 'release_charge' && mechanism >= 48) return ['action_id', 'direction'];
  if (Object.hasOwn(versionedInputs, kind)) {
    const entry = versionedInputs[kind];
    return mechanism >= entry.minimum ? entry.fields : null;
  }
  return Object.hasOwn(allowed, kind) ? allowed[kind] : null;
}
async function payload(req){let text='';for await(const b of req){text+=b;if(Buffer.byteLength(text)>8192)throw fail('INVALID_MESSAGE',413);}try{return JSON.parse(text||'{}')}catch{throw fail('INVALID_MESSAGE')}}

class Room {
  constructor(record,selectedManifest,recovery){
    this.record=record;this.streams=new Map();this.started=new Set();this.busy=false;this.readyAfterRestart=!!recovery;
    this.host=new TwoMemberHost({binary,manifest:selectedManifest,database,record,
      onPublication:(pid,output)=>{const stream=this.streams.get(pid);if(stream)stream.offer(output);},
      onEvent:(recipient,event)=>{for(const [pid,stream] of this.streams){if(recipient===null||recipient===pid)stream.event(event);}},
    });this.start=this.host.start(recovery);this.start.catch(e=>this.host.errors.push(e.stack));
  }
  authenticate(url){const pid=url.searchParams.get('seat'),token=url.searchParams.get('token');if(!Object.hasOwn(this.record.tokens,pid??'')||this.record.tokens[pid]!==token)throw fail('UNAUTHORIZED',401);return pid;}
  async connect(pid,res){
    await this.start;const m=this.host.member(pid);const old=this.streams.get(pid);if(old){this.streams.delete(pid);old.close();}
    if(this.started.has(pid)&&!['closed','faulted'].includes(this.host.phase))await this.host.reconnect(pid);
    this.started.add(pid);const stream=new Stream(this,pid,res);this.streams.set(pid,stream);
  }
  async pump(){
    if(this.busy)return;this.busy=true;
    try{await this.start;await this.host.pump();
      if(this.host.phase==='deploying')for(const m of this.host.members){if(this.streams.has(m.account.participant_id)&&!m.joined){
        try{await this.host.input(m.account.participant_id,{kind:'join'});m.joined=true;}catch(e){if(!['NOT_READY','INVALID_STATE','CLOSED'].includes(e.code))throw e;}
      }}
      if(['closed','faulted'].includes(this.host.phase)&&this.host.pending()===0&&this.host.pendingIo.size===0&&this.host.publicationsComplete())await this.host.dispose();
    }catch(e){this.host.errors.push(e.stack??String(e));for(const s of this.streams.values())s.event({kind:'problem',code:'HOST_IO_ERROR'});}
    finally{this.busy=false;}
  }
  view(pid){const m=this.host.member(pid);const ledger=database.views(this.record.id).find(v=>v.participant_id===pid);return {room_id:this.record.id,participant_id:pid,phase:this.host.phase,logical_ms:this.host.logicalMs,host_live:!this.host.kernel.exited,
    periodic:m.periodic,shields:m.shields,combat:m.combat,areaStats:m.areaStats,flightStats:m.flightStats,skillStats:m.skillStats,hpCostStats:m.hpCostStats,match_key:this.record.originalCreate.key,replication:this.host.replicationStats,publication_id:m.publication,delivery_expired:!!m.deliveryExpired,protocol_version:this.record.originalCreate.protocol_version,snapshot:m.snapshot,ledger,errors:this.host.errors.map(()=>'HOST_IO_ERROR'),configuration:{id:this.record.config.id,mechanism_version:this.record.config.mechanism_version,ally_self_target_skills:this.record.config.combat_rules?.ally_self_target_skills,skill_followup:this.record.config.skill_followup,basic_followup:this.record.config.basic_followup,member_loot_drop:this.record.config.member_loot_drop,member_item_use:this.record.config.member_item_use,spawn:this.record.config.spawn,extraction_point:this.record.config.extraction_point,pve:{width:this.record.config.pve.width,height:this.record.config.pve.height,cell_size:this.record.config.pve.cell_size,isometric_grid:this.record.config.pve.isometric_grid??null,palette:this.record.config.pve.palette,cells_rle:this.record.config.pve.cells_rle},skills:[this.record.config.skill,...(this.record.config.additional_skills??[]),...(this.record.config.skill_library??[])]},sdk:info};}
  links(){return this.record.accounts.map(a=>({participant_id:a.participant_id,url:`/?room=${this.record.id}&seat=${encodeURIComponent(a.participant_id)}&token=${this.record.tokens[a.participant_id]}`}));}
}
class Stream {
  constructor(room,pid,res){this.room=room;this.pid=pid;this.res=res;this.writing=false;this.pending=null;this.events=[];this.dead=false;
    res.writeHead(200,{'content-type':'text/event-stream','cache-control':'no-cache','connection':'keep-alive'});res.write(': connected\n\n');
    res.on('close',()=>{this.dead=true;this.pending=null;if(room.streams.get(pid)===this){room.streams.delete(pid);if(!['closed','faulted'].includes(room.host.phase))room.host.input(pid,{kind:'connection_lost'}).catch(e=>{if(e.code!=='CLOSED')room.host.errors.push(e.stack)});}});
  }
  event(event){if(this.dead)return;if(this.events.length>=64){this.close();return;}this.events.push(event);this.flush();}
  receipt(output,outcome){const b=output.body;this.room.host.transport(this.pid,output.id,b.connection,outcome).catch(e=>this.room.host.errors.push(e.stack));}
  offer(output){
    if(this.dead)return;
    if(this.pending)this.receipt(this.pending,'cancelled');this.pending=output;this.flush();
  }
  flush(){
    if(this.dead||this.writing)return;
    if(this.events.length){this.writing=true;const event=this.events.shift();this.res.write(`event: gameplay\ndata: ${JSON.stringify(event)}\n\n`,()=>{this.writing=false;this.flush();});return;}
    if(!this.pending)return;
    const output=this.pending;this.pending=null;this.writing=true;const body=output.body;
    const envelope=body.kind==='publication_v6'?{key:output.key,publication_id:output.id,recipient_id:body.recipient_id,connection:body.connection,stream_epoch:body.stream_epoch,revision:body.revision,payload:body.payload}:{publication_id:output.id,connection:body.connection,snapshot:body.snapshot};
    this.res.write(`event: snapshot\ndata: ${JSON.stringify(envelope)}\n\n`,error=>{this.writing=false;this.receipt(output,error?'failed':'sent');this.flush();});
  }
  close(){this.dead=true;this.res.end();}
}

for(const record of database.rooms()){
  const recovery=database.recovery(record.id);rooms.set(record.id,new Room(record,database.frozenManifest(record.id),recovery));
}
const timer=setInterval(()=>{for(const room of rooms.values())room.pump();},20);
const server=http.createServer(async(req,res)=>{
  try{
    const url=new URL(req.url,'http://localhost');
    if(req.method==='GET'&&url.pathname==='/api/config'){json(res,200,{sdk:info,scenarios:catalog.active.filter(a=>Object.hasOwn(hostConfig.scenarios,a.scenario)).map(a=>({id:a.scenario,name:a.scenario}))});return;}
    if(req.method==='POST'&&url.pathname==='/api/rooms'){
      const body=await payload(req);if(Object.keys(body).some(k=>k!=='scenario')||!Object.hasOwn(hostConfig.scenarios,body.scenario))throw fail('INVALID_MESSAGE');
      const accounts=JSON.parse(readFileSync(resolve(dirname(hostPath),hostConfig.scenarios[body.scenario].accounts),'utf8'));
      const record=newRecord({catalog,scenario:body.scenario,accounts,manifest});record.tokens=Object.fromEntries(record.accounts.map(a=>[a.participant_id,randomUUID()]));record.owner_token=randomUUID();database.create(record);
      const room=new Room(record,manifest,null);rooms.set(record.id,room);await room.start;json(res,201,{room_id:record.id,owner_token:record.owner_token,links:room.links()});return;
    }
    if(url.pathname.startsWith('/api/')){
      const room=rooms.get(url.searchParams.get('room'));if(!room)throw fail('NOT_FOUND',404);
      if(url.pathname==='/api/links'||url.pathname==='/api/close'){
        if(url.searchParams.get('owner')!==room.record.owner_token)throw fail('UNAUTHORIZED',401);
        if(req.method==='POST'&&url.pathname==='/api/close'){await room.host.close();json(res,200,{ok:true});return;}
        if(req.method==='GET'&&url.pathname==='/api/links'){json(res,200,{links:room.links()});return;}
        throw fail('INVALID_MESSAGE');
      }
      const pid=room.authenticate(url);
      if(req.method==='GET'&&url.pathname==='/api/view'){json(res,200,room.view(pid));return;}
      if(req.method==='GET'&&url.pathname==='/api/ledger'){const v=room.view(pid);json(res,200,{phase:v.phase,ledger:v.ledger,host_live:v.host_live,delivery_expired:v.delivery_expired,errors:v.errors});return;}
      if(req.method==='GET'&&url.pathname==='/api/events'){await room.connect(pid,res);return;}
      if(req.method==='POST'&&url.pathname==='/api/input'){
        const action=await payload(req),fields=allowedFields(action.kind, room.record.config.mechanism_version);if(!fields||Object.keys(action).some(k=>k!=='kind'&&!fields.includes(k)))throw fail('UNAUTHORIZED',403);
        const status=await room.host.input(pid,action);json(res,200,{status});return;
      }
      throw fail('NOT_FOUND',404);
    }
    const file=['/replication.mjs','/periodic.mjs','/shields.mjs','/combat.mjs','/areas.mjs','/flights.mjs','/skill-slots.mjs','/hp-costs.mjs','/isometric.js'].includes(url.pathname)?url.pathname.slice(1):url.pathname==='/'?`${pagePrefix}.html`:url.pathname===`/${pagePrefix}.js`?`${pagePrefix}.js`:null;if(!file)throw fail('NOT_FOUND',404);
    res.writeHead(200,{'content-type':file.endsWith('.html')?'text/html; charset=utf-8':'text/javascript; charset=utf-8','cache-control':'no-store'});res.end(await readFile(resolve(ROOT,'web',file)));
  }catch(e){if(res.headersSent){res.end();return;}json(res,e.status??409,{error:e.code??e.message});}
});
// Optional local pipe for a trusted supervising host. There is no HTTP grant route.
let trustedControl=null;
if(process.argv.includes('--trusted-control-stdin')){
 trustedControl=createInterface({input:process.stdin});let serial=Promise.resolve();
 trustedControl.on('line',line=>{serial=serial.then(async()=>{
  let command;
  try{
   if(Buffer.byteLength(line)>8192)throw fail('INVALID_MESSAGE');command=JSON.parse(line);
   const kind=command?.kind;
   if(!['set_skill_loadout','set_equipped_weapon'].includes(kind))throw fail('INVALID_MESSAGE');
   const fields=['kind','room_id','participant_id','operation_id','expected_revision',kind==='set_skill_loadout'?'loadout':'weapon'];
   if(Object.keys(command).some(key=>!fields.includes(key)))throw fail('INVALID_MESSAGE');
   const room=rooms.get(command.room_id);if(!room)throw fail('NOT_FOUND');await room.start;
   const receipt=kind==='set_skill_loadout'?await room.host.setSkillLoadout(command.participant_id,command)
     :await room.host.setEquippedWeapon(command.participant_id,command);
   console.log(JSON.stringify({kind:'trusted_control_result',operation_id:command.operation_id,receipt}));
  }catch(error){console.log(JSON.stringify({kind:'trusted_control_result',operation_id:command?.operation_id,error:error.code??error.message}));}
 }).catch(error=>console.error(error));});
}
server.listen(port,'127.0.0.1',()=>console.log(JSON.stringify({url:`http://127.0.0.1:${server.address().port}/`,database:database.path,sdk:info.crateVersion})));
let stopping=false;async function shutdown(){if(stopping)return;stopping=true;trustedControl?.close();clearInterval(timer);server.close();for(const room of rooms.values()){for(const s of room.streams.values())s.close();await room.host.dispose();}database.close();}
process.on('SIGTERM',()=>shutdown().then(()=>process.exit()));process.on('SIGINT',()=>shutdown().then(()=>process.exit()));
