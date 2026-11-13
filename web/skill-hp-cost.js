function hasStatusAttribute(status, attribute) { return status.attribute === attribute || status.flags?.includes(attribute); }
import {newHpCostStats,observeHpCosts} from './hp-costs.mjs';
const hpCostStats=newHpCostStats();
let lastChargeCommit = null;
let lastShieldRecord = null;
let lastStatusConsumption = null;
let lastStatusTransfer = null;
let lastDeathGuard = null;
let lastActionTotal = null;
let lastChainDamage = null;
let lastCooldownReturn = null;
let lastContactReaction = null;
let lastFinisher = null;
let lastWindowEnd = null;
let lastStatusCleanse = null;
let lastPeriodicSpirit = null;
let chargeAimDraft = null;
let recastAimDraft = null;
let lastRecast = null;
let basicCommits = 0;
let attackTargetSignature = null;
let allyTargetSignature = null;
let channelPulseCount = 0;
let channelSpiritCost = 0;
let lastChannelEnd = null;
const channelEndLabels = {
  completed: '自然结束', cancelled: '主动取消', interrupted: '引导中断',
  source_invalid: '成员离场', no_resource: '灵力不足', capacity: '弹道容量已满', invalid_origin: '起点失效',
};
import {newSkillStats,observeSkills,skillAction} from './skill-slots.mjs';
import {newFlightStats,observeFlights,observeFlightSnapshot} from './flights.mjs';
import {newAreaStats,observeAreas,observeAreaSnapshot} from './areas.mjs';
import {newCombatStats,observeCombat} from './combat.mjs';
import {newShieldStats,observeShields} from './shields.mjs';
import {newPeriodicStats,observePeriodic} from './periodic.mjs';
import {ReplicaV6} from './replication.mjs';
const $=id=>document.getElementById(id),q=new URLSearchParams(location.search),room=q.get('room'),seat=q.get('seat'),token=q.get('token');
let replica=null,resyncBusy=false,statistics={full:0,delta:0,bytes:0,fullEquivalentBytes:0,resyncs:0,stale:0},snapshot=null,configuration=null,stream=null,readyBusy=false,dirty=true,aim=null,connecting=false;
const skillStats=newSkillStats();
const flightStats=newFlightStats();
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
  stream.addEventListener('gameplay', message => {
    const event = JSON.parse(message.data);
    observePeriodic(periodic, event, seat);
    observeShields(shields, event, seat);
    observeCombat(combat, event, seat);
    observeAreas(areaStats, event, seat);
    observeFlights(flightStats, event, seat);
    observeSkills(skillStats, event);
    observeHpCosts(hpCostStats, event);
    const privateEvent = event.kind === 'private' ? event.event : null;
    if(privateEvent?.kind==='status_end_resolved'){window.__lastStatusEnd=structuredClone(privateEvent);const result=$('lastSelfHeal');result.hidden=false;result.textContent=`状态结束 · ${privateEvent.definition_id} · ${privateEvent.reason} · 当前效果 ${privateEvent.active_definition_ids.join('、') || '无'}${privateEvent.failure ? ' · '+privateEvent.failure : ''}`;}
    if(privateEvent?.kind==='status_ends_suppressed'){window.__lastStatusEndSuppression=structuredClone(privateEvent);}
    if(privateEvent?.kind==='resource_stolen'){window.__lastResourceStolen=structuredClone(privateEvent);const result=$('lastSelfHeal');result.hidden=false;result.textContent=`资源夺取 · 生命扣除 ${privateEvent.hp_removed} / 实回 ${privateEvent.hp_restored} · 灵力扣除 ${privateEvent.spirit_removed} / 实回 ${privateEvent.spirit_restored}`;}
    if(privateEvent?.kind==='shield_record_ended' && privateEvent.recorded>0)lastShieldRecord=privateEvent;
    if(privateEvent?.kind==='status_consumed')lastStatusConsumption=privateEvent;
    if(privateEvent?.kind==='status_transferred')lastStatusTransfer=privateEvent;
    if(privateEvent?.kind==='death_guard_activated')lastDeathGuard=privateEvent;
    if(privateEvent?.kind==='finisher_resolved'){lastFinisher=privateEvent;window.__lastFinisher=structuredClone(privateEvent);}
    if(privateEvent?.kind==='contact_reaction_resolved'){lastContactReaction=privateEvent;window.__lastContactReaction=structuredClone(privateEvent);}
    if(privateEvent?.kind==='skill_cooldowns_reduced'){lastCooldownReturn=privateEvent;window.__lastCooldownReturn=structuredClone(privateEvent);}
    if(privateEvent?.kind==='chain_damage_resolved'){lastChainDamage=privateEvent;window.__lastChainDamage=structuredClone(privateEvent);}
    if(privateEvent?.kind==='combat_action_total'){lastActionTotal=privateEvent;window.__lastActionTotal=structuredClone(privateEvent);}
    if(privateEvent?.kind==='periodic_spirit_tick'){lastPeriodicSpirit=privateEvent;window.__lastPeriodicSpirit=structuredClone(privateEvent);}
    if(privateEvent?.kind==='status_cleanse_resolved'){lastStatusCleanse=privateEvent;window.__lastStatusCleanse=structuredClone(privateEvent);}
    if(privateEvent?.kind==='window_record_ended'){lastWindowEnd=privateEvent;window.__lastWindowEnd=structuredClone(privateEvent);}
    if (privateEvent?.kind === 'action_committed' && privateEvent.charge) {
      lastChargeCommit = privateEvent.charge;
      window.__lastChargeCommit = structuredClone(privateEvent);
    }
    if (privateEvent?.kind === 'action_committed' && !privateEvent.skill) {
      window.__basicCommits = ++basicCommits;
      window.__lastBasicCommit = structuredClone(privateEvent);
    }
    if (privateEvent?.kind === 'heal_resolved' && privateEvent.heal_kind === 'self_skill') {
      const result = $('lastSelfHeal');
      result.hidden = false;
      result.textContent = `自身治疗 · 基础 ${privateEvent.raw_heal} · 实回 ${privateEvent.hp_restored}`;
    }
    if (privateEvent?.kind === 'heal_resolved' && privateEvent.heal_kind === 'ally_skill') {
      const result = $('lastAllyHeal');
      result.hidden = false;
      result.textContent = `友方治疗 · ${privateEvent.source.id} → ${privateEvent.target.id} · 实回 ${privateEvent.hp_restored}`;
    }
    if (privateEvent?.kind === 'periodic_heal_tick' &&
        privateEvent.source.kind === 'participant' && privateEvent.target.kind === 'participant' &&
        privateEvent.source.id !== privateEvent.target.id) {
      const result = $('lastAllyHeal');
      result.hidden = false;
      result.textContent = `友方持续治疗 · ${privateEvent.source.id} → ${privateEvent.target.id} · 第 ${privateEvent.tick_index} 跳 · 实回 ${privateEvent.hp_restored}`;
    }
    if (privateEvent?.kind === 'shield_granted' &&
        privateEvent.source.kind === 'participant' && privateEvent.target.kind === 'participant' &&
        privateEvent.source.id !== privateEvent.target.id) {
      const result = $('lastAllyShield');
      result.hidden = false;
      result.textContent = `友方护盾 · ${privateEvent.source.id} → ${privateEvent.target.id} · 实授 ${privateEvent.granted}`;
    }
    if (privateEvent?.kind === 'status_rejected' && privateEvent.reason === 'IMMUNE') {
      const notice = $('lastControlRejection');
      notice.hidden = false;
      notice.textContent = `已免疫控制 · ${privateEvent.source.id} → ${privateEvent.target.id}`;
    }
    if (privateEvent?.kind === 'item_used') {
      window.__lastItemUsed = structuredClone(privateEvent);
      const item = configuration.member_item_use?.[privateEvent.item_id];
      const restored = [];
      if (item?.cleanse_self_statuses) {
        restored.push('已净化状态');
      } else if (privateEvent.spirit_restored === undefined || item?.heal_hp !== undefined || item?.heal_max_hp_bps !== undefined) {
        restored.push(`恢复生命 ${privateEvent.hp_restored}`);
      }
      if (privateEvent.spirit_restored !== undefined) {
        restored.push(`恢复灵力 ${privateEvent.spirit_restored}`);
      }
      $('lastItemUse').textContent = restored.join('，');
    }
    if (privateEvent?.kind === 'projectile_recast') {
      lastRecast = privateEvent;
      window.__lastProjectileRecast = structuredClone(privateEvent);
    }
    if (['channel_pulse','channel_recovery_pulse','unit_channel_pulse'].includes(privateEvent?.kind)) {
      channelPulseCount++;
      channelSpiritCost += privateEvent.spirit_cost;
      window.__lastChannelPulse = structuredClone(privateEvent);
    }
    if (privateEvent?.kind === 'channel_ended') {
      lastChannelEnd = privateEvent;
      window.__lastChannelEnd = structuredClone(privateEvent);
    }
    window.__skillStats = structuredClone(skillStats);
    window.__hpCostStats = structuredClone(hpCostStats);
    window.__flightStats = structuredClone(flightStats);
    window.__areaStats = structuredClone(areaStats);
    window.__combatStats = structuredClone(combat);
    window.__shieldStats = structuredClone(shields);
    window.__periodicStats = structuredClone(periodic);
    if ($('periodicStats')) {
      $('periodicStats').textContent = `DOT ${periodic.dotTicks} 跳（发出 ${periodic.outgoingDotTicks} / 受击 ${periodic.incomingDotTicks}） · HOT ${periodic.hotTicks} 跳（自身 ${periodic.selfHotTicks} / 发出 ${periodic.outgoingAllyHotTicks} / 受到 ${periodic.incomingAllyHotTicks} / NPC ${periodic.npcHotTicks}） · 实际治疗 ${periodic.hpRestored}`;
    }
    window.__lastEvent = event;
    if (event.kind === 'problem') problem(event.code);
    if (event.kind === 'publication_expired') {
      statistics.expired = (statistics.expired ?? 0) + 1;
      connecting = true;
      $('resetCache').disabled = true;
      problem('显示同步已到期，结算以本人成员账户为准');
    }
    if (event.kind === 'closed') $('phase').textContent = '房间已关闭';
  });
  stream.onerror=()=>{$('phase').textContent='连接中断，正在恢复';};
}
function updateLedger(v){if(!v)return;$('ledger').textContent=`本人成员账户\n准入 ${v.admissions} 次 · 结算 ${v.settlements} 次\n补偿 ${v.compensations} 次\n`+Object.entries(v.balances??{}).map(([k,v])=>`${k}: ${v}`).join('\n');}
function draftDirection(charge) {
  if (!charge?.aim || chargeAimDraft?.actionId !== charge.action_id) return null;
  const direction = chargeAimDraft.direction;
  const [x, y] = charge.aim.direction;
  return Math.hypot(direction[0] - x, direction[1] - y) > 1e-11 ? direction : null;
}
function renderAllyTargets(skill) {
  const selecting = ['ally_heal','ally_shield'].includes(skill?.mechanism);
  $('allyTargetControl').hidden = !selecting;
  if (!selecting) return;
  const select = $('allyTarget');
  const allowSelf = configuration.ally_self_target_skills?.includes(skill.id);
  const targets = snapshot.members.filter(member =>
    (allowSelf || member.entity.id !== seat) && member.phase === 'active' && member.hp > 0 && !member.protected)
    .map(({entity: {id, generation}}) => ({id, generation}));
  const signature = JSON.stringify(targets);
  if (allyTargetSignature !== signature) {
    allyTargetSignature = signature;
    const selected = select.value;
    select.replaceChildren(new Option('请选择成员', ''), ...targets.map(target =>
      new Option(target.id === seat ? '自己' : target.id, JSON.stringify(target))));
    if (targets.some(target => JSON.stringify(target) === selected)) select.value = selected;
  }
  $('cast').disabled ||= !select.value;
}

