// Thin protocol-4 host: authentication binding, ordered RPC, durable I/O and recipient delivery.
// Gameplay inputs in runJourney are an ordinary consumer example, never authority facts.
import {spawn,execFileSync} from 'node:child_process';
import {createInterface} from 'node:readline';
import {readFileSync,mkdirSync,writeFileSync,chmodSync,realpathSync} from 'node:fs';
import {dirname,resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {randomUUID,createHash} from 'node:crypto';
import {DatabaseSync} from 'node:sqlite';
import assert from 'node:assert/strict';
import {newHpCostStats,observeHpCosts} from '../../web/hp-costs.mjs';
import {newSkillStats,observeSkills} from '../../web/skill-slots.mjs';
import {newFlightStats,observeFlights,observeFlightSnapshot} from '../../web/flights.mjs';
import {newAreaStats,observeAreas,observeAreaSnapshot} from '../../web/areas.mjs';
import {newCombatStats,observeCombat} from '../../web/combat.mjs';
import {newShieldStats,observeShields} from '../../web/shields.mjs';
import {newPeriodicStats,observePeriodic} from '../../web/periodic.mjs';
import {ReplicaV6,envelopeFromOutput,SpectatorReplicaV6,spectatorEnvelopeFromOutput} from '../../web/replication.mjs';
import {AssetStore,businessKind} from './asset-store.mjs';
import {captureStore,restoreStore} from '../h5/persistence.mjs';

const hash=s=>createHash('sha256').update(s).digest('hex');
const canonical=v=>Array.isArray(v)?v.map(canonical):v&&typeof v==='object'?Object.fromEntries(Object.keys(v).sort().map(k=>[k,canonical(v[k])])):v;
export const checkpointDigest=state=>hash(JSON.stringify(canonical(state)));
export const delay=ms=>new Promise(r=>setTimeout(r,ms));
export class KernelProcess {
  constructor(binary,manifest){
    this.waiters=[];this.child=spawn(binary,['--configs',manifest],{stdio:['pipe','pipe','pipe']});
    this.stderr='';this.child.stderr.on('data',b=>this.stderr=(this.stderr+b.toString()).slice(-8000));
    this.lines=createInterface({input:this.child.stdout});
    this.lines.on('line',line=>{const w=this.waiters.shift();if(!w)return;clearTimeout(w.timer);try{const value=JSON.parse(line);if(value.error)w.reject(Object.assign(new Error(value.error),{code:value.error}));else w.resolve(value.ok);}catch(e){w.reject(e);}});
    const fail=e=>{this.exited=true;for(const w of this.waiters.splice(0)){clearTimeout(w.timer);w.reject(e);}};
    this.child.on('error',fail);this.child.on('exit',code=>fail(new Error(`KERNEL_EXIT:${code}:${this.stderr}`)));this.child.stdin.on('error',fail);
  }
  rpc(command){
    if(this.exited)return Promise.reject(new Error('KERNEL_EXITED'));
    if(this.waiters.length>=64)return Promise.reject(new Error('QUEUE_FULL'));
    return new Promise((resolve,reject)=>{const timer=setTimeout(()=>{this.child.kill();reject(new Error('RPC_DEADLINE'));},10000);this.waiters.push({resolve,reject,timer});this.child.stdin.write(JSON.stringify(command)+'\n');});
  }
  async dispose(){if(this.exited)return;const done=new Promise(r=>this.child.once('exit',r));this.child.stdin.end();const timer=setTimeout(()=>this.child.kill(),3000);await done;clearTimeout(timer);this.lines.close();}
}

export class TwoMemberDatabase {
  constructor(path){
    this.path=resolve(path);mkdirSync(dirname(this.path),{recursive:true,mode:0o700});this.db=new DatabaseSync(this.path);
    this.db.exec(`PRAGMA busy_timeout=1000;PRAGMA locking_mode=EXCLUSIVE;PRAGMA journal_mode=WAL;PRAGMA synchronous=FULL;
      CREATE TABLE IF NOT EXISTS duo_rooms(id TEXT PRIMARY KEY,record TEXT NOT NULL,digest TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS duo_accounts(room_id TEXT NOT NULL,participant_id TEXT NOT NULL,state TEXT NOT NULL,digest TEXT NOT NULL,PRIMARY KEY(room_id,participant_id));
      CREATE TABLE IF NOT EXISTS duo_outputs(room_id TEXT NOT NULL,output_id TEXT NOT NULL,body TEXT NOT NULL,PRIMARY KEY(room_id,output_id));`);
    assert.equal(this.db.prepare('PRAGMA quick_check').get().quick_check,'ok');
    chmodSync(this.path,0o600);
  }
  transaction(work){this.db.exec('BEGIN IMMEDIATE');try{const result=work();this.db.exec('COMMIT');return result;}catch(e){this.db.exec('ROLLBACK');throw e;}}
  room(id){const row=this.db.prepare('SELECT * FROM duo_rooms WHERE id=?').get(id);if(!row)return null;assert.equal(hash(row.record),row.digest,'ROOM_CORRUPT');return JSON.parse(row.record);}
  writeRoom(record){const text=JSON.stringify(record);this.db.prepare('INSERT OR REPLACE INTO duo_rooms VALUES(?,?,?)').run(record.id,text,hash(text));}
  state(id,pid){const row=this.db.prepare('SELECT state,digest FROM duo_accounts WHERE room_id=? AND participant_id=?').get(id,pid);assert.ok(row,'ACCOUNT_MISSING');assert.equal(hash(row.state),row.digest,'ACCOUNT_CORRUPT');return JSON.parse(row.state);}
  writeState(id,pid,state){const text=JSON.stringify(state);this.db.prepare('INSERT OR REPLACE INTO duo_accounts VALUES(?,?,?,?)').run(id,pid,text,hash(text));}
  create(record){this.transaction(()=>{assert.equal(this.room(record.id),null);this.writeRoom(record);for(const account of record.accounts){const store=new AssetStore(account.stats,{account,config:record.config});this.writeState(record.id,account.participant_id,captureStore(store));}});}
  persist(id,outputs){
    this.transaction(()=>{const room=this.room(id);for(const output of outputs){
      assert.deepEqual(output.key,room.originalCreate.key,'OUTPUT_MATCH_MISMATCH');
      const text=JSON.stringify(output);const old=this.db.prepare('SELECT body FROM duo_outputs WHERE room_id=? AND output_id=?').get(id,output.id);if(old){assert.equal(old.body,text,'OUTPUT_ID_CONFLICT');continue;}
      this.db.prepare('INSERT INTO duo_outputs VALUES(?,?,?)').run(id,output.id,text);
      if(output.body.kind==='responsibility_v4'){
        const checkpoint=output.body.checkpoint;assert.equal(checkpointDigest(checkpoint.state),checkpoint.sha256,'CHECKPOINT_CORRUPT');
        if(!room.checkpoint||checkpoint.state.recovery_generation>room.checkpoint.state.recovery_generation||checkpoint.state.recovery_generation===room.checkpoint.state.recovery_generation&&checkpoint.state.output_sequence>=room.checkpoint.state.output_sequence)room.checkpoint=checkpoint;
        for(const member of checkpoint.state.members){
          const state=this.state(id,member.participant_id);
          for(const op of member.checkpoint.state.operations){
            const previous=state.requests.find(([k])=>k===op.request.operation_id);
            if(previous){assert.deepEqual(previous[1].body,op.request.body);if(previous[1].attempt_id<=op.request.attempt_id)previous[1]=op.request;}else state.requests.push([op.request.operation_id,op.request]);
          }this.writeState(id,member.participant_id,state);
        }
      }
      if(output.body.kind==='business_request_v4'){
        const body=output.body;assert.equal(body.principal,room.accounts.find(a=>a.participant_id===body.participant_id)?.principal,'OUTPUT_PRINCIPAL_MISMATCH');const state=this.state(id,body.participant_id);const previous=state.requests.find(([k])=>k===body.request.operation_id);
        if(previous){assert.deepEqual(previous[1].body,body.request.body);if(previous[1].attempt_id<=body.request.attempt_id)previous[1]=body.request;}else state.requests.push([body.request.operation_id,body.request]);
        this.writeState(id,body.participant_id,state);
      }
    }this.writeRoom(room);});
  }
  execute(id,pid,request,{refuse=false,notCommitted=false,dropReceipt=false,beforeCommit,afterCommit}={}){
    let outcome;
    this.transaction(()=>{const room=this.room(id);const account=room.accounts.find(a=>a.participant_id===pid);assert.ok(account);
      const store=new AssetStore(account.stats,{account,config:room.config});restoreStore(store,this.state(id,pid));
      outcome=refuse?store.refuse(request):notCommitted?store.notCommitted(request):store.execute(request);
      this.writeState(id,pid,captureStore(store));beforeCommit?.(request);
    });afterCommit?.(request);return dropReceipt?{kind:'unknown'}:outcome;
  }
  // Trusted host write. The region revision fences migrations and the sequence orders writes within it.
  savePosition(id,pid,{region_id,revision,sequence,position}){
    assert.ok(typeof region_id==='string'&&Number.isInteger(revision)&&revision>=0&&revision<=0xffffffff);
    assert.ok(Number.isSafeInteger(sequence)&&sequence>0&&Array.isArray(position)&&
      position.length===2&&Number.isFinite(position[0])&&Number.isFinite(position[1]));
    return this.transaction(()=>{
      const state=this.state(id,pid);
      if(state.location?.region_id!==region_id||state.location?.revision!==revision)return false;
      const previous=state.savedPosition;
      if(previous&&sequence<=previous.sequence){
        return sequence===previous.sequence&&position.every((value,i)=>value===previous.position[i]);
      }
      state.savedPosition={sequence,position};
      this.writeState(id,pid,state);
      return true;
    });
  }
  recovery(id,frozenCheckpoint=null){
    return this.transaction(()=>{const room=this.room(id);const checkpoint=frozenCheckpoint??room.checkpoint;assert.ok(checkpoint,'MISSING_IO_CHECKPOINT');assert.equal(checkpointDigest(checkpoint.state),checkpoint.sha256,'CHECKPOINT_CORRUPT');
      for(const member of checkpoint.state.members){const state=this.state(id,member.participant_id);
        for(const op of member.checkpoint.state.operations){
          const request=state.requests.find(([id])=>id===op.request.operation_id)?.[1];assert.ok(request,'MISSING_FROZEN_REQUEST');assert.deepEqual(request.body,op.request.body);
          const proof=state.journal.find(([id])=>id===op.request.operation_id)?.[1];
          if(['commit_assets','append_facts'].includes(op.request.body.kind)&&!op.terminal
            &&!['committed','facts_appended','refused'].includes(proof?.kind))state.fenced=[...new Set([...state.fenced,`${op.request.operation_id}/${op.request.attempt_id}`])];
        }this.writeState(id,member.participant_id,state);
      }
      room.generation=Math.max(room.generation??0,checkpoint.state.recovery_generation)+1;assert.ok(room.generation<=4294967295);this.writeRoom(room);
      return {generation:room.generation,checkpoint};
    });
  }
  views(id){const room=this.room(id);return room.accounts.map(a=>{const s=this.state(id,a.participant_id);return {participant_id:a.participant_id,balances:s.balances,revision:s.revision,admissions:s.admissions,settlements:s.settlements,compensations:s.compensations,...(s.location?{location:s.location}:{}),...(s.instances?.length?{instances:s.instances}:{}),...(s.appendedFacts?.length?{appended_facts:s.appendedFacts}:{})};});}
  rooms(){return this.db.prepare('SELECT id FROM duo_rooms ORDER BY id').all().map(r=>this.room(r.id));}
  frozenManifest(id){
    const room=this.room(id);assert.equal(hash(room.raw),room.originalCreate.config_hash,'FROZEN_CONFIG_CORRUPT');
    const dir=resolve(dirname(this.path),`frozen-${id}`);mkdirSync(dir,{recursive:true,mode:0o700});
    writeFileSync(resolve(dir,'arena.json'),room.raw,{mode:0o600});
    writeFileSync(resolve(dir,'catalog.json'),JSON.stringify({schema_version:1,default_scenario:'frozen',active:{frozen:'arena.json'},archives:[]}),{mode:0o600});
    return resolve(dir,'catalog.json');
  }
  close(){this.db.close();}
}

export function requireHostOption(value,flag) {
  if (!value || value.startsWith('--')) throw new Error(`Supply ${flag} <path>`);
  return value;
}
export function readCatalog(binary,manifest){return JSON.parse(execFileSync(binary,['--configs',manifest,'--catalog'],{encoding:'utf8',maxBuffer:16*1024*1024}));}
export class TwoMemberHost {
  constructor({binary,manifest,database,record,onPublication,onSpectatorPublication,onEvent,ioPolicy,autoAck=false,autoAckSpectators=false}){
    this.binary=resolve(binary);this.database=database;this.record=record;this.originalCreate=record.originalCreate;
    this.kernel=new KernelProcess(this.binary,manifest);this.members=record.accounts.map(a=>({account:a,connection:{...record.originalCreate.participants.find(s=>s.participant_id===a.participant_id).connection},sequence:0,periodic:newPeriodicStats(),shields:newShieldStats(),combat:newCombatStats(),areaStats:newAreaStats(),flightStats:newFlightStats(),skillStats:newSkillStats(),hpCostStats:newHpCostStats(),joined:false,snapshot:null,publication:null}));
    this.autoAck=autoAck;this.replicationStats={full:0,delta:0,bytes:0,fullEquivalentBytes:0,resyncs:0,expired:0};this.onPublication=onPublication;this.onEvent=onEvent;this.ioPolicy=ioPolicy;this.pendingIo=new Map();this.errors=[];this.events=[];this.history=[];this.phase='preparing';this.logicalMs=0;this.serial=Promise.resolve();this.disposed=false;this.needsDrain=true;
    this.spectators=new Map();this.onSpectatorPublication=onSpectatorPublication;this.autoAckSpectators=autoAckSpectators;
  }
  enqueue(work){const result=this.serial.then(work);this.serial=result.catch(()=>{});return result;}
  status(s){if(['closed','faulted'].includes(this.phase)&&!['closed','faulted'].includes(s.phase))return;if(s.logical_ms>=this.logicalMs){this.phase=s.phase;this.logicalMs=s.logical_ms;}}
  async start(recovery=null){const result=await this.enqueue(()=>{this.needsDrain=true;return this.kernel.rpc({type:'create',request:{...this.originalCreate,...(recovery?{recovery}:{})}});});this.status(result.status);return this;}
  member(pid){const m=this.members.find(m=>m.account.participant_id===pid);if(!m)throw new Error('UNAUTHORIZED');return m;}
  async input(pid,action){if(this.disposed)throw Object.assign(new Error('CLOSED'),{code:'CLOSED'});return this.enqueue(async()=>{this.needsDrain=true;const m=this.member(pid);const sequence=++m.sequence;
    const result=await this.kernel.rpc({type:'submit_input',request:{key:this.originalCreate.key,participant_id:pid,entity_generation:1,connection:m.connection,request_id:`host-${m.connection.generation}-${sequence}`,sequence,action}});if(action.kind==='request_full')m.terminalDelivered=false;this.status(result.status);return result.status;});}
  // This API is for the trusted game host; it is not a player input action.
  async setSkillLoadout(pid,change){
    const body=structuredClone(change);this.member(pid);
    return this.enqueue(async()=>{this.needsDrain=true;const result=await this.kernel.rpc({type:'set_skill_loadout',request:{key:this.originalCreate.key,participant_id:pid,entity_generation:1,operation_id:body.operation_id,expected_revision:body.expected_revision,loadout:body.loadout}});return result.loadout;});
  }
  async setEquippedWeapon(pid,change){
    const body=structuredClone(change);
    this.member(pid);
    return this.enqueue(async()=>{
      this.needsDrain=true;
      const request={key:this.originalCreate.key,participant_id:pid,entity_generation:1,
        operation_id:body.operation_id,expected_revision:body.expected_revision,weapon:body.weapon};
      const result=await this.kernel.rpc({type:'set_equipped_weapon',request});
      return result.equipment;
    });
  }
  async reconnect(pid){return this.enqueue(async()=>{this.needsDrain=true;const m=this.member(pid),previous={...m.connection};
    const connection={id:randomUUID(),generation:previous.generation+1};
    const result=await this.kernel.rpc({type:'submit_input',request:{key:this.originalCreate.key,participant_id:pid,entity_generation:1,connection,request_id:`reconnect-${connection.generation}`,sequence:1,
      action:{kind:'reconnect',previous_connection:previous,authenticated_principal:m.account.principal}}});
    m.connection=connection;m.sequence=1;m.joined=!!m.snapshot;this.status(result.status);return result.status;});}
  async receipt(output,outcome){const body=output.body;return this.enqueue(async()=>{this.needsDrain=true;const r=await this.kernel.rpc({type:'submit_business_receipt',request:{key:this.originalCreate.key,participant_id:body.participant_id,operation_id:body.request.operation_id,attempt_id:body.request.attempt_id,outcome}});this.status(r.status);});}
  async transport(pid,outputId,connection,outcome){if(this.disposed)return false;return this.enqueue(async()=>{try{this.needsDrain=true;const r=await this.kernel.rpc({type:'submit_transport_receipt',request:{key:this.originalCreate.key,recipient_id:pid,output_id:outputId,connection,outcome}});this.status(r.status);
      const m=this.member(pid);if(m.pendingPublication?.id===outputId&&['sent','failed','cancelled'].includes(outcome)){if(outcome==='sent'&&m.pendingPublication.body.payload?.kind==='full'&&m.pendingPublication.body.payload.snapshot.self.phase==='terminal')m.terminalDelivered=true;m.pendingPublication=null;}return true;
    }catch(e){if(e.code!=='CLOSED'&&e.code!=='STALE_CONNECTION'&&e.code!=='STALE_PUBLICATION')throw e;return false;}});}
  // The host authenticates the principal and chooses the slot before calling this method.
  async spectate(slot,connection,action){
    if(this.disposed)throw Object.assign(new Error('CLOSED'),{code:'CLOSED'});
    const request={key:this.originalCreate.key,slot,connection:structuredClone(connection),action:structuredClone(action)};
    return this.enqueue(async()=>{
      this.needsDrain=true;
      const result=await this.kernel.rpc({type:'spectate',request});
      const {connection,action}=request;
      if(action.kind==='attach'){
        const previous=this.spectators.get(slot);
        if(previous?.connection.id!==connection.id||previous.connection.generation!==connection.generation)
          this.spectators.set(slot,{connection:request.connection,replica:new SpectatorReplicaV6(request.key,slot),snapshot:null});
      }else if(action.kind==='detach'){
        this.spectators.delete(slot);
      }
      this.status(result.status);
      return result.status;
    });
  }
  async receiveSpectator(output){
    const {slot,connection,stream_epoch}=output.body;
    const viewer=this.spectators.get(slot);
    if(viewer?.connection.id!==connection.id||viewer.connection.generation!==connection.generation)return;
    try{
      viewer.snapshot=viewer.replica.apply(spectatorEnvelopeFromOutput(output));
    }catch(error){
      if(error.code==='BASELINE_MISSING'){
        await this.spectate(slot,connection,{kind:'request_full',stream_epoch});
      }else if(error.code!=='STALE_FRAME')throw error;
      return;
    }
    let outcome='sent';
    try{
      await this.onSpectatorPublication?.(slot,output,this);
    }catch(error){
      viewer.error=error;
      outcome='failed';
    }
    if((this.autoAckSpectators||outcome==='failed')&&this.spectators.get(slot)===viewer&&!this.disposed)
      await this.spectate(slot,connection,{kind:'transport',publication_id:output.id,outcome});
  }
  scheduleSpectator(output){
    const {slot,connection}=output.body;
    const viewer=this.spectators.get(slot);
    if(viewer?.connection.id!==connection.id||viewer.connection.generation!==connection.generation||viewer.delivering)return;
    viewer.delivering=true;
    this.receiveSpectator(output).catch(error=>{
      if(!['CLOSED','STALE_CONNECTION','STALE_PUBLICATION'].includes(error.code))viewer.error=error;
    }).finally(()=>{viewer.delivering=false;});
  }
  publicationsComplete(){return this.originalCreate.protocol_version!==6||this.members.every(m=>!m.snapshot||m.terminalDelivered||m.deliveryExpired);}
  scheduleIo(output){
    const body=output.body,r=body.request,id=`${r.operation_id}/${r.attempt_id}`;if(this.pendingIo.has(id))return;
    const work=(async()=>{const account=this.member(body.participant_id).account;assert.equal(body.principal,account.principal,'PRINCIPAL_MISMATCH');
      const policy=await this.ioPolicy?.(body.participant_id,r,this)??{};if(policy.hold)return;
      if(policy.delayMs)await delay(policy.delayMs);
      const outcome=this.database.execute(this.record.id,body.participant_id,r,policy);await this.receipt(output,outcome);
    })().catch(e=>this.errors.push(e.stack??String(e))).finally(()=>this.pendingIo.delete(id));this.pendingIo.set(id,work);
  }
  async pump(){
    if(this.disposed)return [];
    const outputs=await this.enqueue(async()=>{const r=await this.kernel.rpc({type:'drain_outputs'});this.database.persist(this.record.id,r.outputs.filter(o=>o.body.kind!=='spectator_publication_v6'));this.needsDrain=false;return r.outputs;});
    for(const output of outputs){const body=output.body;
      if(body.kind==='spectator_publication_v6'){this.scheduleSpectator(output);continue;}
      this.history.push(output);if(this.history.length>512)this.history.shift();
      if(body.kind==='business_request_v4')this.scheduleIo(output);
      else if(body.kind==='publication_v6'){
        const m=this.member(body.recipient_id);m.replica??=new ReplicaV6(this.originalCreate.key,body.recipient_id);m.pendingPublication=output;m.publication=output.id;m.joined=true;const envelope=envelopeFromOutput(output);this.replicationStats[body.payload.kind]++;this.replicationStats.bytes+=Buffer.byteLength(JSON.stringify(envelope));
        try{m.snapshot=m.replica.apply(envelope);observeAreaSnapshot(m.areaStats,m.snapshot);observeFlightSnapshot(m.flightStats,m.snapshot);m.readyPublication=m.replica.lastFullId;this.status({phase:m.snapshot.phase,logical_ms:m.snapshot.logical_ms});this.replicationStats.fullEquivalentBytes+=Buffer.byteLength(JSON.stringify({...envelope,payload:{kind:'full',snapshot:m.snapshot,reason:'initial'}}));}
        catch(e){if(e.code==='BASELINE_MISSING'){this.replicationStats.resyncs++;await this.input(body.recipient_id,{kind:'request_full',stream_epoch:body.stream_epoch});}else if(e.code!=='STALE_FRAME')throw e;}
        await this.onPublication?.(body.recipient_id,output,this);if(this.autoAck)await this.transport(body.recipient_id,output.id,body.connection,'sent');
      }
      else if(body.kind==='publication_v4'){
        const m=this.member(body.recipient_id);m.snapshot=body.snapshot;observeAreaSnapshot(m.areaStats,m.snapshot);observeFlightSnapshot(m.flightStats,m.snapshot);m.publication=output.id;m.joined=true;this.status({phase:body.snapshot.phase,logical_ms:body.snapshot.logical_ms});
        await this.onPublication?.(body.recipient_id,output,this);
      }else if(body.kind==='event_v4'){for(const m of body.recipient_id?[this.member(body.recipient_id)]:this.members){observeCombat(m.combat,body.event,m.account.participant_id);observeAreas(m.areaStats,body.event,m.account.participant_id);observeFlights(m.flightStats,body.event,m.account.participant_id);observeSkills(m.skillStats,body.event);observeHpCosts(m.hpCostStats,body.event);}if(body.recipient_id){observePeriodic(this.member(body.recipient_id).periodic,body.event,body.recipient_id);observeShields(this.member(body.recipient_id).shields,body.event,body.recipient_id);}if(body.event.kind==='publication_expired'&&body.recipient_id){const m=this.member(body.recipient_id);m.deliveryExpired=true;m.pendingPublication=null;this.replicationStats.expired++;}this.events.push({recipient_id:body.recipient_id,event:body.event});if(this.events.length>512)this.events.shift();await this.onEvent?.(body.recipient_id,body.event,this);}
    }
    return outputs;
  }
  async until(predicate,timeout=15000){const deadline=Date.now()+timeout;while(!predicate()){assert.ok(Date.now()<deadline,`STAGE_DEADLINE:${this.phase}:${this.errors.join('|')}`);assert.equal(this.errors.length,0,this.errors.join('\n'));await this.pump();await delay(15);}}
  async joinAndReady({reverse=false}={}){
    await this.until(()=>this.phase==='deploying');
    for(const m of (reverse?[...this.members].reverse():this.members))await this.input(m.account.participant_id,{kind:'join'});
    await this.until(()=>this.members.every(m=>m.snapshot));
    for(const m of this.members){await this.transport(m.account.participant_id,m.publication,m.connection,'sent');await this.input(m.account.participant_id,{kind:'combat_ready',publication_id:m.readyPublication??m.publication});}
    await this.until(()=>this.phase==='active');
  }
  pending(){if(this.needsDrain)return null;const c=this.database.room(this.record.id).checkpoint;return c?c.state.members.reduce((n,m)=>n+m.checkpoint.state.operations.filter(o=>!o.terminal).length,0):null;}
  async close(){if(this.disposed)return;const r=await this.enqueue(()=>{this.needsDrain=true;return this.kernel.rpc({type:'close',request:{key:this.originalCreate.key,request_id:'host-close'}});});this.status(r.status);}
  async dispose(){if(this.disposal)return this.disposal;this.disposed=true;this.disposal=(async()=>{await Promise.allSettled(this.pendingIo.values());await this.serial;await this.kernel.dispose();})();return this.disposal;}
}

export function newRecord({catalog,scenario,accounts,configPath,manifest}){
  const active=catalog.active.find(a=>a.scenario===scenario);assert.ok(active,'UNKNOWN_SCENARIO');
  const version=catalog.versions.find(v=>v.id===active.id&&v.hash===active.hash);assert.ok([5,6,7].includes(version.config.schema_version),'UNSUPPORTED_MEMBER_SCHEMA');
  const id=randomUUID();return {id,config:version.config,raw:version.raw,scenario,configPath,manifest,accounts:accounts.participants,generation:0,
    originalCreate:{protocol_version:version.config.schema_version-1,key:{id:`two-${id}`,incarnation:1},request_id:'create',config_id:active.id,config_hash:active.hash,seed:accounts.seed,
      participants:accounts.participants.map(a=>({...Object.hasOwn(a,'weapon')?{weapon:structuredClone(a.weapon)}:{},...Object.hasOwn(a,'basic_profile')?{basic_profile:a.basic_profile}:{},...Object.hasOwn(a,'combat_build')?{combat_build:structuredClone(a.combat_build)}:{},...Object.hasOwn(a,'skills')?{skills:structuredClone(a.skills)}:{},participant_id:a.participant_id,authenticated_principal:a.principal,connection:{id:randomUUID(),generation:1}})),publication_mode:'interactive'}};
}

export async function runJourney(host,{reverse=false}={}){
  await host.joinAndReady({reverse});const order=reverse?[...host.members].reverse():host.members;
  const first=order[0],second=order[1],c=host.record.config;
  const casts=[];let last=0;
  await host.until(()=>host.members.every(m=>m.snapshot?.phase==='active'));
  await host.input(first.account.participant_id,{kind:'cast_self',skill_id:c.additional_skills[0].id});
  // Scripted consumer sends intent from the authoritative snapshot; Rust owns targets and damage.
  await host.until(()=>first.snapshot.self.pending_action===null&&first.snapshot.self.spirit<first.account.stats.spirit_points);
  const fightDeadline=Date.now()+20000;
  while(first.snapshot.monsters.some(n=>n.hp>0)){
    assert.ok(Date.now()<fightDeadline,'FIGHT_DEADLINE');
    for(const m of order){const target=m.snapshot.monsters.filter(n=>n.hp>0).sort((a,b)=>Math.hypot(...a.position.map((v,i)=>v-m.snapshot.self.position[i]))-Math.hypot(...b.position.map((v,i)=>v-m.snapshot.self.position[i])))[0];
      if(target&&!m.snapshot.self.pending_action&&m.snapshot.logical_ms>=m.snapshot.self.recovery_until_ms&&m.snapshot.logical_ms>=(m.snapshot.self.player_skills.find(s=>s.skill_id===c.skill.id)?.ready_at_ms??0)){
        try{await host.input(m.account.participant_id,{kind:'cast',skill_id:c.skill.id,direction:target.position.map((v,i)=>v-m.snapshot.self.position[i])});casts.push(m.account.participant_id);}catch(e){if(!['INVALID_STATE','NO_RESOURCE','INVALID_TARGET'].includes(e.code))throw e;}
      }
    }await host.pump();await delay(35);last++;
  }
  const resource=first.snapshot.resources.find(r=>r.item_id);assert.ok(resource,'REAL_DROP_REQUIRED');
  for(const m of order)await host.input(m.account.participant_id,{kind:'navigate',destination:resource.position});
  await host.until(()=>order.every(m=>Math.hypot(...m.snapshot.self.position.map((v,i)=>v-resource.position[i]))<5));
  for(const m of order)await host.input(m.account.participant_id,{kind:'stop'});
  const won=await host.input(first.account.participant_id,{kind:'pickup',resource_id:resource.id,expected_revision:first.snapshot.self.inventory_revision});
  let lost;try{await host.input(second.account.participant_id,{kind:'pickup',resource_id:resource.id,expected_revision:second.snapshot.self.inventory_revision});}catch(e){lost=e.code;}
  assert.equal(lost,'INVALID_TARGET');
  for(const m of order)await host.input(m.account.participant_id,{kind:'navigate',destination:c.extraction_point});
  await host.until(()=>order.every(m=>Math.hypot(...m.snapshot.self.position.map((v,i)=>v-c.extraction_point[i]))<5)&&first.snapshot.extraction_open);
  for(const m of order)await host.input(m.account.participant_id,{kind:'stop'});
  await host.input(first.account.participant_id,{kind:'extract_start'});
  await host.until(()=>host.database.views(host.record.id).find(v=>v.participant_id===first.account.participant_id).settlements===1);
  assert.equal(host.phase,'active','SURVIVOR_WORLD_MUST_KEEP_RUNNING');
  await host.input(second.account.participant_id,{kind:'leave'});
  await host.until(()=>host.phase==='closed'&&host.pending()===0);
  const views=host.database.views(host.record.id);assert.ok(views.every(v=>v.admissions===1&&v.settlements===1&&v.compensations===0));
  assert.equal(host.errors.length,0);return {clock:'ordinary',worlds:1,members:views,casts,resourceId:resource.id,pickupWinner:first.account.participant_id,pickupLoserResult:lost,pending:host.pending(),phase:host.phase,iterations:last,at:won.logical_ms};
}

async function main(){
  const arg=(key,fallback)=>{const i=process.argv.indexOf(key);return i<0?fallback:process.argv[i+1]};
  const binary=resolve(arg('--binary','target/debug/kernel-host'));const resume=process.argv.includes('--resume');
  let manifest=resume?undefined:resolve(requireHostOption(arg('--configs',process.env.MATCH_CONFIG_MANIFEST),'--configs'));
  const accounts=resume?undefined:JSON.parse(readFileSync(requireHostOption(arg('--accounts'),'--accounts'),'utf8'));
  const database=new TwoMemberDatabase(arg('--database','.local/two-member-node.sqlite'));
  let record,recovery;
  if(resume){record=database.room(arg('--resume'));const checkpointPath=arg('--frozen-checkpoint');recovery=database.recovery(record.id,checkpointPath?JSON.parse(readFileSync(checkpointPath,'utf8')):null);manifest=database.frozenManifest(record.id);}
  else{
    const catalog=readCatalog(binary,manifest);
    record=newRecord({catalog,scenario:arg('--scenario',catalog.defaultScenario),accounts,manifest});
    database.create(record);
  }
  const admissionOrder=[],reverse=process.argv.includes('--reverse');
  const host=new TwoMemberHost({binary,manifest,database,record,ioPolicy:(pid,r)=>businessKind(r)==='commit_admission'?{delayMs:reverse&&pid===record.accounts[0].participant_id?100:0,afterCommit:()=>admissionOrder.push(pid)}:{}});
  try{await host.start(recovery);let report;
    if(recovery){await host.until(()=>host.pending()===0);report={phase:host.phase,pending:host.pending(),members:database.views(record.id),recoveryGeneration:recovery.generation};}
    else report={...await runJourney(host,{reverse}),admissionOrder};
    console.log(JSON.stringify({roomId:record.id,...report}));
  }finally{await host.dispose();database.close();}
}
if(process.argv[1]&&import.meta.url===pathToFileURL(realpathSync(process.argv[1])).href)main().catch(e=>{console.error(e.stack);process.exitCode=1;});