function renderAttackTargets() {
  if (!(configuration.mechanism_version >= 54)) return;
  const select = $('attackTarget');
  $('attackTargetControl').hidden = false;
  const targets = snapshot.monsters.filter(npc => npc.hp > 0 && !npc.returning && npc.summon === undefined)
    .map(({entity: {id, generation}}) => ({id, generation}));
  const signature = JSON.stringify(targets);
  if (attackTargetSignature !== signature) {
    attackTargetSignature = signature;
    const selected = select.value;
    const options = targets.map(target => new Option(`${target.id} #${target.generation}`, JSON.stringify(target)));
    if (selected && !options.some(option => option.value === selected)) {
      const stale = JSON.parse(selected);
      const option = new Option(`${stale.id} #${stale.generation} 已失效`, selected);
      option.disabled = true;
      options.push(option);
    }
    select.replaceChildren(new Option('自动选择', ''), ...options);
    select.value = selected;
  }
  $('attack').disabled ||= select.selectedOptions[0]?.disabled === true;
}
function updateItemChoices(select, ids) {
  const signature = JSON.stringify(ids);
  if (signature === select.dataset.items) return;
  const selected = select.value;
  select.replaceChildren(...ids.map(id => new Option(id, id)));
  if (ids.includes(selected)) select.value = selected;
  select.dataset.items = signature;
}
function renderInventoryActions(inventory, active) {
  const ids = inventory.map(item => item.item_id);
  $('dropControl').hidden = !configuration.member_loot_drop;
  $('useItemControl').hidden = !configuration.member_item_use;
  if (configuration.member_loot_drop) {
    updateItemChoices($('dropItem'), ids);
    $('dropLoot').disabled = !active || !$('dropItem').value;
  }
  if (configuration.member_item_use) {
    updateItemChoices($('useItemChoice'), ids.filter(id => Object.hasOwn(configuration.member_item_use, id)));
    const itemId = $('useItemChoice').value;
    const deadline = snapshot.self.item_cooldowns?.find(entry => entry.item_id === itemId)?.ready_at_ms;
    const remaining = deadline ? deadline - snapshot.logical_ms : 0;
    $('useItem').disabled = !active || !itemId || remaining > 0;
    $('useItem').textContent = configuration.member_item_use[itemId]?.cleanse_self_statuses ? '使用净化道具' : '使用恢复道具';
    $('itemCooldown').textContent = remaining > 0 ? `冷却 ${remaining}ms` : '';
  }
}
function equippedSkills(){return snapshot?snapshot.self.player_skills.map(slot=>configuration.skills.find(skill=>skill.id===slot.skill_id)).filter(Boolean):[];}
let skillChoiceSignature=null;
function renderText(){if(!snapshot||!configuration)return;const available=equippedSkills(),signature=JSON.stringify(available.map(s=>s.id));if(skillChoiceSignature!==signature){const selected=$('skill').value;$('skill').replaceChildren(...available.map(s=>{const o=document.createElement('option');o.value=s.id;o.textContent=s.id;return o;}));if(available.some(s=>s.id===selected))$('skill').value=selected;skillChoiceSignature=signature;}
const s=snapshot.self;observeAreaSnapshot(areaStats,snapshot);observeFlightSnapshot(flightStats,snapshot);$('phase').textContent=labels[snapshot.phase]??snapshot.phase;$('own').textContent=`${seat} · ${labels[s.phase]??s.phase}\nHP ${s.hp} / ${s.max_hp}\n灵力 ${s.spirit} / ${s.max_spirit}\n攻击 ${s.effective_atk} · 防御 ${s.effective_def}\n逻辑时间 ${(snapshot.logical_ms/1000).toFixed(2)}s\n${s.protected?'保护中':'可操作'} · ${s.ready?'已就绪':'未就绪'}\n状态 ${s.statuses.length}`;
  $('own').textContent+=`\n准备技能：${s.pending_action?.skill_id??'无'}`;
  $('own').textContent+=`\n穿透提交 ${flightStats.ownStarted} · 实际接触 ${flightStats.ownContacts}\n`+snapshot.projectiles.filter(p=>p.piercing).map(p=>`${p.source.id} · 剩余接触 ${p.piercing.remaining_hits}/${p.piercing.max_hits} · 余程 ${p.piercing.remaining_range.toFixed(0)}`).join('\n');
  $('own').textContent+=`\n技能槽 ${snapshot.self.player_skills.length}${snapshot.self.skill_loadout_revision!==undefined?" · 版本 "+snapshot.self.skill_loadout_revision:""}\n`+snapshot.self.player_skills.map(s=>`${s.skill_id}: CD ${Math.max(0,s.ready_at_ms-snapshot.logical_ms)}ms · 提交 ${skillStats.commits[s.skill_id]??0}`).join('\n');
  $('own').textContent+=`\n实际生命成本 ${hpCostStats.total} · 提交 ${hpCostStats.commits} 次\n`+Object.entries(hpCostStats.bySkill).map(([id,v])=>`${id}: 本次 ${v.last} · 累计 ${v.total}`).join('\n');
  $('members').textContent='共享成员\n'+snapshot.members.map(m=>`${m.entity.id}: HP ${m.hp} · ${labels[m.phase]??m.phase}`).join('\n');
  $('own').textContent+=`\n本人护盾 ${s.shield_total??0} · 可见吸收 ${shields.visibleAbsorbed} · DOT 实际吸收 ${shields.dotAbsorbed}\nNPC 总盾：${snapshot.monsters.map(n=>n.entity.id+': '+n.shield_total).join(' / ')}`;
  for(const status of s.statuses.filter(row=>row.shield?.record))$('own').textContent+=`\n护盾记录 ${status.shield.record.recorded} / ${status.shield.record.cap} · ${status.source.id} → ${status.target.id}`;
  if(lastShieldRecord)$('own').textContent+=`\n盾结束 · 记录 ${lastShieldRecord.recorded} · 反击预算 ${lastShieldRecord.raw_counter} · 命中 ${lastShieldRecord.targets_hit}`;
  if(lastStatusConsumption)$('own').textContent+=`\n状态消费 ${lastStatusConsumption.definition_id} · ${lastStatusConsumption.consumed_stacks} 层 · 剩余 ${lastStatusConsumption.remaining_stacks}`;
  if(lastStatusTransfer)$('own').textContent+=`\n状态转移 ${lastStatusTransfer.status.definition_id} · ${lastStatusTransfer.previous_target.id} → ${lastStatusTransfer.status.target.id} · 原来源 ${lastStatusTransfer.status.source.id}`;
  if(lastDeathGuard)$('own').textContent+=`\n致死保护 · 阻止扣血 ${lastDeathGuard.prevented_hp_loss} · 恢复后生命 ${lastDeathGuard.hp}`;
  if(lastChainDamage)$('own').textContent+=`\n连锁跳跃 ${lastChainDamage.hops.length} 次 · 命中 ${lastChainDamage.hops.filter(h=>h.applied).length} · ${lastChainDamage.hops.map(h=>h.target.id).join(' → ')}`;
  if(lastCooldownReturn)$('own').textContent+=`\n冷却返还 ${lastCooldownReturn.changes.map(c=>`${c.skill_id} −${c.reduced_ms}ms`).join(' · ')}`;
  if(lastContactReaction)$('own').textContent+=`\n接触反应 · 来源 ${lastContactReaction.source.id} · 原始 ${lastContactReaction.raw_damage} · 生命 ${lastContactReaction.hp_damage} · 吸收 ${lastContactReaction.absorbed}`;
  if(lastFinisher)$('own').textContent+=`\n${lastFinisher.mode==='execute'?'终结尝试':'阈值追加'} · HP −${lastFinisher.hp_damage} · 吸收 ${lastFinisher.absorbed} · ${lastFinisher.lethal?'目标死亡':'目标存活'}`;
  if(lastActionTotal)$('own').textContent+=`\n动作汇总 ${lastActionTotal.trigger_id} · 总量 ${lastActionTotal.total_amount} · 接触 ${lastActionTotal.contacts} · ${lastActionTotal.fired?'已触发':'未触发'}`;
  for(const status of s.statuses.filter(row=>row.flags))$('own').textContent+=`\n复合标记 ${status.definition_id} · ${status.flags.join(' / ')}`;
  for(const npc of snapshot.monsters)for(const status of (npc.statuses??[]).filter(row=>row.flags))$('own').textContent+=`\n目标复合标记 ${npc.entity.id} · ${status.definition_id} · ${status.flags.join(' / ')}`;
  for(const status of s.statuses.filter(row=>row.stack_counter===true))$('own').textContent+=`\n层数 ${status.definition_id} · ${status.stacks} / ${status.max_stacks}`;
  for(const status of s.statuses.filter(row=>row.window))$('own').textContent+=`\n窗口记录 ${status.definition_id} · ${status.window.recorded} / ${status.window.cap} · 接触 ${status.window.contacts}`;
  if(lastPeriodicSpirit)$('own').textContent+=`\n周期回灵 ${lastPeriodicSpirit.target.id} · 基础 ${lastPeriodicSpirit.base_restored} / 转换 ${lastPeriodicSpirit.converted_spirit} · 消耗治疗 ${lastPeriodicSpirit.spent_heal} / 剩余 ${lastPeriodicSpirit.remaining_excess}`;
  if(lastStatusCleanse)$('own').textContent+=`\n状态净化 ${lastStatusCleanse.definition_id} · ${lastStatusCleanse.target.id} · ${lastStatusCleanse.phase} · 清理 ${lastStatusCleanse.removed_count}${lastStatusCleanse.suppressed?' · '+lastStatusCleanse.suppressed:''}`;
  if(lastWindowEnd)$('own').textContent+=`\n窗口结束 ${lastWindowEnd.definition_id} · ${lastWindowEnd.target.id} · 累计 ${lastWindowEnd.recorded} · 恢复生命 ${lastWindowEnd.hp_restored} / 灵力 ${lastWindowEnd.spirit_restored}`;
  $('own').textContent+='\n本人周期\n'+s.statuses.filter(v=>v.periodic).map(v=>`${v.definition_id}: ${v.source.id} → ${v.target.id} · ${v.periodic.ticks_done}/${v.periodic.total_ticks}`).join('\n')+'\nNPC 上本人成员来源 / NPC 自身状态\n'+snapshot.monsters.flatMap(n=>(n.statuses??[]).filter(v=>v.periodic).map(v=>`${n.entity.id}: ${v.source.id} · ${v.definition_id} ${v.periodic.ticks_done}/${v.periodic.total_ticks}`)).join('\n');
  $('inventory').textContent='本人背包\n'+s.inventory.map(i=>`${i.item_id}: ${i.quantity}`).join('\n');
  $('resetCache').disabled=s.phase==='terminal'||snapshot.phase==='closed'||!!statistics.expired;
  $('reconnect').disabled=s.phase==='terminal'||snapshot.phase==='closed'||!!statistics.expired;
  const active=!connecting&&snapshot.phase==='active'&&s.phase==='active'&&s.ready&&!s.protected;for(const id of ['attack','cast','selfCast','cancel','pickup','toExit','extract','stop','leave'])$(id).disabled=!active;
  renderInventoryActions(s.inventory, active);
  const selected = configuration.skills.find(skill => skill.id === $('skill').value);
  const reserved = s.pending_action?.resource_reservation;
  if(reserved) $('own').textContent+=`\n准备预留 · 灵力 ${reserved.spirit} · 生命 ${reserved.hp}`;
  const charge = s.pending_action?.charge;
  const releasing = charge && (charge.released_at_ms !== undefined || s.pending_action.commits_at_ms < charge.auto_commit_at_ms);
  const channel = s.active_channel;
  const recast = s.projectile_recast;
  $('recast').hidden = !recast;
  $('recast').disabled = !active || !recast || recastAimDraft?.actionId !== recast.action_id;
  if (recast) {
    $('own').textContent += `\n可再施放 · ${recast.projectile_id} · 剩余 ${recast.expires_at_ms - snapshot.logical_ms}ms`;
  }
  if (lastRecast) {
    $('own').textContent += `\n弹道已转向 · ${lastRecast.projectile_id} · 灵力 ${lastRecast.spirit_cost}`;
  }
  const followup = s.followup;
  const basicFollowup = s.basic_followup;
  const pursuit = s.basic_pursuit;
  if (configuration.basic_followup) {
    const available = !s.pending_action && !channel && !s.extraction_finishes_ms &&
      snapshot.logical_ms >= s.recovery_until_ms;
    $('attack').disabled = !active || !available;
    $('attack').textContent = available && snapshot.logical_ms < s.basic_ready_at_ms ? '排队普攻' : '普攻';
    $('own').textContent += `\n普攻提交 ${basicCommits} 次`;
    if (basicFollowup) {
      $('own').textContent += `\n已排队普攻 · ${basicFollowup.target.id} #${basicFollowup.target.generation}`;
      $('extract').disabled = true;
    }
  }
  const cooldown = s.player_skills.find(slot => slot.skill_id === selected?.id)?.ready_at_ms ?? 0;
  const recoveryCancel = s.recovery_cancel;
  const recoveryPolicy=configuration.skills.find(skill=>skill.id===recoveryCancel?.skill_id)?.recovery_cancel;
  const expandedRecovery=recoveryPolicy && (Object.hasOwn(recoveryPolicy,'by_move')||Object.hasOwn(recoveryPolicy,'by_explicit_action'));
  const cancelReady = recoveryCancel && snapshot.logical_ms >= recoveryCancel.cancel_at_ms && recoveryPolicy?.by_explicit_action !== false;
  const ordinary = selected?.mechanism === 'projectile' && !selected.charge && !selected.channel && !selected.recast;
  const takeOver = cancelReady && (expandedRecovery || ordinary && selected.id !== recoveryCancel.skill_id);
  const canReplace = takeOver && cooldown <= snapshot.logical_ms;
  const canQueue = !takeOver && followup && ordinary && selected.id !== followup.source_skill_id &&
    cooldown <= followup.promotes_at_ms &&
    followup.promotes_at_ms - snapshot.logical_ms <= configuration.skill_followup?.window_ms;
  $('cast').textContent = canReplace ? '接管施法' : canQueue ? '排队跟进' :
    selected?.charge ? '开始蓄力' : selected?.channel ? '开始引导' : '施放所选技能';
  $('cancel').textContent = pursuit ? '取消追击' : channel ? '取消引导' : cancelReady ? '取消后摇' :
    basicFollowup ? '取消排队普攻' : followup?.queued ? '取消跟进' : '取消准备';
  if (pursuit) {
    $('own').textContent += `\n追击 · ${pursuit.target.id} #${pursuit.target.generation} · 剩余 ${pursuit.expires_at_ms - snapshot.logical_ms}ms`;
    for (const id of ['attack', 'extract']) $(id).disabled = true;
  }
  if (recoveryCancel) {
    const wait = recoveryCancel.cancel_at_ms - snapshot.logical_ms;
    $('own').textContent += `\n后摇 · ${wait > 0 ? '距可取消 ' + wait + 'ms' : '可取消'}`;
  }
  if (followup) {
    $('own').textContent += `\n后摇跟进 · 距释放 ${followup.promotes_at_ms - snapshot.logical_ms}ms`;
    if (followup.queued) {
      $('own').textContent += `\n已排队 ${followup.queued.skill_id}`;
      $('extract').disabled = true;
    }
  }
  if (channel) {
    $('attack').disabled = true;
    $('extract').disabled = true;
  }
  if (channel) {
    $('own').textContent += `\n引导中 · ${channel.pulses} ${channel.recovery ? '跳' : '发'} · 剩余 ${channel.ends_at_ms - snapshot.logical_ms}ms`;
  }
  if (channel || lastChannelEnd) {
    $('own').textContent += `\n引导脉冲 ${channelPulseCount} · 周期灵力 ${channelSpiritCost}`;
  }
  if (lastChannelEnd) {
    $('own').textContent += `\n上次引导 ${channelEndLabels[lastChannelEnd.reason] ?? lastChannelEnd.reason}`;
  }
  $('updateChargeAim').hidden = !charge?.aim;
  $('updateChargeAim').disabled = !active || !!releasing || !draftDirection(charge);
  $('releaseCharge').hidden = !charge;
  $('releaseCharge').disabled = !active || !!releasing;
  if (charge) {
    const wait = Math.max(0, charge.min_release_at_ms - snapshot.logical_ms);
    const queuedRelease = configuration.skills.find(skill => skill.id === s.pending_action.skill_id)?.charge?.early_release?.kind === 'queue_until_min';
    $('releaseCharge').textContent = releasing ? '等待提交' : wait > 0 ? (queuedRelease ? '松手并等待提交' : '释放并取消') : '释放蓄力';
    $('own').textContent += `\n蓄力 ${snapshot.logical_ms - charge.started_at_ms}ms · ${charge.released_at_ms !== undefined ? '已松手，等待提交' : wait ? '最早释放剩余 ' + wait + 'ms' : '可释放'}`;
  }
  if (charge?.aim) {
    $('own').textContent += `\n权威方向 ${charge.aim.direction.map(value => value.toFixed(2)).join(', ')}`;
  }
  if (lastChargeCommit) {
    $('own').textContent += `\n上次蓄力 ${lastChargeCommit.held_ms}ms · 威力 ${lastChargeCommit.damage_factor_bps / 100}%`;
  }
  renderAttackTargets();
  $('cast').disabled = !selected || !active || !!s.pending_action || !!channel || !!pursuit ||
    (!canQueue && !canReplace && (snapshot.logical_ms < s.recovery_until_ms || snapshot.logical_ms < cooldown));$('selfCast').disabled=!active||!!channel||!!pursuit||!available.some(s=>s.mechanism==='self_status');
  renderAllyTargets(selected);
  const immunity = s.statuses.filter(status => hasStatusAttribute(status, 'control_immunity'))
    .reduce((end, status) => Math.max(end, status.expires_at_ms), 0);
  if (immunity > snapshot.logical_ms) {
    $('own').textContent += `\n控制免疫中 · 剩余 ${immunity - snapshot.logical_ms}ms`;
  }
  if (s.statuses.some(status =>
    hasStatusAttribute(status, 'first_control_immunity') && status.expires_at_ms > snapshot.logical_ms)) {
    $('own').textContent += '\n首控免疫可用';
  }
  const durationReduction = s.statuses.filter(status => status.attribute === 'control_duration_reduction')
    .reduce((end, status) => Math.max(end, status.expires_at_ms), 0);
  if (durationReduction > snapshot.logical_ms) {
    $('own').textContent += `\n控制时长减免中 · 剩余 ${durationReduction - snapshot.logical_ms}ms`;
  }
  if (s.statuses.some(status =>
    status.attribute === 'dodge_penalty' && status.expires_at_ms > snapshot.logical_ms)) {
    $('own').textContent += '\n闪避压制中';
  }
  if (s.statuses.some(status =>
    status.attribute === 'dodge_chance' && status.expires_at_ms > snapshot.logical_ms)) {
    $('own').textContent += '\n闪避率加成中';
  }
  if (s.statuses.some(status =>
    hasStatusAttribute(status, 'damage_immunity') && status.expires_at_ms > snapshot.logical_ms)) {
    $('own').textContent += '\n伤害免疫中';
  }
  if (s.statuses.some(status =>
    status.attribute === 'accuracy' && status.expires_at_ms > snapshot.logical_ms)) {
    $('own').textContent += '\n命中加成中';
  }
  if(s.skill_powers)$('own').textContent+='\n技能威力 '+Object.entries(s.skill_powers).map(([id,p])=>id+':'+p).join(' · ');
  if(s.statuses.some(status=>status.attribute==='life_steal'&&status.expires_at_ms>snapshot.logical_ms))$('own').textContent+='\n吸血状态生效中';
  if (s.statuses.some(status =>
    status.attribute === 'skill_power_bonus' && status.expires_at_ms > snapshot.logical_ms)) {
    $('own').textContent += '\n技能威力加成中';
  }
  if (s.statuses.some(status =>
    status.attribute === 'critical_damage_bonus' && status.expires_at_ms > snapshot.logical_ms)) {
    $('own').textContent += '\n暴伤加成中';
  }
  if (s.statuses.some(status =>
    status.attribute === 'critical_chance' && status.expires_at_ms > snapshot.logical_ms)) {
    $('own').textContent += '\n暴击率加成中';
  }
  if (s.statuses.some(status =>
    status.attribute === 'armor_penetration' && status.expires_at_ms > snapshot.logical_ms)) {
    $('own').textContent += '\n破甲中';
  }
  if (s.statuses.some(status =>
    status.attribute === 'cooldown_reduction' && status.expires_at_ms > snapshot.logical_ms)) {
    $('own').textContent += '\n冷却缩减中';
  }
  if (s.statuses.some(status =>
    status.attribute === 'movement_haste' && status.expires_at_ms > snapshot.logical_ms)) {
    $('own').textContent += '\n加速中';
  }
  if (s.statuses.some(status =>
    status.attribute === 'movement_slow' && status.expires_at_ms > snapshot.logical_ms)) {
    $('own').textContent += '\n减速中';
  }
  const stun = s.statuses.filter(status => hasStatusAttribute(status, 'stun'))
    .reduce((end, status) => Math.max(end, status.expires_at_ms), 0);
  if (stun > snapshot.logical_ms) {
    $('own').textContent += `\n眩晕中 · 剩余 ${stun - snapshot.logical_ms}ms`;
    for (const id of ['attack', 'cast', 'selfCast', 'recast', 'updateChargeAim', 'releaseCharge', 'toExit', 'extract']) {
      $(id).disabled = true;
    }
  }
  const root = s.statuses.filter(status => hasStatusAttribute(status, 'root'))
    .reduce((end, status) => Math.max(end, status.expires_at_ms), 0);
  if (root > snapshot.logical_ms) {
    $('own').textContent += `\n定身中 · 剩余 ${root - snapshot.logical_ms}ms`;
    $('toExit').disabled = true;
  }
  const silence = s.statuses.filter(status => hasStatusAttribute(status, 'silence'))
    .reduce((end, status) => Math.max(end, status.expires_at_ms), 0);
  if (silence > snapshot.logical_ms) {
    $('own').textContent += `\n沉默中 · 剩余 ${silence - snapshot.logical_ms}ms`;
    for (const id of ['cast', 'selfCast', 'recast', 'updateChargeAim', 'releaseCharge']) $(id).disabled = true;
  }
}
function nearest(){return snapshot?.monsters.filter(n=>n.hp>0&&n.summon===undefined).sort((a,b)=>Math.hypot(...a.position.map((v,i)=>v-snapshot.self.position[i]))-Math.hypot(...b.position.map((v,i)=>v-snapshot.self.position[i])))[0];}
$('attackTarget').onchange = renderText;
$('attack').onclick = () => {
  const selected = $('attackTarget').value;
  const action = selected ? {kind: 'attack_unit', target: JSON.parse(selected)} : {kind: 'attack'};
  input(action).catch(() => {});
};
$('selectGround').onclick=()=>{selectingGround=true;$('groundPoint').textContent='请在地图上选择目标点点';};
$('skill').onchange=renderText;
$('allyTarget').onchange=renderText;
$('cast').onclick = () => {
  try {
    const skill = configuration.skills.find(s => s.id === $('skill').value);
    if (!skill) return;
    const selectedAlly = $('allyTarget').value;
    const action = skillAction(skill, snapshot, aim, selectedAlly ? JSON.parse(selectedAlly) : null);
    if (skill.charge) action.kind = 'start_charge';
    input(action).catch(() => {});
  } catch (error) {
    problem(error);
  }
};
$('updateChargeAim').onclick = () => {
  const charge = snapshot?.self.pending_action?.charge;
  const direction = draftDirection(charge);
  if (direction) input({kind: 'update_charge_aim', action_id: charge.action_id, direction}).catch(() => {});
};
$('recast').onclick = () => {
  const recast = snapshot?.self.projectile_recast;
  if (!recast || recastAimDraft?.actionId !== recast.action_id) return;
  const shot = snapshot.projectiles.find(projectile => projectile.id === recast.projectile_id);
  if (!shot) return;
  const direction = recastAimDraft.point.map((value, i) => value - shot.position[i]);
  input({kind: 'recast_projectile', action_id: recast.action_id, recast_index: recast.next_index, direction}).catch(() => {});
};
$('releaseCharge').onclick = () => {
  const charge = snapshot?.self.pending_action?.charge;
  if (!charge) return;
  const direction = draftDirection(charge);
  const action = {kind: 'release_charge', action_id: charge.action_id};
  if (direction) action.direction = direction;
  input(action).catch(() => {});
};
$('selfCast').onclick=()=>{const skill=equippedSkills().find(s=>s.mechanism==='self_status');if(skill)input({kind:'cast_self',skill_id:skill.id}).catch(()=>{});};
$('cancel').onclick=()=>input({kind:'cancel_action'}).catch(()=>{});$('stop').onclick=()=>input({kind:'stop'}).catch(()=>{});
$('toExit').onclick=()=>input({kind:'navigate',destination:snapshot.extraction_point}).catch(()=>{});
$('resetCache').onclick=()=>{replica.forgetBaseline();connecting=true;if(snapshot)renderText();};
$('extract').onclick=()=>input({kind:'extract_start'}).catch(()=>{});$('leave').onclick=()=>input({kind:'leave'}).catch(()=>{});$('reconnect').onclick=connect;
$('useItemChoice').onchange = renderText;
$('useItem').onclick = () => {
  input({kind: 'use_item', item_id: $('useItemChoice').value,
    expected_revision: snapshot.self.inventory_revision}).catch(() => {});
};
$('dropLoot').onclick = () => {
  const itemId = $('dropItem').value;
  const quantity = $('dropQuantity').valueAsNumber;
  if (!Number.isInteger(quantity) || quantity <= 0 || quantity > 4294967295) {
    problem('请输入有效的正整数数量');
    return;
  }
  input({kind: 'drop_item', item_id: itemId, quantity,
    expected_revision: snapshot.self.inventory_revision}).catch(() => {});
};
$('pickup').onclick=()=>{const r=snapshot.resources.filter(r=>!r.owner||r.owner.id===seat).sort((a,b)=>Math.hypot(...a.position.map((v,i)=>v-snapshot.self.position[i]))-Math.hypot(...b.position.map((v,i)=>v-snapshot.self.position[i])))[0];if(r)input({kind:'pickup',resource_id:r.id,expected_revision:snapshot.self.inventory_revision}).catch(()=>{});};
const keys=new Set(),directions={w:[-1,-1],s:[1,1],a:[-1,1],d:[1,-1],ArrowUp:[-1,-1],ArrowDown:[1,1],ArrowLeft:[-1,1],ArrowRight:[1,-1]};
function movementBlocked() {
  return snapshot?.self.statuses.some(status =>
    (hasStatusAttribute(status, 'root') || hasStatusAttribute(status, 'stun')) && status.expires_at_ms > snapshot.logical_ms);
}
function movement(){if(connecting||!snapshot?.self.ready||movementBlocked())return;let direction=[0,0];for(const k of keys){direction[0]+=directions[k][0];direction[1]+=directions[k][1];}input({kind:'move',direction}).catch(()=>{});}
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

  const chargeAim = snapshot.self.pending_action?.charge?.aim ?? snapshot.self.active_channel;
  if (chargeAim) {
    const [x, y] = project(snapshot.self.position);
    const [dx, dy] = project(chargeAim.direction.map(value => value * 160));
    ctx.beginPath();
    ctx.moveTo(x + g.ox, y + g.oy);
    ctx.lineTo(x + g.ox + dx, y + g.oy + dy);
    ctx.strokeStyle = '#ffd67d';
    ctx.lineWidth = 3;
    ctx.stroke();
  }
  point(snapshot.extraction_point,'#68bdab',20,'撤离点');for(const r of snapshot.resources)point(r.position,r.item_id?'#e9c967':'#93d9f2',5,r.item_id?`${r.quantity}`:'光球');
  const actors=[...snapshot.monsters.filter(m=>m.hp>0).map(m=>({p:m.position,color:'#dd806c',label:`${m.entity.id} ${m.hp}`})),...snapshot.members.map(m=>({p:m.position,color:m.entity.id===seat?'#84d4cf':'#80acd7',label:`${m.entity.id} ${m.hp}`}))];actors.sort((a,b)=>a.p[0]+a.p[1]-b.p[0]-b.p[1]);for(const a of actors)point(a.p,a.color,10,a.label);for(const p of snapshot.projectiles){point(p.position,p.piercing?'#72e0ca':p.source.kind==='participant'?'#e5d197':'#e29ea9',p.piercing?7:4,p.piercing?String(p.piercing.remaining_hits):'');if(p.piercing)window.__flightRendered={id:p.id,source:structuredClone(p.source),piercing:structuredClone(p.piercing)};}
}
canvas.addEventListener('pointerdown', event => {
  if (connecting || !snapshot?.self.ready) return;
  const rect = canvas.getBoundingClientRect();
  const g = geometry();
  const dx = event.clientX - rect.left - g.ox;
  const dy = event.clientY - rect.top - g.oy;
  const destination = [dx + 2 * dy, 2 * dy - dx];
  if (selectingGround) {
    aim = destination;
    selectingGround = false;
    $('groundPoint').textContent = `方向点 ${aim.map(value => value.toFixed(0)).join(', ')}`;
    const recast = snapshot.self.projectile_recast;
    if (recast) {
      recastAimDraft = {actionId: recast.action_id, point: destination};
      renderText();
    }
    const charge = snapshot.self.pending_action?.charge;
    if (charge?.aim) {
      const direction = destination.map((value, i) => value - snapshot.self.position[i]);
      const length = Math.hypot(...direction);
      chargeAimDraft = length > 1e-9
        ? {actionId: charge.action_id, direction: direction.map(value => value / length)}
        : null;
      renderText();
    }
    return;
  }
  if (!movementBlocked()) input({kind: 'navigate', destination}).catch(() => {});
});
new ResizeObserver(()=>dirty=true).observe(canvas);requestAnimationFrame(draw);launch().catch(problem);
