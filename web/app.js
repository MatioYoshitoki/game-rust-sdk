import { project, unproject, corners, createViewport } from './isometric.js';

const $ = id => document.getElementById(id);
const state = {
  selectedGround:null, selectedUnit: null, unitButtonsKey: null, booting: true, config: null, session: null, source: null, snapshot: null, phase: 'idle', sequence: 0,
  ledger: { warehouse: 0, cultivation: 0, admissions: 0, settlements: 0, pending: [], manual: 0 },
  events: [], logs: [], seen: new Set(), paused: false, focused: !document.hidden,
  handshake: false, starting: false, retiringSession: null, lastInput: null,
  lastSettlement: null, worldLive: null, floating: [], pulse: null, pointer: null,
  poses: {}, snapshotAt: null, syncGap: null,
  online: false, reconnecting: false, replaced: false, unloading: false,
  joinToken: null, joinPromise: null,
};
let reconnectJob = null;
let requests = Promise.resolve();
let acknowledgements = Promise.resolve();
let hudDirty = true;
let toastTimer;
const phaseNames = { idle: '尚未进入', preparing: '准入准备', deploying: '等待就绪', active: '对局进行中', draining: '结算处理中', closed: '已关闭回收', faulted: '故障封存' };
const messages = {
  INVALID_TARGET: '目标身份已失效，请重新选择存活的敌人。',
  INVALID_STATE: '当前状态不允许该操作：请检查开放时间、冷却或动作阶段。',
  NOT_READY: '角色仍受保护，恢复前台并就绪后才能操作。',
  OUT_OF_RANGE: '距离过远或被障碍阻挡，请靠近后重试。',
  NO_RESOURCE: '目标、物品或所需灵力不足。', REVISION_CONFLICT: '背包状态已变化，请按最新状态重试。',
  CLOSED: '此对局已经关闭，可以重新进入一局。', UNRESOLVED_IO: '上一局仍有未决资产，请先点击核对再创建新局。',
  QUEUE_FULL: '输入队列繁忙，请稍后重试。', STALE_PUBLICATION: '该快照已过期，等待新的完整状态。',
  KERNEL_EXITED: '内核进程已退出，请重新进入或查看本地服务。',
  STALE_CONNECTION: '连接已被更新，此页面已停止操作。',
};

async function api(path, body) {
  let result;
  try { result = await fetch(path, body === undefined ? { cache: 'no-store', signal: AbortSignal.timeout(5000) } : {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body),
    signal: AbortSignal.timeout(5000),
  }); } catch (error) {
    if (state.session && state.online && !state.starting && !state.unloading && !state.replaced && !state.reconnecting) recover(state.session);
    throw error;
  }
  const data = await result.json();
  if (!result.ok) { const error = new Error(data.error || 'NETWORK_ERROR'); error.code = data.error; throw error; }
  return data;
}
function ordered(path, body) {
  const result = requests.then(() => api(path, body)); requests = result.catch(() => {}); return result;
}
function acknowledge(sessionId, outputId) {
  const connection = { ...state.session.connection };
  // Keep receipt traffic in order without putting keyboard input behind frame ACKs.
  const result = acknowledgements.then(() => {
    if (!state.online || state.session?.id !== sessionId || state.retiringSession === sessionId || !sameConnection(connection, state.session.connection)) return { ignored: true };
    return api(`/api/sessions/${sessionId}/ack`, { outputId, connection });
  });
  acknowledgements = result.catch(() => {}); return result;
}
function pose(name, now) {
  const segment = state.poses[name];
  if (!segment) return null;
  const t = segment.duration ? Math.min(1, Math.max(0, (now - segment.at) / segment.duration)) : 1;
  return segment.from.map((value, i) => value + (segment.to[i] - value) * t);
}
function applySnapshot(snapshot) {
  if (state.selectedUnit && !(snapshot.pve?.monsters??[]).some(m=>sameUnit(m.unit_target,state.selectedUnit))) state.selectedUnit=null;

  const now = performance.now(); const previous = state.snapshot;
  if (state.snapshotAt !== null) {
    const gap = now - state.snapshotAt;
    if (gap > 5) state.syncGap = state.syncGap === null ? gap : state.syncGap * .8 + gap * .2;
  }
  const smooth = state.session?.clock === 'live' && previous
    && previous.phase === snapshot.phase && previous.protected === snapshot.protected;
  const duration = smooth ? Math.max(16, Math.min(60, snapshot.logical_ms - previous.logical_ms)) : 0;
  for (const [name, point] of [['player', snapshot.position], ['target', snapshot.target?.position]]) {
    if (!point) { delete state.poses[name]; continue; }
    const segment = state.poses[name];
    if (segment && point.every((value, i) => value === segment.to[i])) continue;
    const from = pose(name, now) ?? point;
    const continuous = Math.hypot(point[0] - from[0], point[1] - from[1]) < 128;
    state.poses[name] = { from, to: [...point], at: now, duration: continuous ? duration : 0 };
  }
  // Rendering interpolates received positions only. Inputs and all rules use this raw snapshot.
  state.snapshot = snapshot; state.snapshotAt = now;
}
function toast(text, error = false) {
  clearTimeout(toastTimer); $('toast').textContent = text; $('toast').classList.toggle('error', error);
  $('toast').classList.remove('hidden'); toastTimer = setTimeout(() => $('toast').classList.add('hidden'), 4500);
}
function fail(error) {
  if ((state.reconnecting || !state.online) && ['TypeError', 'TimeoutError', 'AbortError'].includes(error.name)) return;
  if (state.replaced && error.message === 'STALE_CONNECTION') return;
  const guardianMessage=({ENCOUNTER_LOCKED:'请先完成绑定遭遇的指定目标。',GUARDIAN_PENDING:'守护者待生成，暂不能交互。',GUARDIAN_ALIVE:'请先击败本容器的全部守护者。'})[error.message];
  log('拒绝', `${guardianMessage||messages[error.message] || '操作未完成。'} [${error.message}]`, 'error'); toast(guardianMessage||messages[error.message] || error.message, true);
}
function time(ms = 0, precision = false) {
  const seconds = Math.max(0, ms) / 1000;
  return `${String(Math.floor(seconds / 60)).padStart(2, '0')}:${String(Math.floor(seconds % 60)).padStart(2, '0')}${precision ? `.${String(Math.floor((ms % 1000) / 10)).padStart(2, '0')}` : ''}`;
}
function log(type, text, level = '') {
  state.logs.push({ time: state.snapshot?.logical_ms ?? 0, type, text, level });
  if (state.logs.length > 120) state.logs.shift();
  const element = $('event-log'); const follow = element.scrollHeight - element.scrollTop - element.clientHeight < 35;
  element.replaceChildren(...state.logs.map(entry => {
    const row = document.createElement('div'); row.className = `log-row ${entry.level}`;
    for (const [name, value] of [['log-time', time(entry.time, true)], ['log-type', entry.type], ['log-text', entry.text]]) {
      const span = document.createElement('span'); span.className = name; span.textContent = value; row.append(span);
    }
    return row;
  }));
  if (follow) element.scrollTop = element.scrollHeight;
  $('event-count').textContent = `${state.events.length} 条内核事件`;
}

function input(action) {
  return { requestId: `web-${crypto.randomUUID()}`, sequence: ++state.sequence, action, connection: { ...state.session.connection } };
}
async function action(payload) {
  if (!state.session) throw new Error('NOT_READY');
  const id = state.session.id;
  const command = input(payload); state.lastInput = command;
  const result = await ordered(`/api/sessions/${id}/input`, command);
  return result;
}
function sameUnit(a,b){return a && b && a.id===b.id && a.generation===b.generation;}
function selectUnit(target){
  if(!(state.snapshot?.pve?.monsters??[]).some(m=>sameUnit(m.unit_target,target)))return;
  state.selectedUnit={...target};render();
}
function renderUnitTargets(active){
  const enabled=state.config?.playerSkills?.some(s=>s.mechanism==='unit_projectile');
  $('unit-target-panel').hidden=!enabled||['closed','faulted'].includes(state.phase);
  if(!enabled)return;
  const selected=state.selectedUnit;
  $('unit-target-selected').textContent=selected?`已选 ${selected.id} · 第${selected.generation}代`:'请选择目标：点击敌人或下方目标按钮';
  const targets=(state.snapshot?.pve?.monsters??[]).filter(m=>m.unit_target);
  const key=JSON.stringify([active,selected,targets.map(m=>m.unit_target)]);
  if(key===state.unitButtonsKey)return;state.unitButtonsKey=key;
  $('unit-target-list').replaceChildren(...targets.map(m=>{const b=document.createElement('button');b.textContent=`${m.id} · 第${m.unit_target.generation}代`;b.dataset.targetId=m.id;b.setAttribute('aria-pressed',String(Boolean(sameUnit(selected,m.unit_target))));b.disabled=!active;b.onclick=()=>selectUnit(m.unit_target);return b;}));
}
function isDirectionSkill(s){return s?.mechanism==='piercing_projectile'||s?.mechanism==='line_field';}
function isGroundSkill(s){return s?.mechanism==='ground_circle'||s?.mechanism==='ground_field';}
function renderGroundTarget(active){
  const piercing=state.config?.playerSkills?.some(isDirectionSkill);
  $('piercing-panel').hidden=!piercing||['closed','faulted'].includes(state.phase);
  $('piercing-aim').textContent=state.pointer?`已设方向：朝向 (${state.pointer.map(v=>v.toFixed(1)).join(', ')})`:'点击或触摸地图设置发射方向';
  $('piercing-summary').textContent=(state.snapshot?.pve?.piercing_projectiles??[]).map(p=>`${p.skill_id} · ${p.repeat_interval_ms===undefined?'':p.repeat_interval_ms===0?'每身份一次 · ':'重复间隔 '+(p.repeat_interval_ms/1000)+'秒 · '}预算 ${p.remaining_hits}/${p.max_hits} · 历史 ${p.contacts.length}：${p.contacts.map(h=>h.target.id+'@'+h.target.generation+(p.repeat_interval_ms===undefined?'':' / 最新 '+(h.hit_at_ms/1000).toFixed(2)+'秒')+(h.next_eligible_at_ms===undefined?'':' / 下次资格 '+(h.next_eligible_at_ms/1000).toFixed(2)+'秒')).join('、')} · 剩余射程 ${p.remaining_range.toFixed(1)}`).join('；');
  const enabled=state.config?.playerSkills?.some(isGroundSkill);
  $('ground-target-panel').hidden=!enabled||['closed','faulted'].includes(state.phase);
  const p=state.selectedGround;
  $('ground-target-selected').textContent=p?`地面落点 (${p[0].toFixed(1)}, ${p[1].toFixed(1)})`:'点击或触摸地图选择地面落点';
  $('ground-target-panel').setAttribute('aria-disabled',String(!active));
  const fields=(state.snapshot?.pve?.player_fields??[]).filter(f=>f.contact_policy);const summary=$('field-contact-summary');summary.hidden=!fields.length;
  summary.textContent=fields.map(f=>{const p=f.contact_policy;return `${f.skill_id}${f.line?' · 线形 '+Math.hypot(f.line.end[0]-f.line.start[0],f.line.end[1]-f.line.start[1]).toFixed(1)+' × '+f.line.width:''} · 预算 ${p.remaining_hits}/${p.max_total_hits} · 每周期 ${p.max_targets_per_tick} · ${p.repeat_interval_ms===0?'每身份一次':'重复间隔 '+(p.repeat_interval_ms/1000)+'秒'} · 历史 ${p.contacts.length}：${p.contacts.slice(0,4).map(h=>h.target.id+'@'+h.target.generation).join('、')}`;}).join('；');
}
function skillIntent(skill, direction) {
  if(isDirectionSkill(skill) && !state.pointer){toast('请先点选发射方向');return null;}
  if(isGroundSkill(skill)){
    if(!state.selectedGround){toast('请先点选地面落点');return null;}
    return {kind:'cast_ground',skill_id:skill.id,position:[...state.selectedGround]};
  }
  if(skill.mechanism==='unit_projectile'){
    if(!state.selectedUnit){toast('请先选择目标');return null;}
    return {kind:'cast_unit',skill_id:skill.id,target:{...state.selectedUnit}};
  }

  return skill.mechanism === 'self_status' ? {kind:'cast_self',skill_id:skill.id} : {kind:'cast',skill_id:skill.id,direction};
}
function userAction(payload) { if (payload && !state.starting && state.online && !state.replaced && !state.reconnecting) action(payload).catch(fail); }

function event(output) {
  const item = output.body.event;
  state.events.push(item); if (state.events.length > 512) state.events.shift();
  switch (item.kind) {
    case 'piercing_started':log('穿透发射',`${item.projectile.skill_id} · 预算 ${item.projectile.max_hits}`);break;
    case 'piercing_hit':log('穿透命中',`${item.target.id}@${item.target.generation} · 剩余 ${item.remaining_hits}${item.hit_at_ms===undefined?'':' · 接触 '+(item.hit_at_ms/1000).toFixed(2)+'秒'}`);break;
    case 'piercing_ended':log('穿透结束',`${item.action_ids.length}发 · ${item.reason}`);break;
    case 'player_field_started': log('区域创建', `${item.field.skill_id} · ${item.field.total_ticks}个周期`);break;
    case 'player_field_tick': log('区域周期', `${item.skill_id} · 第${item.tick_index}次 · 命中 ${item.hits}${item.remaining_hits==null?'':' · 剩余预算 '+item.remaining_hits}`);break;
    case 'player_field_ended': log('区域结束', item.reason==='hit_budget_exhausted'?'命中预算耗尽':item.reason);break;
    case 'player_area_started': log('地面预警', `${item.warning.skill_id} · (${item.warning.center.map(v=>v.toFixed(1)).join(', ')})`);break;
    case 'player_area_resolved': log('范围结算', `${item.skill_id} · 命中 ${item.hits}`);break;
    case 'player_area_cancelled': log('预警取消', item.reason);break;
    case 'resource_stolen': log('资源夺取', `生命 ${item.hp_removed} → 实回 ${item.hp_restored} · 灵力 ${item.spirit_removed} → 实回 ${item.spirit_restored} · 余量 ${item.hp_excess}/${item.spirit_excess}`); break;
    case 'heal_resolved': log('治疗结算', `${item.heal_kind} · 基础 ${item.raw_heal} · 倍率 ${item.multiplier_bps/100}% · 调整 ${item.adjusted_heal} · 实回 ${item.hp_restored}`); break;
    case 'periodic_heal_tick': log('周期治疗', `${item.definition_id} · 第${item.tick_index}跳 · 请求 ${item.raw_heal} · 实回 ${item.hp_restored}${item.outcome==='applied'?'':` · 跳过 ${item.outcome}`}`); break;
    case 'periodic_tick': log('周期伤害', `${item.definition_id} · 第${item.tick_index}跳 · 护盾吸收 ${item.absorbed} · 扣血 ${item.hp_damage}${item.outcome==='applied'?'':` · 跳过 ${item.outcome}`}`); break;
    case 'shield_granted': log('护盾授予', `${item.definition_id} · 新增 ${item.granted} · 剩余 ${item.remaining}`); break;
    case 'shield_absorbed': log('护盾吸收', `${item.target.id} 的护盾吸收 ${item.amount} 点伤害`); break;
    case 'io_timed_out': log('业务超时', `${item.operation_id.split(':').at(-1)} · 尝试 ${item.attempt_id} 未按期回执，Rust 正在处理冻结责任。`, 'warn'); break;
    case 'io_drain_expired': log('回收', '结算等待期限已结束，World 已回收，未决责任继续保留。', 'warn'); break;
    case 'connection_changed': log('重连', `连接已更新到第 ${item.generation} 代，等待全量就绪。`); break;
    case 'connection_lost': log('断线', '角色已停止移动并进入保护，等待重新连接。', 'warn'); break;
    case 'connection_expired': log('超时', '重连保留时间已结束，内核执行结算与回收。', 'warn'); break;
    case 'joined': log('加入', '角色已建立，等待完整快照和就绪确认。'); break;
    case 'activated': log('激活', '对局已激活，撤离与探索计时从现在开始。'); break;
    case 'action_committed':
      log('动作', item.skill ? `${item.skill_id ?? state.config.skill.id} 已提交，灵力 −${item.spirit_cost}。` : '普攻已提交，弹道由内核结算。');
      state.pulse = { at: performance.now(), skill: item.skill }; break;
    case 'action_rejected': log('拒绝', `动作未提交：${messages[item.reason]??item.reason}`, 'warn'); break;
    case 'action_interrupted': log('中断', '动作在提交前被中断，预留资源已释放。', 'warn'); break;
    case 'damage': {
      const target = item.target === 'participant' ? '你' : '验证目标';
      log('伤害', `${target}受到 ${item.amount} 点实际伤害。`);
      state.floating.push({ position: item.target === 'participant' ? state.snapshot?.position ?? [0, 0] : state.snapshot?.target?.position ?? [90, 0], amount: item.amount, at: performance.now(), own: item.target === 'participant' });
      break;
    }
    case 'target_killed': log('击杀', '目标死亡，内核按配置处理击杀与奖励。'); break;
    case 'timed_position_started': {
      const seconds = (item.warning.resolves_at_ms - item.warning.started_at_ms) / 1000;
      if (item.warning.miss_heal != null) {
        log('破冰窗口', `请在 ${seconds} 秒内发起攻击并命中核心`);
        break;
      }
      const destination = item.warning.site ? `进入${item.warning.site.name}` : '靠近守卫';
      log('站位预警', `请在 ${seconds} 秒内${destination}`);
      break;
    }
    case 'timed_position_resolved':
      if (item.spirit_restored != null) {
        log('镇守成功', `阻止脉冲，回复 ${item.spirit_restored} 点灵力`);
      } else {
        const failure = state.config?.pve?.timed_position?.success_spirit_restore_bps != null ? '脉冲' : '反噬';
        log(item.target_id ? (item.site_id ? '承星成功' : '导雷成功') : failure,
          `${item.target_id ?? '你'}受到 ${item.damage} 点实际伤害`);
      }
      break;
    case 'timed_position_cancelled': log('窗口取消', '本次限时判定已取消'); break;
    case 'timed_hit_triggered': log('破冰成功', `核心受到 ${item.damage} 点追加伤害，本轮回冻已阻止`); break;
    case 'timed_hit_missed': log('回冻', `核心回复 ${item.healed} 点生命`); break;
    case 'heavy_started':log('重击预警',`${item.monster_id} · 请整体移出危险轨迹`);break;
    case 'heavy_committed':log('重击释放',`${item.cast.monster_id} · 沿固定方向推进`);break;
    case 'heavy_resolved':log('重击结束',item.contacted?`命中 · ${item.damage}点伤害`:item.blocked?'轨迹被地形阻挡':item.counter_awarded?'移动躲开成功 · 获得借隙机会':'轨迹结束');break;
    case 'heavy_cancelled':log('重击／机会结束',`${item.casts.length}个重击 · ${item.reason}`);break;
    case 'counter_opened':log('反击',`${item.window.monster_id} · 窗口内主动普攻或技能命中`);break;
    case 'counter_triggered':log('反击',`${item.cast.monster_id} · 追加${item.damage}点实际伤害`);break;
    case 'support_started':log('支援准备',`${item.monster_id} → ${item.target_id}`);break;
    case 'support_pulse':log('支援治疗',`${item.monster_id} → ${item.target_id} · 第${item.tick}跳 +${item.amount}`);break;
    case 'support_completed':log('支援结束',`${item.monster_id} · 本次治疗已完成`);break;
    case 'support_interrupted':log('打断成功',`${item.cast.monster_id} · ${item.cast.committed?'已生效治疗保留，停止后续回复':'准备已取消，未扣治疗成本'}`);break;
    case 'support_cancelled':log('支援取消',`${item.casts.length}个动作 · ${item.reason}`);break;
    case 'attunement_started':log('调谐',`${item.objective_id} · 读条成功后扣材料`);break;
    case 'attunement_interrupted':log('调谐中断',`${unlockReason(item.reason)} · 本次未扣材料`,'warn');break;
    case 'attunement_completed':log('调谐完成',item.consumed.map(r=>`${r.item_id} -${r.quantity}`).join(' / ')||'免费调谐');break;
    case 'chest_opened': log('开箱', `${item.chest_id} · 第${item.generation}轮 · 生成 ${item.resource_ids.length}份地面掉落，拾取后入背包。`); break;
    case 'chest_respawned': log('重生', `${item.chest_id} · 第${item.generation}轮已可开启。`); break;
    case 'chest_unlock_started': log('破解',`${item.chest_id} · 开始/继续破解`);break;
    case 'chest_unlock_interrupted': log('破解中断',`${item.chest_id} · ${unlockReason(item.reason)}`,'warn');break;
    case 'chest_layer_completed': log('解除封印',`${item.chest_id} · 已完成 ${item.completed_layers} 层`);break;
    case 'chest_unlocked': log('解封完成',`${item.chest_id} · 可以开启领取地面掉落`);break;
    case 'encounter_wave_scheduled':log('下一波',`${item.encounter_id} · 第${item.wave_index}波已安排，等待间隔结束`);break;
    case 'encounter_wave_started':log('波次开始',`${item.encounter_id} · 第${item.wave_index}波 · ${item.monster_ids.length}名敌人`);break;
    case 'encounter_wave_spawn_blocked':log('本波受阻',`${item.encounter_id} · 第${item.wave_index}波 · 请让出落点`,'warn');break;
    case 'encounter_wave_completed':log('波次完成',`${item.encounter_id} · 第${item.wave_index}波目标已完成`);break;
    case 'boss_phase_changed':log('守将阶段',`${item.monster_id} · 第${item.phase_index}阶段 ${item.phase_name} · 第${item.epoch}轮`);break;
    case 'area_started':log('范围预警',`${item.monster_id} · ${item.skill_id} · 落点已锁定，${((item.resolves_at_ms-item.started_at_ms)/1000).toFixed(1)}秒后判定`);break;
    case 'area_resolved':log('范围判定',`${item.monster_id} · ${item.hit?`命中，实际伤害 ${item.damage}`:'未命中'}`);break;
    case 'area_cancelled':log('预警取消',`${item.casts.length}个预警 · ${({background:'进入保护',returning:'施法者归位',owner_died:'施法者死亡',closed:'对局结束',replaced:'守护代替换'})[item.reason]??item.reason}`);break;
    case 'kill_scored':log('击杀积分',`${item.monster_id} · +${item.score}分 · 累计${item.total_score}分 · 实际击杀${item.kills}`);break;
    case 'boss_reset':log('守将归位',`${item.monster_id} · 已归位，恢复${item.phase_name}阶段 · 第${item.epoch}轮`);break;
    case 'encounter_reset_scheduled':log('脱战计时',`${item.encounter_id} · 第${item.round}轮 · 回到追击范围可继续本轮`);break;
    case 'encounter_reset_cancelled':log('复位计时结束',item.encounter_ids.join('、'));break;
    case 'encounter_reset':log('整组复位',`${item.encounter_id} · 第${item.round}轮 · 已得收益保留，重复击杀不重复发奖`);break;
    case 'encounter_triggered':log('遭遇触发',`${item.encounter_id} · 已进入触发区域`);break;
    case 'encounter_spawn_blocked':log('生成受阻',`${item.encounter_id} · 请让出落点，等待整组生成`,'warn');break;
    case 'encounter_started':log('遭遇开始',`${item.encounter_id} · ${item.monster_ids.length}名敌人已生成`);break;
    case 'encounter_completed':log('遭遇完成',`${item.encounter_id} · 奖励容器已解锁，请前往开启并拾取`);break;
    case 'guardians_ready':log('守护计划',`${item.chest_id} · 第${item.generation}代 · ${item.monster_ids.length}名守护者已生成`);break;
    case 'guardian_spawn_blocked':log('守护受阻',`${item.chest_id} · 落点受阻，门禁保留，等待重试`,'warn');break;
    case 'picked_up': log('拾取', state.snapshot?.resources?.some(resource => resource.id === item.resource_id && resource.orb) ? '回复光球已消耗，回复量受上限约束。' : `${state.config?.pve?.items?.[Object.fromEntries(state.snapshot?.pve?.resource_items ?? [])[item.resource_id]]?.name ?? '物品'} +${item.quantity}，背包修订已更新。`); break;
    case 'extraction_started': log('撤离', `开始 ${state.config.extractionChannelMs / 1000} 秒撤离读条。受到伤害或离开范围会中断。`); break;
    case 'extraction_interrupted': log('撤离', '读条已中断，本次没有终局结算。', 'warn'); break;
    case 'operation_needs_reconciliation': log('待核对', `业务 ${item.operation_id.split(':').at(-1)} 需要存储侧核对；资产责任仍保留。`, 'warn'); break;
    case 'operation_resolved': break;
    case 'monster_killed': log('怪物', `${item.monster_id} 已死亡，掉落由内核独立裁决。`); break;
    case 'monster_skill': log('怪物技能', `${item.monster_id} · ${item.skill_id}`); break;
    case 'monster_healed': log('自疗', `${item.monster_id} 回复 ${item.amount}。`); break;
    case 'recovery_restored':
      state.session.recovered = { generation: item.generation, compensatingEntry: item.compensating_entry };
      log('账本恢复', item.compensating_entry ? '中断对局已关闭，按原业务核对并退还入场扣费。' : '已恢复冻结责任和结果，战斗现场已关闭。'); break;
    case 'asset_settlement_applied':
    case 'result_settlement_applied':
    case 'settlement_applied':
      state.lastSettlement = settlementView(item);
      log('结算', `${state.lastSettlement.result_id ?? (item.outcome === 'extraction' ? '撤离' : '死亡')}结算提交：保留 ${state.lastSettlement.kept}，掉落 ${state.lastSettlement.dropped}，收益 +${state.lastSettlement.cultivation}。`); break;
    case 'closed':
      state.ledger.pending = [...item.pending_operations];
      state.selectedGround=null;state.selectedUnit=null; state.phase = 'closed'; state.worldLive = item.live_worlds;
      log('回收', `World = ${item.live_worlds}，关闭时未决责任 ${item.pending_operations.length} 项。`); break;
    case 'fault': state.phase = 'faulted'; state.worldLive = 0; log('故障', `实例已封存：${item.code}`, 'error'); break;
  }
}

function onOutput(output, sessionId) {
  if (state.session?.id !== sessionId || state.seen.has(output.id)) return;
  state.seen.add(output.id); if (state.seen.size > 512) state.seen.delete(state.seen.values().next().value);
  const body = output.body;
  if (body.kind === 'publication') {
    if (!sameConnection(body.connection, state.session.connection)) return;
    applySnapshot(body.snapshot);
    if (!['closed', 'faulted'].includes(state.phase)) state.worldLive = 1;
    if (!['closed', 'faulted'].includes(state.phase)) state.phase = body.snapshot.phase;
    if (!body.snapshot.protected) state.handshake = false;
    // Apply the state and draw a frame before acknowledging its publication.
    render();
    requestAnimationFrame(() => {
      if (state.session?.id !== sessionId) return;
      acknowledge(sessionId, output.id).then(result => {
        if (result.ignored || state.session?.id !== sessionId) return;
        if (body.snapshot.protected && ['deploying', 'active'].includes(state.phase)
          && state.online && !state.reconnecting && !state.replaced && state.focused && !state.paused && !document.hidden && !state.handshake) {
          state.handshake = true;
          action({ kind: 'combat_ready', publication_id: output.id }).catch(error => {
            state.handshake = false;
            if (!['CLOSED', 'NOT_READY', 'STALE_PUBLICATION'].includes(error.message)) fail(error);
          });
        }
      }).catch(error => { if (state.session?.id === sessionId && state.retiringSession !== sessionId && !['CLOSED', 'STALE_CONNECTION'].includes(error.message)) fail(error); });
    });
  } else if (body.kind === 'event') event(output);
  else {
    const names = { load_participant: '读取入场事实', commit_admission: '原子扣除战备与入场寿元', commit_settlement: '原子提交终局资产', query_operation: '查询原业务结果', reconcile_operation: '等待人工核对冻结责任', compensate_admission: '补偿未激活入场' };
    log('业务', `${names[body.request.body.kind] || body.request.body.kind} · 尝试 ${body.request.attempt_id}`, body.request.body.kind === 'reconcile_operation' ? 'warn' : '');
  }
  render();
}

function sameConnection(a, b) { return a?.id === b?.id && a?.generation === b?.generation; }
async function loadScenario(scenario, session = null) {
  if (state.config?.scenario === scenario && (!session || state.config.configHash === session.configHash)) return;
  state.config = await api(`/api/config?scenario=${encodeURIComponent(scenario)}${session ? `&sessionId=${encodeURIComponent(session.id)}` : ''}`);
  $('scenario').value = scenario;
  $('arena').dataset.configHash = state.config.configHash;
  $('arena-title').textContent = state.config.pve?.map_id ?? scenario;
  $('batch-label').textContent = scenario;
  $('arena-scale').textContent = `2:1 等距 · ${state.config.geometry.max[0]-state.config.geometry.min[0]} × ${state.config.geometry.max[1]-state.config.geometry.min[1]} 世界单位`;
  $('inventory-label').textContent = '背包物品总数';
  $('cast-name').textContent = state.config.skill.id;
  $('navigate-chest').textContent=state.config.pve?.treasure_rules?.template.seal?'前往封印容器 ↗':'前往储物袋 ↗';
  if (state.config.pve) state.config.pve.cells = state.config.pve.cells_rle.flatMap(([index,count]) => Array(count).fill(index));
  $('reconnect-note').textContent = `刷新可恢复同一局；断线保留 ${state.config.params.runtime.reconnect_grace_ms / 1000} 秒，超时结算并回收。`;
  clockNote();
  floorKey = null; pveLayerKey = '';
}
const isSettlement = item => ['settlement_applied','asset_settlement_applied','result_settlement_applied'].includes(item?.kind);
function settlementView(item) {
  if (!item?.summary) return item;
  const summary=item.summary, rows=summary.inventory;
  const main=rows.find(row=>row.item_id===state.config?.itemId)??{kept:0,dropped:0};
  return {...item,...summary,...main,result_id:summary.result?.result_id,
    cultivation:item.credits.find(row=>row.asset_id===state.config?.assetContract?.reward_asset_id)?.quantity??0,
    items:rows.filter(row=>row.item_id!==state.config?.itemId)};
}
function restore(info) {
  state.sequence = Math.max(state.session?.id === info.id ? state.sequence : 0, info.sequence);
  if(state.session?.id!==info.id){state.selectedGround=null;state.selectedUnit=null;state.unitButtonsKey=null;}
  state.session = info;
  state.phase = info.phase; state.ledger = info.ledger;
  state.events = [...(info.events ?? [])]; state.lastSettlement = settlementView(info.settlement ?? state.events.findLast(isSettlement) ?? null);
  state.worldLive = ['closed', 'faulted', 'preparing'].includes(info.phase) ? 0 : 1;
  if (info.snapshot) applySnapshot(info.snapshot);
  render();
}
function replaced() {
  state.source?.close(); state.online = false; state.replaced = true; state.handshake = false;
  keys.clear(); lastDirection = '0,0';
  $('network-status').textContent = '已由新页面接管'; $('network-dot').classList.remove('online');
  document.body.dataset.connection = 'replaced'; render();
}
function connect(info) {
  state.source?.close();
  const id = info.id, connection = { ...info.connection };
  const source = new EventSource(`/api/sessions/${id}/events?connectionId=${encodeURIComponent(connection.id)}&generation=${connection.generation}`);
  state.source = source;
  const current = () => state.source === source && state.session?.id === id && sameConnection(connection, state.session.connection);
  source.addEventListener('output', message => { if (current()) onOutput(JSON.parse(message.data), id); });
  source.addEventListener('ledger', message => { if (current()) { state.ledger = JSON.parse(message.data); render(); } });
  source.addEventListener('status', message => {
    if (!current()) return;
    const status = JSON.parse(message.data);
    if (!['closed', 'faulted'].includes(state.phase) || ['closed', 'faulted'].includes(status.phase)) state.phase = status.phase;
    ensureJoined().catch(fail);
    render();
  });
  source.addEventListener('notice', message => { if (current()) log('模拟', JSON.parse(message.data).message, 'warn'); });
  source.addEventListener('problem', message => { if (current()) fail(new Error(JSON.parse(message.data).code)); });
  source.addEventListener('session', message => {
    if (!current()) return;
    const latest = JSON.parse(message.data); state.ledger = latest.ledger;
    state.session.recovered = latest.recovered;
    state.events = latest.events ?? state.events;
    state.lastSettlement = settlementView(latest.settlement ?? state.events.findLast(isSettlement) ?? state.lastSettlement);
    state.sequence = Math.max(state.sequence, latest.sequence);
    if (['closed', 'faulted'].includes(latest.phase)) { state.phase = latest.phase; state.worldLive = 0; }
    render();
    ensureJoined().catch(fail);
  });
  source.addEventListener('replaced', () => { if (current()) replaced(); });
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => { source.close(); reject(new Error('连接本地服务超时')); }, 5000);
    source.onerror = () => {
      if (!current() || state.unloading || state.replaced) return;
      clearTimeout(timeout); source.close(); state.online = false; state.handshake = false;
      if (state.retiringSession === id) return;
      keys.clear(); lastDirection = '0,0';
      $('network-status').textContent = '连接中断，正在恢复'; $('network-dot').classList.remove('online'); render();
      reject(new Error('连接暂时中断'));
      if (!reconnectJob && !state.starting) recover(state.session);
    };
    source.addEventListener('open', () => {
      if (!current()) return;
      clearTimeout(timeout); state.online = true;
      $('network-status').textContent = state.reconnecting ? '正在恢复对局' : '内核已连接';
      $('network-dot').classList.add('online'); document.body.dataset.connection = 'connected'; render(); resolve();
    }, { once: true });
  });
}
function recover(previous) {
  if (state.unloading) return Promise.resolve();
  if (reconnectJob) return reconnectJob;
  reconnectJob = (async () => {
    state.source?.close(); state.reconnecting = true; state.online = false; state.handshake = false;
    state.replaced = false; keys.clear(); lastDirection = '0,0';
    $('network-status').textContent = '正在恢复对局'; $('network-dot').classList.remove('online');
    document.body.dataset.connection = 'recovering'; render();
    let request = null; const deadline = performance.now() + 30000;
    while (!state.unloading && performance.now() < deadline) {
      try {
        if (!request) request = { connection: { ...previous.connection }, requestId: `resume-${crypto.randomUUID()}` };
        const info = await api(`/api/sessions/${previous.id}/resume`, request);
        if (state.unloading) return;
        state.poses = {}; state.snapshotAt = null; state.syncGap = null; state.paused = false;
        await loadScenario(info.scenario, info);
        state.seen.clear(); restore(info);
        await connect(info);
        await ensureJoined();
        if (['deploying', 'active'].includes(info.phase) && state.focused && !document.hidden) await action({ kind: 'foreground' });
        state.reconnecting = false;
        $('network-status').textContent = info.recovered ? '账本已恢复' : '对局已恢复'; document.body.dataset.connection = 'connected';
        log('恢复', info.recovered ? '服务已重启，已恢复结果与待核对责任；战斗现场已关闭。' : `继续同一局，连接 ${info.connection.generation}；入场次数 ${info.ledger.admissions}。`);
        render(); return;
      } catch (error) {
        state.source?.close(); state.online = false;
        if (error.message === 'STALE_CONNECTION') {
          const latest = await api('/api/current').catch(() => null);
          if (latest?.id === previous.id && (latest.recovered?.generation ?? 0) > (previous.recovered?.generation ?? 0)) {
            previous = latest; request = null; continue;
          }
          state.reconnecting = false; replaced(); return;
        }
        if (['NOT_FOUND', 'KERNEL_EXITED'].includes(error.message)) { state.reconnecting = false; fail(error); render(); return; }
        await new Promise(resolve => setTimeout(resolve, 500));
      }
    }
    state.reconnecting = false; $('network-status').textContent = '暂未连接，可刷新重试'; render();
  })().finally(() => { reconnectJob = null; });
  return reconnectJob;
}

async function start(clock = $('clock').value) {
  resetMovement();
  state.retiringSession = state.session?.id ?? null;
  state.starting = true; render();
  try {
    // Finish in-flight receipts before the server replaces the previous session.
    await acknowledgements;
    const info = await ordered('/api/sessions', { clock, fault: $('fault').value, seed: 7,
      previousConnection: state.session?.connection ?? null, scenario: $('scenario').value });
    await loadScenario(info.scenario, info);
    state.selectedGround=null;state.selectedUnit=null;state.unitButtonsKey=null;state.session = info; state.retiringSession = null; state.snapshot = null; state.poses = {}; state.snapshotAt = null; state.syncGap = null;
    if (info.snapshot) applySnapshot(info.snapshot);
    state.phase = info.phase; state.sequence = info.sequence;
    state.ledger = info.ledger; state.events = [...(info.events ?? [])]; state.logs = []; state.seen.clear(); state.handshake = false;
    state.replaced = false; state.reconnecting = false;
    state.lastSettlement = null; state.worldLive = info.phase === 'deploying' ? 1 : 0;
    state.floating = []; state.pointer = null; state.paused = false; state.focused = !document.hidden;
    delete document.body.dataset.verification;
    clockNote();
    $('event-log').replaceChildren();
    await connect(info);
    const deadline = performance.now() + 20000;
    while (state.phase === 'preparing' && state.session?.id === info.id && !state.unloading && performance.now() < deadline) {
      await new Promise(resolve => setTimeout(resolve, 30));
    }
    if (!['deploying', 'active'].includes(state.phase)) return false;
    await ensureJoined();
    $('arena').focus({ preventScroll: true });
    return true;
  } finally {
    const retiredStreamClosed = state.retiringSession === state.session?.id &&
      state.source?.readyState === EventSource.CLOSED;
    state.starting = false; state.retiringSession = null; render();
    if (retiredStreamClosed && !state.replaced) recover(state.session);
  }
}

function ensureJoined() {
  if (state.phase !== 'deploying' || !state.online || state.replaced || state.unloading) return Promise.resolve();
  const token = `${state.session.id}/${state.session.connection.id}/${state.session.connection.generation}`;
  if (state.joinToken === token) return state.joinPromise;
  state.joinToken = token;
  state.joinPromise = (async () => {
    await action({ kind: 'join' });
    if (state.focused && !state.paused && !document.hidden) await action({ kind: 'foreground' });
  })().catch(error => { if (state.joinToken === token) state.joinToken = null; throw error; });
  return state.joinPromise;
}

function render() { hudDirty = true; }
function renderHud() {
  const snapshot = state.snapshot;
  // Publish identity and phase in the same frame, so a new match never displays the old result.
  if(state.session)$('match-id').textContent=`MATCH ${state.session.id.slice(0,8)} · SEED ${state.session.seed} · 连接 ${state.session.connection.generation}`;
  const terminal = ['closed', 'faulted'].includes(state.phase);
  const active = state.phase === 'active' && snapshot && !snapshot.protected && state.online && !state.reconnecting && !state.replaced && !state.starting && !state.paused && state.focused && !document.hidden;
  $('phase').textContent = phaseNames[state.phase] || state.phase; $('phase').dataset.phase = state.phase;
  $('logical-time').textContent = time(snapshot?.logical_ms, true);
  $('empty-scene').classList.toggle('hidden', Boolean(state.session));
  $('hp-value').textContent = snapshot?.hp ?? '—';
  $('spirit-value').textContent = snapshot?.spirit ?? '—';
  $('sync-rate').textContent = state.session?.clock === 'fast' ? '加速验证'
    : state.syncGap ? `同步 ${Math.round(1000 / state.syncGap)} Hz` : '等待同步';
  $('hp-max').textContent=`/ ${(snapshot?.max_hp ?? state.config?.player.maxHp) ?? '—'}`;
  $('hp-meter').style.width = `${snapshot ? snapshot.hp / (snapshot.max_hp ?? state.config.player.maxHp) * 100 : 0}%`;
  $('spirit-max').textContent=`/ ${(snapshot?.max_spirit ?? state.config?.player.maxSpirit) ?? '—'}`;
  $('spirit-meter').style.width = `${snapshot ? snapshot.spirit / Math.max(1,snapshot.max_spirit ?? state.config.player.maxSpirit) * 100 : 0}%`;
  $('inventory').textContent = snapshot?.inventory_quantity ?? 0;
  for (const id of ['attack', 'cast', 'cast-secondary', 'pickup', 'extract', 'open-chest']) $(id).disabled = !active;
  const skillCooldown = Math.max(0, (snapshot?.skill_ready_at_ms ?? 0) - (snapshot?.logical_ms ?? 0));
  $('cast').disabled ||= skillCooldown > 0;
  $('cast-detail').textContent = skillCooldown ? `冷却 ${(skillCooldown / 1000).toFixed(1)}s` : `消耗 ${state.config?.skill.cost ?? '—'} 灵力 · 冷却 ${(state.config?.skill.cooldown_ms ?? 0)/1000}s`;
  renderUnitTargets(active);renderGroundTarget(active);
  const secondary = state.config?.playerSkills?.[1];
  $('additional-skill-row').classList.toggle('hidden', !secondary);
  $('cast-secondary').classList.toggle('hidden', !secondary);
  if (state.config?.playerSkills) {
    const shared = snapshot?.pending_action ? '准备中' : (snapshot?.logical_ms ?? 0) < (snapshot?.recovery_until_ms ?? 0) ? '后摇中' : '';
    $('attack').disabled ||= Boolean(shared);
    for (const [index, button] of ['cast', 'cast-secondary'].entries()) {
      const skill = state.config.playerSkills[index];
      const row = snapshot?.player_skills?.find(v => v.skill_id === skill?.id);
      const cooldown = Math.max(0, (row?.ready_at_ms ?? 0) - (snapshot?.logical_ms ?? 0));
      $(button).disabled ||= !skill || (skill?.mechanism==='unit_projectile' && !state.selectedUnit) || (isGroundSkill(skill) && !state.selectedGround) || (isDirectionSkill(skill) && !state.pointer) || Boolean(shared) || cooldown > 0 || (snapshot?.spirit ?? 0) < skill.cost;
      if (skill) {
        $(button+'-name').textContent = skill.id + (skill.mechanism === 'self_status' ? ' · 自身施法' : skill.mechanism==='unit_projectile'?' · 指定单位':skill.mechanism==='ground_circle'?' · 地面范围':skill.mechanism==='ground_field'?' · 持续区域':skill.mechanism==='line_field'?' · 线形区域':skill.mechanism==='piercing_projectile'?' · 直线穿透':'');
        $(button+'-detail').textContent = shared || (cooldown ? `冷却 ${(cooldown/1000).toFixed(1)}s` : (snapshot?.spirit ?? 0) < skill.cost ? `灵力不足 · 需要 ${skill.cost}` : `消耗 ${skill.cost} 灵力`);
      }
    }
  }
  $('pause').disabled = !state.session || terminal || !state.online || state.replaced || state.reconnecting;
  $('pause').textContent = state.paused ? '▷ 恢复前台' : 'Ⅱ 保护暂停';
  $('leave').disabled = !active;
  $('close').disabled = !state.session || terminal || state.starting || state.replaced || state.reconnecting;
  $('start').disabled = state.booting || state.starting || state.replaced || state.reconnecting;
  $('canvas-start').disabled = $('start').disabled;

  $('scenario').disabled = state.booting || state.starting;
  $('navigate-extraction').disabled = !active || !snapshot?.pve;
  const chests = snapshot?.pve?.chests ?? [];
  const bosses=(snapshot?.pve?.monsters??[]).filter(m=>m.boss);
  const supports=(snapshot?.pve?.monsters??[]).filter(m=>m.support),supportCasts=activeSupportCasts();
  $('support-panel').hidden=!supports.length;
  $('support-status').textContent=supports.map(m=>{
    const s=m.support,a=supportCasts.find(actor=>actor.id===m.id)?.support.cast,target=snapshot.pve.monsters.find(t=>t.id===a?.target_id),name=state.config.pve.additional_templates[m.template_id]?.name??m.id;
    const cooldown=Math.max(0,s.interrupt_ready_at_ms-snapshot.logical_ms),window=a&&snapshot.logical_ms<=a.interrupt_until_ms;
    return `${name} · ${s.name} · ${m.hp>0?`剩余治疗 ${s.remaining_casts}/${s.max_casts}`:'已击败'}${a?`\n治疗目标 ${target?state.config.pve.additional_templates[target.template_id]?.name??target.id:a.target_id} · ${a.committed?`引导 ${a.completed_ticks}/${a.total_ticks}`:`准备 ${Math.max(0,(a.commits_at_ms-snapshot.logical_ms)/1000).toFixed(1)}秒`} · ${window?(cooldown?`打断冷却 ${(cooldown/1000).toFixed(1)}秒`:'可主动命中打断'):'打断窗口已结束'}`:''}`;
  }).join('\n');
  const supportCast=supportCasts[0]?.support.cast;
  $('support-progress').value=supportCast?(supportCast.committed?100*supportCast.completed_ticks/supportCast.total_ticks:Math.min(100,100*(snapshot.logical_ms-supportCast.started_at_ms)/(supportCast.commits_at_ms-supportCast.started_at_ms))):0;
  $('navigate-support').disabled=!active||!supports.some(m=>m.hp>0);
  const heavies=(snapshot?.pve?.monsters??[]).filter(m=>m.heavy),heavyCasts=activeHeavyCasts(),counter=activeCounter();
  $('heavy-panel').hidden=!heavies.length;
  $('heavy-status').textContent=heavies.map(m=>{const h=m.heavy,a=heavyCasts.find(x=>x.id===m.id)?.heavy.cast;return `${h.name} · ${m.hp===0?'已击败':a?`${a.committed?'轨迹推进':'准备预警'} ${Math.max(0,((a.committed?a.ends_at_ms:a.commits_at_ms)-snapshot.logical_ms)/1000).toFixed(1)}秒`:`冷却 ${Math.max(0,(h.ready_at_ms-snapshot.logical_ms)/1000).toFixed(1)}秒`}`;}).join(' / ');
  const heavyCast=heavyCasts[0]?.heavy.cast;
  $('heavy-progress').value=heavyCast?(heavyCast.committed?Math.min(100,100*(snapshot.logical_ms-heavyCast.commits_at_ms)/(heavyCast.ends_at_ms-heavyCast.commits_at_ms)):Math.min(100,100*(snapshot.logical_ms-heavyCast.started_at_ms)/(heavyCast.commits_at_ms-heavyCast.started_at_ms))):0;
  const counterName = '反击';
  $('counter-status').textContent=counter?`${counterName} · 剩余 ${Math.max(0,(counter.ends_at_ms-snapshot.logical_ms)/1000).toFixed(1)}秒 · 主动命中 ${state.config.pve.additional_templates[snapshot.pve.monsters.find(m=>m.id===counter.monster_id)?.template_id]?.name??counter.monster_id}，追加一次物理伤害`:'移动躲过锁定自己的整次重击，可获得一次反击机会';
  const warnings=activeAreaWarnings();
  $('area-status').textContent=warnings.length?`${warnings.map(a=>`${a.name} · 剩余 ${(Math.max(0,a.resolves_at_ms-snapshot.logical_ms)/1000).toFixed(1)}秒`).join(' / ')} · 请让角色整体离开预警圈`:'';
  $('boss-status').textContent=bosses.map(m=>{const template=state.config?.pve?.additional_templates?.[m.template_id],area=template?.area;return `${template?.name??m.id} · ${m.hp>0?m.boss.phase_name:'已击败'} · 阶段 ${m.boss.phase_index}/${m.boss.phase_count} · 第${m.boss.epoch}轮 · HP ${m.hp}/${m.max_hp} · 近战 ${(m.boss.attack_interval_ms/1000).toFixed(1)}秒 · ${area?'弹道/治疗':'施法'} ${Math.round(m.boss.cast_chance*100)}%${area?` · ${area.name} ${Math.round(area.cast_chance*100)}%`:''}`;}).join(' / ');
  const encounters=snapshot?.pve?.encounters??[];
  const timedPosition = snapshot?.pve?.timed_position;
  $('timed-position-panel').hidden = !timedPosition;
  if (timedPosition) {
    const warning = active ? timedPosition.warning : null;
    const encounter = encounters.find(entry => entry.id === timedPosition.encounter_id);
    const coreId = timedPosition.contact_target_id;
    const core = coreId && snapshot.pve.monsters.find(monster => monster.id === coreId);
    const instruction = coreId
      ? warning?.hit_confirmed ? '本轮已破冰' : '发起新攻击并命中核心'
      : warning?.site ? `进入 ${warning.site.name}` : '靠近任一存活守卫';
    const status = warning
      ? `剩余 ${Math.max(0, (warning.resolves_at_ms - snapshot.logical_ms) / 1000).toFixed(1)} 秒 · ${instruction}`
      : encounter?.phase === 'completed' ? '已完成' : core?.hp === 0 ? '核心已击败' : '进入终局范围后等待预警';
    const extension = timedPosition.warning_bonus_active ? '后续预警已延长' : '基础预警';
    const reduction = timedPosition.failure_scale_active ? '后续反噬已减轻' : '基础反噬';
    const preparation = timedPosition.stabilized_sites == null
      ? `${extension} · ${reduction}` : `已稳定 ${timedPosition.stabilized_sites} 条灵流`;
    const hitPreparation = `${timedPosition.warning_bonus_active ? '后续窗口已延长' : '基础窗口'} · ${timedPosition.failure_scale_active ? '后续回冻已削弱' : '基础回冻'}`;
    $('timed-position-status').textContent =
      `${timedPosition.name} · ${status}\n${coreId ? hitPreparation : preparation} · 本轮成功 ${timedPosition.successes} 次`;
    $('navigate-timed-position').disabled = !warning || Boolean(coreId && !core?.hp);
    $('navigate-timed-position').textContent = coreId ? '靠近核心 ↗' : warning?.site ? `前往${warning.site.name} ↗` : '靠近守卫 ↗';
  }
  const resets=encounters.filter(e=>e.reset);
  $('reset-panel').hidden=!resets.length;
  $('reset-status').textContent=resets.map(e=>{const r=e.reset,name=state.config.pve.encounters.find(c=>c.id===e.id)?.name??e.id;return `${name} · 第${r.round}轮 · 实际击杀${r.actual_defeats} / 首次收益${r.rewarded_defeats} · ${e.phase==='completed'?'已完成':r.reset_at_ms!=null&&active?`脱战 ${Math.max(0,(r.reset_at_ms-snapshot.logical_ms)/1000).toFixed(1)}秒后整组复位`:'离开追击范围后开始计时'}`;}).join('\n');
  $('navigate-reset').disabled=!active||!resets.some(e=>!['dormant','completed'].includes(e.phase));
  $('encounter-status').textContent=encounters.map(e=>`${state.config?.pve?.encounters?.find(c=>c.id===e.id)?.name??e.id} · ${encounterDetail(e)}`).join(' / ');
  const nextEncounter = availableEncounter(snapshot);
  const requiredEncounter = availableEncounter(snapshot, true);
  $('navigate-encounter').hidden=!(state.config?.pve?.encounters?.length);
  $('navigate-encounter').disabled=!active||!nextEncounter;
  $('navigate-required-encounter').hidden = !requiredEncounter || requiredEncounter === nextEncounter;
  $('navigate-required-encounter').disabled = !active;
  const objective=snapshot?.pve?.map_objective,attuning=objective?.finishes_at_ms!=null;
  $('objective-panel').hidden=!objective;
  if(objective){
    const completed=new Set(objective.completed_encounter_ids),cost=objective.cost,owned=snapshot.pve.inventory.find(([id])=>id===cost.item_id)?.[1]??0;
    const target=chests.find(ch=>ch.id===objective.reward_chest_id),distance=target?Math.hypot(...target.position.map((v,i)=>v-snapshot.position[i])):Infinity;
    const status=objective.reward_opened?'藏匣已开启，请拾取后撤离':objective.attuned?'调谐完成，请解封开启藏匣':attuning?`调谐中 · 剩余 ${Math.max(0,(objective.finishes_at_ms-snapshot.logical_ms)/1000).toFixed(1)}秒`:objective.available?'目标完成，请前往藏匣调谐':'完成地图目标以解锁调谐';
    $('objective-status').textContent=`${objective.name} · ${status}\n${state.config.pve.encounters.map(e=>`${completed.has(e.id)?'✓':'○'} ${e.name}`).join(' → ')}\n调谐需要 ${(state.config.pve.items[cost.item_id]?.name??cost.item_id)} ×${cost.quantity} · 当前 ${owned} · 成功后扣除${objective.interruption_reason&&!objective.attuned?' · 上次中断：'+unlockReason(objective.interruption_reason):''}`;
    $('attune-progress').value=objective.attuned?100:attuning?Math.max(0,100*(1-(objective.finishes_at_ms-snapshot.logical_ms)/objective.duration_ms)):0;
    $('attune').disabled=!active||!objective.available||objective.attuned||attuning||owned<cost.quantity||distance>state.config.pve.treasure_rules.interaction_range||Boolean(snapshot.extraction_finishes_ms)||chests.some(ch=>ch.seal?.unlocking);
    $('cancel-attune').disabled=!active||!attuning;
  }
  const unlocking=chests.find(chest=>chest.seal?.unlocking);
  const nextChest = nearestChest();
  const chestDistance = nextChest && snapshot ? Math.hypot(...nextChest.position.map((v,i)=>v-snapshot.position[i])) : Infinity;
  $('navigate-chest').disabled = !active || !nextChest;
  $('open-chest').disabled ||= !nextChest || chestDistance > (state.config?.pve?.treasure_rules?.interaction_range ?? 0) || Boolean(snapshot?.extraction_finishes_ms);
  $('open-chest').hidden = !state.config?.pve?.treasure_rules;
  $('open-chest').disabled ||= Boolean(nextChest?.encounter&&!nextChest.encounter.completed||nextChest?.guardians?.pending||nextChest?.guardians?.alive);
  $('guardian-status').textContent=chests.filter(chest=>chest.guardians).map(chest=>`${chest.id} · 第${chest.generation}代 · ${chest.guardians.pending?'守护者待生成':`剩余守护 ${chest.guardians.alive}/${chest.guardians.monster_ids.length}`}`).join(' / ');
  if(objective&&!objective.attuned)$('open-chest').disabled ||= nextChest?.id===objective.reward_chest_id;
  if(unlocking||attuning)for(const id of ['attack','cast','cast-secondary','pickup','extract','open-chest'])$(id).disabled=true;
  const sealed=Boolean(state.config?.pve?.treasure_rules?.template.seal);
  $('cancel-chest').hidden=!sealed;$('cancel-chest').disabled=!active||!unlocking;
  $('open-chest-label').textContent=nextChest?.seal && nextChest.seal.completed_layers<nextChest.seal.layers ? '破解封印' : sealed?'开启容器':'开启储物袋';
  document.querySelector('.actionbar').classList.toggle('with-seals',sealed);
  const sealChest=unlocking??nextChest??chests[0],seal=sealChest?.seal;
  $('seal-panel').classList.toggle('hidden',!seal);
  if(seal){
    $('seal-state').textContent=`${sealChest.id} · 已解除 ${seal.completed_layers}/${seal.layers} 层 · ${sealChest.opened?(sealChest.respawn_at_ms==null?'本局已领取':'等待重生'):seal.unlocking?'破解中':seal.completed_layers===seal.layers?'已解封，等待开启':'等待继续破解'} · 当前层 ${(seal.layer_progress_ms/1000).toFixed(1)}/${(seal.layer_duration_ms/1000).toFixed(1)} 秒`;
    $('seal-progress').value=seal.completed_layers===seal.layers?100:seal.layer_progress_ms/seal.layer_duration_ms*100;
    $('seal-reason').textContent=seal.interruption_reason?`中断原因：${unlockReason(seal.interruption_reason)}`:'';
  }
  document.querySelector('.actionbar').classList.toggle('with-chests',Boolean(state.config?.pve?.treasure_rules));
  $('navigate-chest').hidden = !state.config?.pve?.treasure_rules;
  $('chest-status').textContent = chests.length ? chests.map(chest => `${chest.id.replace('forest-pouch-','储物袋 ')} · 第${chest.generation}轮 · ${chest.opened ? chest.respawn_at_ms==null?'已领取':`重生 ${Math.ceil(Math.max(0,chest.respawn_at_ms-snapshot.logical_ms)/1000)}秒` : chest.encounter&&!chest.encounter.completed?'遭遇未完成':chest.guardians?.pending?'守护待生成':chest.guardians?.alive?'守护存活':chest.seal&&chest.seal.completed_layers<chest.seal.layers?'待破解':'可开启'}`).join(' / ') : '';
  const names = id => state.config?.pve?.items?.[id]?.name ?? id;
  const score=terminal?state.lastSettlement?.scoring:snapshot?.pve?.scoring;
  $('score-status').textContent=score?`实际击杀 ${terminal?(state.lastSettlement?.reset_progress?.actual_kills??score.kills):score.kills} · 计分击杀 ${score.credited_kills} · 击杀积分 ${score.kill_score} · ${terminal?`已结算修为 +${state.lastSettlement.cultivation}`:`当前战果若撤离 +${score.extraction_cultivation} / 若死亡 +${score.death_cultivation} 修为`}`:'';
  $('pve-inventory').textContent = snapshot?.pve ? `计分击杀 ${snapshot.pve.kills} · 存活 ${snapshot.pve.monsters.filter(m=>m.hp>0).length}/${snapshot.pve.monsters.length} · 背包 ${snapshot.pve.inventory.map(([id,q]) => `${names(id)} ${q}`).join(' / ')} · 仓库 ${Object.entries(state.ledger.items ?? {}).map(([id,q]) => `${names(id)} ${q}`).join(' / ') || '暂无额外物品'}` : '';
  $('clock').disabled = state.booting || state.starting; $('fault').disabled = state.booting || state.starting;
  $('start').firstChild.textContent = state.starting ? '正在进入… ' : state.session ? '重新进入一局 ' : '进入试炼 ';
  const elapsed = snapshot?.active_at_ms == null ? 0 : snapshot.logical_ms - snapshot.active_at_ms;
  const remaining = Math.max(0, (state.config?.extractionUnlockMs ?? 180000) - elapsed);
  $('extract-label').textContent = snapshot?.extraction_finishes_ms
    ? `撤离中 ${((snapshot.extraction_finishes_ms - snapshot.logical_ms) / 1000).toFixed(1)}s`
    : snapshot?.extraction_open ? '申请撤离' : `撤离 ${time(remaining)}`;
  $('warehouse').textContent = state.ledger.warehouse; $('cultivation').textContent = state.ledger.cultivation;
  $('admissions').textContent = state.ledger.admissions; $('settlements').textContent = state.ledger.settlements;
  $('pending').textContent = state.ledger.pending.length;
  $('world-live').textContent = state.worldLive ?? (['deploying', 'active', 'draining'].includes(state.phase) ? 1 : '—');
  $('reconcile').classList.toggle('hidden', !state.ledger.manual);
  const statusRows = [
    ...(snapshot?.statuses??[]).map(s=>({status:s,label:'角色',atk:snapshot.effective_atk,def:snapshot.effective_def})),
    ...(snapshot?.pve?.monsters??[]).flatMap(m=>(m.statuses??[]).map(s=>({status:s,label:m.id,atk:m.effective_atk,def:m.effective_def}))),
  ];
  $('status-effects').hidden = snapshot?.effective_def == null || terminal;
  $('status-effects').textContent = `${snapshot?.healing_received_bps!=null?`受治疗 ${snapshot.healing_received_bps/100}% · `:''}${snapshot?.shield_total!=null?`当前护盾 ${snapshot.shield_total} · `:''}${snapshot?.effective_atk!=null?`当前攻击 ${snapshot.effective_atk} · `:''}当前防御 ${snapshot?.effective_def??'—'}\n` + (statusRows.length ? statusRows.map(({status:s,label,atk,def})=>`${label} · ${s.definition_id}${s.stacks!=null?` · ${s.stacks}/${s.max_stacks}层`:''}${s.periodic?` · ${s.periodic.kind==='max_hp_heal'?'HOT':'DOT'} ${s.periodic.ticks_done}/${s.periodic.total_ticks}跳 · 下一跳 ${s.periodic.next_tick_at_ms==null?'结束':`${s.periodic.next_tick_at_ms}ms`} · 冻结量 ${s.periodic.raw_heal??s.periodic.raw_damage}`:s.shield?` · 护盾 ${s.shield.remaining}/${s.shield.granted}`:s.attribute?` · ${s.attribute==='healing_received'?'受治疗':s.attribute==='atk'?'攻击':'防御'}修正`:''} · 来源 ${s.source.id} 第${s.source.generation}代${atk!=null?` · 攻击 ${atk}`:''} · 防御 ${def} · ${(Math.max(0,s.expires_at_ms-snapshot.logical_ms)/1000).toFixed(1)}秒`).join('\n') : '暂无定时状态');
  const has = kind => state.events.some(item => item.kind === kind || kind==='settlement_applied' && isSettlement(item));
  const combatDone=snapshot?.pve ? snapshot.pve.monsters.every(m=>m.hp===0)&&(snapshot.pve.encounters??[]).every(e=>e.phase==='completed')&&chests.every(chest=>!chest.guardians?.pending) : has('target_killed');
  const stages = { entry: has('activated'), combat: combatDone, loot: snapshot?.pve ? combatDone && snapshot.resources.length === 0 && (snapshot.pve.chests??[]).every(chest=>chest.opened) : has('picked_up') && snapshot?.resources.length === 0,
    settle: has('settlement_applied'), close: state.worldLive === 0 && terminal && state.ledger.pending.length === 0 };
  for (const element of document.querySelectorAll('[data-step]')) {
    const done = stages[element.dataset.step]; element.classList.toggle('done', done); element.querySelector('b').textContent = done ? '✓' : '○';
  }
  $('progress-count').textContent = `${Object.values(stages).filter(Boolean).length} / 5`;
  $('result-card').classList.toggle('hidden', !terminal);
  if (terminal) {
    const pending = state.ledger.pending.length;
    const refunded = state.session?.recovered && state.ledger.compensations > 0 && !state.lastSettlement;
    $('result-title').textContent = pending ? '世界已关闭，待核对' : state.lastSettlement?.result_id ? `${state.lastSettlement.result_id} · 结算完成` : state.lastSettlement?.outcome === 'extraction' ? '撤离成功' : state.lastSettlement ? '死亡结算完成' : refunded ? '对局中断，入场已退还' : '验证实例已关闭';
    $('result-detail').textContent = pending ? '冻结的资产责任仍保留在账本中。请完成核对。' : state.lastSettlement ? `保留 ${state.lastSettlement.kept} · 掉落 ${state.lastSettlement.dropped} · 修为 +${state.lastSettlement.cultivation}${state.lastSettlement.scoring?` · 击杀积分 ${state.lastSettlement.scoring.kill_score}`:''}${(state.lastSettlement.map_progress?.consumed??[]).map(row=>` · 消耗 ${state.config?.pve?.items?.[row.item_id]?.name??row.item_id} ×${row.quantity}`).join('')}${(state.lastSettlement.items ?? []).map(item => `；${state.config?.pve?.items?.[item.item_id]?.name ?? item.item_id} 保留${item.kept}/掉落${item.dropped}`).join('')}` : refunded ? `已按原业务退还 ${state.config.carriedQuantity} 份入场物品与 ${state.config.params.entry_years} 年寿元。可重新进入。` : '可重新开始，验证另一条路径。';
    $('result-warehouse').textContent = state.ledger.warehouse; $('result-cultivation').textContent = state.ledger.cultivation;
    $('result-cleanup').textContent = `World ${state.worldLive ?? 0} / 未决业务 ${pending}`;
  }

}

function clockNote() { $('clock-note').textContent = $('clock').value === 'live'
  ? `实时模式适合手动体验。撤离点在逻辑时间 ${state.config.extractionUnlockMs / 1000} 秒后开放。`
  : '加速模式会加快整场模拟，请保持页面前台。'; }
function encounterPhase(phase){return ({dormant:'待触发',waiting:'等待下一波',spawning:'等待生成',active:'战斗中',completed:'已完成'})[phase]??phase;}
function encounterDetail(e){
  if(e.reset)return `${encounterPhase(e.phase)} · 第${e.reset.round}轮${e.wave?` / 第${e.wave.index}波`:""} · 目标${e.defeated_required}/${e.required_monster_ids.length}`;
  if(e.blocked_by?.length)return `等待前置：${e.blocked_by.map(id=>state.config.pve.encounters.find(c=>c.id===id)?.name??id).join('、')}`;
  if(!e.wave)return `${encounterPhase(e.phase)} · 目标 ${e.defeated_required}/${e.required_monster_ids.length}`;
  const w=e.wave,remaining=w.ready_at_ms==null?'':` · 间隔 ${Math.max(0,(w.ready_at_ms-state.snapshot.logical_ms)/1000).toFixed(1)}秒`;
  return `${encounterPhase(e.phase)} · 第${w.index}/${w.total}波 · 当前目标 ${w.defeated_required}/${w.required_monster_ids.length} · 已完成 ${w.completed_waves}波${remaining}`;
}
const keys = new Set(); let lastDirection = '0,0';
function movable() { return state.phase === 'active' && state.snapshot && !state.snapshot.protected && state.online && !state.reconnecting && !state.replaced && !state.starting && !state.paused && state.focused && !document.hidden; }
function movement() {
  const direction = [Number(keys.has('d') || keys.has('arrowright')) - Number(keys.has('a') || keys.has('arrowleft')),
    Number(keys.has('s') || keys.has('arrowdown')) - Number(keys.has('w') || keys.has('arrowup'))];
  const id = direction.join(','); if (id === lastDirection) return; lastDirection = id;
  if (movable()) action(id === '0,0' ? { kind: 'stop' } : { kind: 'move', direction: unproject(direction) }).catch(fail);
}
function resetMovement() { keys.clear(); movement(); lastDirection = '0,0'; }
function aim() {
  const position = state.snapshot?.position ?? [0, 0];
  const point = state.pointer ?? state.snapshot?.target?.position ?? [position[0] + 100, position[1]];
  const direction = [point[0] - position[0], point[1] - position[1]];
  return Math.hypot(...direction) < 1e-6 ? [1, 0] : direction;
}
function nearestChest() {
  const position = state.snapshot?.position ?? [0,0];
  return [...(state.snapshot?.pve?.chests ?? [])].filter(chest => !chest.opened)
    .sort((a,b)=>Math.hypot(...a.position.map((v,i)=>v-position[i]))-Math.hypot(...b.position.map((v,i)=>v-position[i])))[0];
}
function unlockReason(reason){return ({cancelled:'主动取消',movement:'移动或导航',damage:'受到伤害',background:'后台或连接保护',closed:'死亡或关闭',out_of_range:'离开范围或遭到阻隔'})[reason]??reason;}
async function openChest() {
  const chest = nearestChest();
  if (!chest) { toast('当前没有可开启的储物袋'); return; }
  await action({kind:'open_chest',chest_id:chest.id,expected_generation:chest.generation});
}
async function pickup() {
  const position = state.snapshot?.position ?? [0, 0];
  const distance = resource => Math.hypot(resource.position[0] - position[0], resource.position[1] - position[1]);
  const resources = [...(state.snapshot?.resources ?? [])].sort((a, b) => distance(a) - distance(b));
  if (!resources.length) { toast('附近没有可见掉落'); return; }
  await action({ kind: 'pickup', resource_id: resources[0].id, expected_revision: state.snapshot.inventory_revision });
}
window.addEventListener('keydown', event => {
  if (event.target instanceof HTMLElement && event.target.matches('input,select,textarea,button')) return;
  const key = event.key.toLowerCase();
  if (['w', 'a', 's', 'd', 'arrowup', 'arrowdown', 'arrowleft', 'arrowright'].includes(key)) {
    event.preventDefault(); keys.add(key); movement();
  } else if (!event.repeat && movable()) {
    if (key === ' ') { event.preventDefault(); userAction({ kind: 'attack' }); }
    if (key === 'q') userAction(skillIntent(state.config.skill, aim()));
    if (key === '2' && state.config.playerSkills?.[1]) userAction(skillIntent(state.config.playerSkills[1], aim()));
    if (key === 'e') pickup().catch(fail);
    if (key === 'f') openChest().catch(fail);
    if (key === 'r') userAction({ kind: 'extract_start' });
    if (key === 'escape') userAction({ kind: 'cancel_action' });
  }
});
window.addEventListener('keyup', event => { keys.delete(event.key.toLowerCase()); movement(); });
function focusChange(focused) {
  if (state.focused === focused) return;
  resetMovement(); state.focused = focused; state.handshake = false;
  if (state.session && state.online && !state.reconnecting && !state.replaced && !state.starting && ['active', 'deploying'].includes(state.phase)) {
    action({ kind: focused && !state.paused ? 'foreground' : 'background' }).catch(error => { if (error.message !== 'CLOSED') fail(error); });
  }
}
window.addEventListener('blur', () => focusChange(false));
window.addEventListener('focus', () => focusChange(!document.hidden));
document.addEventListener('visibilitychange', () => focusChange(!document.hidden));
for (const type of ['beforeunload', 'pagehide']) window.addEventListener(type, () => {
  state.unloading = true; state.source?.close(); state.online = false;
});
for (const button of document.querySelectorAll('[data-dir]')) {
  button.addEventListener('pointerdown', event => { event.preventDefault(); button.setPointerCapture(event.pointerId); if (movable()) action({ kind: 'move', direction: unproject(button.dataset.dir.split(',').map(Number)) }).catch(fail); });
  for (const type of ['pointerup', 'pointercancel']) button.addEventListener(type, () => { if (movable()) action({ kind: 'stop' }).catch(fail); });
}

$('start').onclick = () => start().catch(fail);
$('canvas-start').onclick = $('start').onclick;

$('clock').onchange = clockNote;
$('attack').onclick = () => userAction({ kind: 'attack' });
$('cast').onclick = () => userAction(skillIntent(state.config.skill, aim()));
$('cast-secondary').onclick = () => { const skill = state.config.playerSkills?.[1]; if (skill) userAction(skillIntent(skill, aim())); };
$('pickup').onclick = () => pickup().catch(fail);
$('attune').onclick=()=>{const o=state.snapshot?.pve?.map_objective;if(o)userAction({kind:'attune_objective',objective_id:o.id,expected_revision:state.snapshot.inventory_revision});};
$('cancel-attune').onclick=()=>userAction({kind:'cancel_attunement'});
$('open-chest').onclick = () => openChest().catch(fail);
$('cancel-chest').onclick=()=>{const chest=state.snapshot?.pve?.chests.find(chest=>chest.seal?.unlocking);if(chest)userAction({kind:'cancel_chest_unlock',chest_id:chest.id,expected_generation:chest.generation});};
$('navigate-support').onclick=()=>navigateSupport().catch(fail);
async function navigateSupport(){
  const caster=activeSupportCasts()[0]??state.snapshot?.pve?.monsters.find(m=>m.hp>0&&m.support);if(!caster)return;
  const target=state.snapshot.pve.monsters.find(m=>m.id===caster.support.cast?.target_id&&m.id!==caster.id)??state.snapshot.pve.monsters.find(m=>m.hp>0&&m.id!==caster.id&&m.encounter_id===caster.encounter_id);
  const delta=target?caster.position.map((v,i)=>v-target.position[i]):[1,0],length=Math.hypot(...delta)||1,dir=delta.map(v=>v/length),offset=state.config.params.player_radius+(caster.radius??state.config.pve.combat.radius)+12;
  for(const d of [dir,[-dir[1],dir[0]],[dir[1],-dir[0]],dir.map(v=>-v)]){try{await action({kind:'navigate',destination:caster.position.map((v,i)=>v+d[i]*offset)});return;}catch(error){if(error.message!=='OUT_OF_RANGE')throw error;}}
  throw new Error('OUT_OF_RANGE');
}
$('navigate-reset').onclick=()=>leaveEncounter().catch(fail);
async function leaveEncounter(){
 const e=(state.snapshot?.pve?.encounters??[]).find(e=>e.reset&&!['dormant','completed'].includes(e.phase));if(!e)return;
 const c=state.config.pve.encounters.find(c=>c.id===e.id),radius=e.reset.pursuit_radius+state.config.pve.cell_size*2+state.config.params.player_radius;
 const delta=state.snapshot.position.map((v,i)=>v-c.trigger.center[i]),angle=Math.atan2(delta[1],delta[0]);
 for(let i=0;i<8;i++){const a=angle+i*Math.PI/4;try{await action({kind:'navigate',destination:[c.trigger.center[0]+Math.cos(a)*radius,c.trigger.center[1]+Math.sin(a)*radius]});return;}catch(error){if(error.message!=='OUT_OF_RANGE')throw error;}}
 throw new Error('OUT_OF_RANGE');
}
function availableEncounter(snapshot, requiredOnly = false) {
  const pve = snapshot?.pve;
  return pve?.encounters?.find(encounter =>
    encounter.phase !== 'completed' &&
    !encounter.blocked_by?.length &&
    (!requiredOnly || pve.map_objective?.required_encounter_ids.includes(encounter.id))
  );
}
function navigateToEncounter(requiredOnly) {
  const encounter = availableEncounter(state.snapshot, requiredOnly);
  const config = state.config?.pve?.encounters?.find(entry => entry.id === encounter?.id);
  if (config) userAction({kind: 'navigate', destination: config.trigger.center});
}
$('navigate-encounter').onclick = () => navigateToEncounter(false);
$('navigate-required-encounter').onclick = () => navigateToEncounter(true);
$('navigate-timed-position').onclick = () => {
  const snapshot = state.snapshot;
  const mechanic = snapshot?.pve?.timed_position;
  if (!mechanic?.warning) return;
  if (mechanic.contact_target_id) {
    const core = snapshot.pve.monsters.find(monster => monster.id === mechanic.contact_target_id);
    if (core?.hp) userAction({kind: 'navigate', destination: core.position});
    return;
  }
  if (mechanic.warning.site) {
    userAction({kind: 'navigate', destination: mechanic.warning.site.position});
    return;
  }
  const target = timedPositionMembers(snapshot).sort((a, b) =>
    Math.hypot(a.position[0] - snapshot.position[0], a.position[1] - snapshot.position[1]) -
    Math.hypot(b.position[0] - snapshot.position[0], b.position[1] - snapshot.position[1])
  )[0];
  if (target) userAction({kind: 'navigate', destination: target.position});
};
$('navigate-chest').onclick = () => { const chest=nearestChest();if(chest)userAction({kind:'navigate',destination:chest.position}); };
$('extract').onclick = () => userAction({ kind: 'extract_start' });
$('leave').onclick = () => userAction({ kind: 'leave' });
$('close').onclick = () => {
  resetMovement(); ordered(`/api/sessions/${state.session.id}/close`, { connection: { ...state.session.connection } }).catch(fail); render();
};
$('pause').onclick = () => {
  resetMovement(); state.paused = !state.paused; state.handshake = false;
  action({ kind: state.paused ? 'background' : 'foreground' }).catch(fail); render();
};
$('navigate-extraction').onclick = () => userAction({kind:'navigate',destination:state.config.extractionPoint});
$('reconcile').onclick = async () => {
  $('reconcile').disabled = true;
  try { await ordered(`/api/sessions/${state.session.id}/reconcile`, { connection: { ...state.session.connection } }); log('核对', '已提交核对请求，结果以内核确认和账本为准。'); }
  catch (error) { fail(error); } finally { $('reconcile').disabled = false; }
};
$('copy-report').onclick = async () => {
  const report = { scope: 'H5 + Rust kernel + local SQLite verification storage', session: state.session?.id,
    phase: state.phase, worldLive: state.worldLive, ledger: state.ledger, events: state.events };
  try { await navigator.clipboard.writeText(JSON.stringify(report, null, 2)); toast('验证报告已复制'); } catch { toast('当前浏览器不允许访问剪贴板', true); }
};

const canvas = $('arena'); const ctx = canvas.getContext('2d');
const fallbackBounds = { min: [-512, -512], max: [512, 512], walls: [{ min: [220, -80], max: [240, 80] }] };
let width = 0, height = 0, scale = 1, viewport = null, floor = null, floorKey = null;
function screen(position, elevation = 0) { return viewport.screen(position, elevation); }
new ResizeObserver(() => {
  const rect = canvas.getBoundingClientRect(); const ratio = Math.min(2, window.devicePixelRatio || 1);
  width = rect.width; height = rect.height; canvas.width = width * ratio; canvas.height = height * ratio;
  ctx.setTransform(ratio, 0, 0, ratio, 0, 0); floorKey = null;
  viewport = createViewport(width, height, state.config?.geometry ?? fallbackBounds); scale = viewport.scale;
}).observe(canvas);
canvas.addEventListener('pointerdown', event => {
  canvas.focus({preventScroll:true});
  if(movable() && state.config?.playerSkills?.some(isDirectionSkill)){const rect=canvas.getBoundingClientRect();state.pointer=viewport.world([event.clientX-rect.left,event.clientY-rect.top]);render();}
  if(movable() && state.config?.playerSkills?.some(isGroundSkill)){const rect=canvas.getBoundingClientRect();state.selectedGround=viewport.world([event.clientX-rect.left,event.clientY-rect.top]);render();return;}
  if(!state.config?.playerSkills?.some(s=>s.mechanism==='unit_projectile') || !movable())return;
  const rect=canvas.getBoundingClientRect(),point=[event.clientX-rect.left,event.clientY-rect.top];
  const hit=(state.snapshot?.pve?.monsters??[]).filter(m=>m.unit_target).map(m=>({m,p:screen(m.position)})).filter(({m,p})=>Math.hypot(point[0]-p[0],point[1]-p[1])<=Math.max(18,(m.radius??14)*scale)).sort((a,b)=>Math.hypot(point[0]-a.p[0],point[1]-a.p[1])-Math.hypot(point[0]-b.p[0],point[1]-b.p[1]))[0];
  if(hit)selectUnit(hit.m.unit_target);
});
canvas.addEventListener('pointermove', event => {
  if (!viewport || state.config?.playerSkills?.some(isDirectionSkill)) return;
  const rect = canvas.getBoundingClientRect();
  state.pointer = viewport.world([event.clientX - rect.left, event.clientY - rect.top]);
});
function polygon(points, fill, stroke) {
  ctx.beginPath(); points.forEach(([x, y], i) => i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)); ctx.closePath();
  if (fill) { ctx.fillStyle = fill; ctx.fill(); }
  if (stroke) { ctx.strokeStyle = stroke; ctx.lineWidth = 1; ctx.stroke(); }
}
function activeHeavyCasts(){return state.phase==='active'&&state.online&&!state.reconnecting&&!state.replaced&&!state.paused&&!state.snapshot?.protected?(state.snapshot?.pve?.monsters??[]).filter(m=>m.hp>0&&m.heavy?.cast):[];}
function activeCounter(){const v=state.snapshot?.pve?.counter;return state.phase==='active'&&state.online&&!state.reconnecting&&!state.replaced&&!state.paused&&!state.snapshot?.protected&&v&&state.snapshot.logical_ms<=v.ends_at_ms?v:null;}
function drawHeavyCasts(){
 const casts=activeHeavyCasts(),counter=activeCounter();canvas.dataset.heavyCasts=String(casts.length);canvas.dataset.counterReady=String(Boolean(counter));
 for(const m of casts){const a=m.heavy.cast,angle=Math.atan2(a.end[1]-a.origin[1],a.end[0]-a.origin[0]),points=[];
  for(let i=0;i<=12;i++){const t=angle-Math.PI/2+i*Math.PI/12;points.push(screen([a.end[0]+Math.cos(t)*a.half_width,a.end[1]+Math.sin(t)*a.half_width]));}
  for(let i=0;i<=12;i++){const t=angle+Math.PI/2+i*Math.PI/12;points.push(screen([a.origin[0]+Math.cos(t)*a.half_width,a.origin[1]+Math.sin(t)*a.half_width]));}
  polygon(points,a.committed?'#ff785640':'#eebc6a30',a.committed?'#ff7856':'#eebc6a');
  if(a.committed)groundCircle(a.front,a.half_width,'#ffcba4','#ff785650',2);
  text(`${m.heavy.name} · ${a.committed?'推进':'准备'} ${Math.max(0,((a.committed?a.ends_at_ms:a.commits_at_ms)-state.snapshot.logical_ms)/1000).toFixed(1)}秒`,m.position,-82,'#ffcba4',10);
 }
 if(counter){const target=state.snapshot.pve.monsters.find(m=>m.id===counter.monster_id&&m.hp>0);if(target){groundCircle(target.position,(target.radius??14)+12,'#a1e1ca',null,2);text(`借隙 ${Math.max(0,(counter.ends_at_ms-state.snapshot.logical_ms)/1000).toFixed(1)}秒`,target.position,-98,'#a1e1ca',11);}}
}
function activeSupportCasts(){return state.phase==='active'&&state.online&&!state.reconnecting&&!state.replaced&&!state.paused&&!state.snapshot?.protected?(state.snapshot?.pve?.monsters??[]).filter(m=>m.hp>0&&m.support?.cast):[];}
function drawSupportCasts(){
 const casts=activeSupportCasts();canvas.dataset.supportCasts=String(casts.length);
 for(const m of casts){const a=m.support.cast,target=state.snapshot.pve.monsters.find(t=>t.id===a.target_id);if(!target||target.hp===0)continue;
  const from=screen(m.position),to=screen(target.position),window=state.snapshot.logical_ms<=a.interrupt_until_ms&&state.snapshot.logical_ms>=m.support.interrupt_ready_at_ms;
  ctx.save();ctx.strokeStyle=window?'#a6d3ff':'#708ba0';ctx.lineWidth=2;ctx.setLineDash([5,5]);ctx.beginPath();ctx.moveTo(from[0],from[1]-15);ctx.lineTo(to[0],to[1]-15);ctx.stroke();ctx.restore();
  groundCircle(m.position,24,null,window?'#a6d3ff':'#708ba0');text(a.committed?`治疗 ${a.completed_ticks}/${a.total_ticks}`:`支援准备 ${Math.max(0,(a.commits_at_ms-state.snapshot.logical_ms)/1000).toFixed(1)}秒`,m.position,-65,'#b8d9ff',10);
 }
}
function activeAreaWarnings(){return state.phase==='active'&&state.online&&!state.reconnecting&&!state.replaced&&!state.paused&&!state.snapshot?.protected?(state.snapshot?.pve?.monsters??[]).filter(m=>m.hp>0&&m.area_warning).map(m=>m.area_warning):[];}
function drawAreaWarnings(){
 const warnings=activeAreaWarnings();canvas.dataset.areaWarnings=String(warnings.length);
 for(const a of warnings){
  groundCircle(a.center,a.radius,'#f5a37d','#e96b4225',2);
  const ratio=Math.max(0,Math.min(1,(state.snapshot.logical_ms-a.started_at_ms)/(a.resolves_at_ms-a.started_at_ms)));
  const [x,y]=screen(a.center);ctx.beginPath();ctx.ellipse(x,y,a.radius*scale*Math.SQRT2,a.radius*scale/Math.SQRT2,0,-Math.PI/2,-Math.PI/2+Math.PI*2*ratio);ctx.lineWidth=4;ctx.strokeStyle='#ff7856';ctx.stroke();
  text(`${a.name} · ${(Math.max(0,a.resolves_at_ms-state.snapshot.logical_ms)/1000).toFixed(1)}秒`,a.center,-a.radius*scale/Math.SQRT2-8,'#ffcba4',11);
 }
}
function groundCapsule(line, stroke, fill) {
  const angle=Math.atan2(line.end[1]-line.start[1],line.end[0]-line.start[0]), points=[];
  for(const [center,from] of [[line.end,angle-Math.PI/2],[line.start,angle+Math.PI/2]]){
    for(let i=0;i<=16;i++){const a=from+Math.PI*i/16;points.push(screen([center[0]+Math.cos(a)*line.width/2,center[1]+Math.sin(a)*line.width/2]));}
  }
  polygon(points,fill,stroke);
}
function groundCircle(position, radius, stroke, fill, lineWidth = 1) {
  const [x, y] = screen(position); ctx.beginPath();
  ctx.ellipse(x, y, radius * scale * Math.SQRT2, radius * scale / Math.SQRT2, 0, 0, Math.PI * 2);
  if (fill) { ctx.fillStyle = fill; ctx.fill(); }
  if (stroke) { ctx.strokeStyle = stroke; ctx.lineWidth = lineWidth; ctx.stroke(); }
}
function timedPositionMembers(snapshot) {
  const {timed_position: mechanic, monsters} = snapshot.pve;
  return monsters.filter(monster => monster.hp > 0 && monster.encounter_id === mechanic.encounter_id);
}
function drawTimedPosition() {
  const snapshot = state.snapshot;
  const mechanic = snapshot?.pve?.timed_position;
  if (!mechanic?.warning || !movable()) return;
  if (mechanic.contact_target_id) {
    const core = snapshot.pve.monsters.find(monster => monster.id === mechanic.contact_target_id);
    if (core?.hp) {
      const consumed = mechanic.warning.hit_confirmed;
      groundCircle(core.position, core.radius + 12, consumed ? '#9ed5aa' : '#8cd9ff', '#8cd9ff25', 2);
      text(consumed ? '已破冰' : '破冰窗口', core.position, -26, '#ceefff', 12);
    }
    return;
  }
  if (mechanic.warning.site) {
    const site = mechanic.warning.site;
    groundCircle(site.position, mechanic.radius, '#b5a6ff', '#b5a6ff38', 2);
    text(site.name, site.position, -12, '#e5dbff', 12);
    return;
  }
  for (const monster of timedPositionMembers(snapshot)) {
    groundCircle(monster.position, mechanic.radius, '#b5a6ff', '#b5a6ff20', 2);
  }
}
function text(label, position, offset, color, size = 10) {
  const [x, y] = screen(position); ctx.fillStyle = color; ctx.font = `${size}px -apple-system, "PingFang SC", sans-serif`;
  ctx.textAlign = 'center'; ctx.fillText(label, x, y + offset);
}
function drawFloor(bounds) {
  const key = JSON.stringify([canvas.width, canvas.height, bounds]);
  if (key === floorKey) { ctx.drawImage(floor, 0, 0, width, height); return; }
  viewport = createViewport(width, height, bounds); scale = viewport.scale;
  const outline = corners(bounds).map(point => screen(point));
  const lowered = outline.map(([x, y]) => [x, y + 12]);
  polygon([outline[1], outline[2], lowered[2], lowered[1]], '#172c35', '#30464e');
  polygon([outline[2], outline[3], lowered[3], lowered[2]], '#12242c', '#2b414b');
  polygon(outline, '#172b34', '#4c6e78');
  ctx.save(); polygon(outline); ctx.clip();
  const tile = 64;
  for (let x = bounds.min[0], ix = 0; x < bounds.max[0]; x += tile, ix++) {
    for (let y = bounds.min[1], iy = 0; y < bounds.max[1]; y += tile, iy++) {
      const rectangle = { min: [x, y], max: [Math.min(x + tile, bounds.max[0]), Math.min(y + tile, bounds.max[1])] };
      polygon(corners(rectangle).map(point => screen(point)), (ix + iy) % 2 ? '#192e36' : '#172a32', '#36525c40');
    }
  }
  ctx.restore(); polygon(outline, null, '#53727c');
  for (const [label, point, offset] of [['−X', [bounds.min[0], 0], -10], ['+X', [bounds.max[0], 0], 20],
    ['−Y', [0, bounds.min[1]], -10], ['+Y', [0, bounds.max[1]], 20]]) text(label, point, offset, '#64848f', 9);
  floor = document.createElement('canvas'); floor.width = canvas.width; floor.height = canvas.height;
  floor.getContext('2d').drawImage(canvas, 0, 0); floorKey = key;
}
function drawBlock(bounds) {
  const base = corners(bounds).map(point => screen(point));
  const rise = Math.max(22, 72 * scale); const top = base.map(([x, y]) => [x, y - rise]);
  polygon([base[1], base[2], top[2], top[1]], '#4a5c63', '#667b82');
  polygon([base[2], base[3], top[3], top[2]], '#344952', '#5b727c');
  polygon(top, '#657b81', '#8da2a6');
}
function drawTarget(target) {
  if(sameUnit(target.unit_target,state.selectedUnit))groundCircle(target.position,(target.radius??14)+12,null,'#9de0c7',2);
  const [x, y] = screen(target.position); const radius = Math.max(7, (target.boss?21:15) * scale);
  groundCircle(target.position, target.boss?32:24, target.boss?'#de9a64':null, '#06101470');
  // Upright markers use a ground anchor; they are not flattened with the floor.
  polygon([[x, y - 28], [x + radius, y - 17], [x, y - 4], [x - radius, y - 17]], '#c38c63', '#f2c6a1');
  polygon([[x, y - 28], [x + radius, y - 17], [x, y - 16]], '#e7b78a');
  const label=target.boss?`${state.config?.pve?.additional_templates?.[target.template_id]?.name??'守将'} · ${target.boss.phase_name} ${target.boss.phase_index}/${target.boss.phase_count}`:target.guardian?`${state.config?.pve?.additional_templates?.[target.template_id]?.name??'守护者'} · ${target.guardian.chest_id}/第${target.guardian.generation}代`:(state.config?.pve?target.kill_score!=null?(state.config.pve.additional_templates?.[target.template_id]?.name??target.id):target.id:'验证目标');
  text(`${label}${target.kill_score!=null?` · ${target.kill_score}分`:''} · ${target.hp} HP`, target.position, -43, '#e6bd96', 9);
  ctx.fillStyle = '#3d3531'; ctx.fillRect(x - 21, y - 36, 42, 3);
  ctx.fillStyle = '#dda574'; ctx.fillRect(x - 21, y - 36, 42 * target.hp / (target.max_hp ?? state.config?.target.maxHp ?? 200), 3);
}
function drawPlayer(position, snapshot, now) {
  const [x, y] = screen(position);
  groundCircle(position, 27, null, '#06101480');
  groundCircle(position, 16, snapshot.protected ? '#7996a2' : '#92dabb', '#92dabb18');
  const color = snapshot.protected ? '#6a8b98' : '#8ed6b9';
  polygon([[x, y - 20], [x + 8, y - 4], [x, y], [x - 8, y - 4]], color, '#b3e7d6');
  ctx.beginPath(); ctx.arc(x, y - 22, 6, 0, Math.PI * 2); ctx.fillStyle = '#c5efdf'; ctx.fill();
  ctx.beginPath(); ctx.arc(x + 1, y - 23, 1.5, 0, Math.PI * 2); ctx.fillStyle = '#315d55'; ctx.fill();
  if (snapshot.protected) {
    ctx.beginPath(); ctx.ellipse(x, y - 15, 14, 22, 0, 0, Math.PI * 2); ctx.strokeStyle = '#9cb7c6aa'; ctx.stroke();
  }
  text(snapshot.protected ? '你 · 保护中' : '你', position, 17, snapshot.protected ? '#9cb4bd' : '#c5e8db', 10);
  if (snapshot.extraction_finishes_ms) {
    const ratio = Math.max(0, 1 - (snapshot.extraction_finishes_ms - snapshot.logical_ms) / (state.config?.extractionChannelMs ?? 5000));
    ctx.beginPath(); ctx.arc(x, y - 16, 24, -Math.PI / 2, -Math.PI / 2 + Math.PI * 2 * ratio);
    ctx.strokeStyle = '#b7e8ef'; ctx.lineWidth = 3; ctx.stroke();
  }
}
function drawChest(chest) {
  const [x,y]=screen(chest.position);
  groundCircle(chest.position,18,chest.opened?'#34423850':'#b99b5d33','#c6a66580');
  polygon([[x,y-24],[x+13,y-10],[x+10,y],[x-10,y],[x-13,y-10]],chest.opened?'#46534d':'#997342','#dcc295');
  text(chest.opened ? chest.respawn_at_ms==null?'已领取':`待重生 · 第${chest.generation}轮` : chest.encounter&&!chest.encounter.completed?'遭遇锁定':chest.seal?`封印 ${chest.seal.completed_layers}/${chest.seal.layers}${chest.seal.unlocking?' · 破解中':''}`:`储物袋 · 第${chest.generation}轮`,chest.position,19,chest.opened?'#90a098':'#e4c898',9);
}
function drawResource(resource) {
  const [x, y] = screen(resource.position);
  groundCircle(resource.position, 15, null, '#06101470');
  if (resource.orb) {
    ctx.beginPath(); ctx.arc(x, y - 8, 5, 0, Math.PI * 2); ctx.fillStyle = '#b7d8ad'; ctx.fill();
    text('回复光球', resource.position, -21, '#b5d5ad', 8);
  } else {
    polygon([[x, y - 14], [x + 5, y - 7], [x, y - 1], [x - 5, y - 7]], '#dbbc87', '#f1d8b1');
    text(`${state.config?.pve?.items?.[Object.fromEntries(state.snapshot?.pve?.resource_items ?? [])[resource.id]]?.name ?? '物品'} ×${resource.quantity}`, resource.position, 17, '#d9bc8c', 8);
  }
}
let pveLayer = null, pveLayerKey = '', pveLayerViewport = null;
function drawPveFloor(now) {
  const map=state.config.pve, center=pose('player',now) ?? state.snapshot?.position ?? state.config.spawn;
  const bounds=c=>({min:[c[0]-520,c[1]-520],max:[c[0]+520,c[1]+520]});
  viewport=createViewport(width,height,bounds(center));scale=viewport.scale;
  const cell=map.cell_size, radius=Math.ceil(520/cell)+2;
  const anchor=center.map(v=>Math.floor(v/(cell*2))*(cell*2)), key=[width,height,...anchor,map.map_id].join('/');
  if(key!==pveLayerKey) {
    pveLayer??=document.createElement('canvas');pveLayer.width=width+128;pveLayer.height=height+128;
    const paint=pveLayer.getContext('2d');pveLayerViewport=createViewport(width,height,bounds(anchor));
    paint.fillStyle='#102126';paint.fillRect(0,0,width+128,height+128);
    const colors={plain:'#1b3431',forest_light:'#244139',forest_dense:'#102a24',water_shallow:'#214b59',road:'#52604b',swamp:'#3f4940',stone:'#424e4c'};
    const tilePolygon=(points,color,stroke,lift=0)=>{paint.beginPath();points.forEach((point,i)=>{const [x,y]=pveLayerViewport.screen(point,lift);if(i)paint.lineTo(x+64,y+64);else paint.moveTo(x+64,y+64);});paint.closePath();paint.fillStyle=color;paint.fill();if(stroke){paint.strokeStyle=stroke;paint.stroke();}};
    for(let y=Math.max(0,Math.floor(anchor[1]/cell)-radius);y<Math.min(map.height,Math.floor(anchor[1]/cell)+radius+1);y++)for(let x=Math.max(0,Math.floor(anchor[0]/cell)-radius);x<Math.min(map.width,Math.floor(anchor[0]/cell)+radius+1);x++) {
      const tile=map.palette[map.cells[y*map.width+x]];
      tilePolygon(corners({min:[x*cell,y*cell],max:[x*cell+cell,y*cell+cell]}),colors[tile.terrain]??'#243d35','#152e2930');
      if(!tile.walkable)tilePolygon(corners({min:[x*cell+cell/8,y*cell+cell/8],max:[x*cell+cell*7/8,y*cell+cell*7/8]}),'#142e28','#41615260',5);
    }
    pveLayerKey=key;
  }
  const current=viewport.screen([0,0]),cached=pveLayerViewport.screen([0,0]);
  ctx.drawImage(pveLayer,current[0]-cached[0]-64,current[1]-cached[1]-64);

}
function draw(now) {
  if (hudDirty) { hudDirty = false; renderHud(); }
  ctx.clearRect(0, 0, width, height);
  if (!width || !viewport || !state.config) { requestAnimationFrame(draw); return; }
  const bounds = state.config?.geometry ?? fallbackBounds;
  if (state.config?.pve) drawPveFloor(now); else drawFloor(bounds);
  const snapshot = state.snapshot;
  for(const config of state.config?.pve?.encounters??[]){
    const e=snapshot?.pve?.encounters?.find(e=>e.id===config.id),phase=e?.phase??'dormant';
    const color=phase==='completed'?'#91c4ae':'#d3ae74';
    if(e?.reset&&!['dormant','completed'].includes(phase)&&state.phase==='active'&&!snapshot?.protected){ctx.save();ctx.setLineDash([5,6]);groundCircle(config.trigger.center,e.reset.pursuit_radius,'#91c4ae80');ctx.restore();}
    groundCircle(config.trigger.center,config.trigger.radius,color,phase==='dormant'?'#d3ae7418':null);
    text(`${config.name}${e?.reset?` 第${e.reset.round}轮`:""} · ${encounterPhase(phase)}${e?.wave?` 第${e.wave.index}/${e.wave.total}波`:e?` ${e.defeated_required}/${e.required_monster_ids.length}`:''}`,config.trigger.center,-config.trigger.radius*scale/Math.SQRT2-12,color,10);
  }
  const extraction = state.config?.extractionPoint ?? [0, 0];
  const extractionRange=state.config.params.extraction_range;
  ctx.setLineDash([4, 5]); groundCircle(extraction, extractionRange, snapshot?.extraction_open ? '#80b7d2' : '#567f90', '#36546a35'); ctx.setLineDash([]);
  groundCircle(extraction, 92, '#3c657250');
  text(snapshot?.extraction_open ? '撤离点 · 已开放' : '撤离点 · 等待开放', extraction, extractionRange * scale / Math.SQRT2 + 22, '#7eabbc', 9);
  const player = snapshot ? pose('player', now) ?? snapshot.position : null;
  if (player && state.pointer && state.phase === 'active') {
    const direction = aim(), length = Math.hypot(...direction);
    const tip = screen([player[0] + direction[0] / length * 110, player[1] + direction[1] / length * 110]);
    ctx.strokeStyle = '#a2e5ce66'; ctx.setLineDash([3, 4]); ctx.beginPath(); ctx.moveTo(...screen(player)); ctx.lineTo(...tip); ctx.stroke(); ctx.setLineDash([]);
  }
  if (player && state.pulse && now - state.pulse.at < 400) {
    const t = (now - state.pulse.at) / 400;
    groundCircle(player, 30 + t * 80, state.pulse.skill ? `rgba(157,224,199,${.7 - t * .7})` : `rgba(144,185,217,${.6 - t * .6})`);
  }
  if(state.phase==='active' && state.selectedGround){for(const s of state.config?.playerSkills??[])if(isGroundSkill(s))groundCircle(state.selectedGround,s.radius,'#9de0c7','#9de0c710',1);}
  const playerWarnings=state.phase==='active'?(snapshot?.pve?.player_area_warnings??[]):[];canvas.dataset.playerAreaWarnings=String(playerWarnings.length);
  for(const a of playerWarnings){groundCircle(a.center,a.radius,'#9de0c7','#9de0c740',3);text(`${a.skill_id} · ${(Math.max(0,a.resolves_at_ms-snapshot.logical_ms)/1000).toFixed(1)}秒`,a.center,-a.radius*scale/Math.SQRT2-8,'#9de0c7',11);}
  const fields=state.phase==='active'?(snapshot?.pve?.player_fields??[]):[];canvas.dataset.playerFields=String(fields.length);
  canvas.dataset.lineFields=String(fields.filter(f=>f.line).length);
  for(const f of fields){const warning=snapshot.logical_ms<f.activates_at_ms;if(f.line)groundCapsule(f.line,warning?'#f5a37d':'#9de0c7',warning?'#f5a37d20':'#9de0c745');else groundCircle(f.center,f.radius,warning?'#f5a37d':'#9de0c7',warning?'#f5a37d20':'#9de0c745',2);text(`${f.skill_id} · ${warning?'预警':f.completed_ticks+'/'+f.total_ticks} · ${(Math.max(0,f.expires_at_ms-snapshot.logical_ms)/1000).toFixed(1)}秒${f.contact_policy?' · 预算 '+f.contact_policy.remaining_hits:''}`,f.center,-f.radius*scale/Math.SQRT2-8,warning?'#f5a37d':'#9de0c7',11);}
  const flights=state.phase==='active'?(snapshot?.pve?.piercing_projectiles??[]):[];canvas.dataset.piercingProjectiles=String(flights.length);
  for(const p of flights){groundCircle(p.position,Math.max(p.radius,4),'#d9b8ff','#d9b8ff90',2);text(`${p.skill_id} · ${p.remaining_hits}/${p.max_hits}`,p.position,-20,'#d9b8ff',11);}
  drawAreaWarnings();
    drawTimedPosition();
  drawHeavyCasts();
  drawSupportCasts();
  const objects = [];
  for (const wall of bounds.walls) {
    // Split long walls into sortable sections so nearby actors can pass in front or behind them.
    for (let x = wall.min[0]; x < wall.max[0]; x += 32) for (let y = wall.min[1]; y < wall.max[1]; y += 32) {
      const block = { min: [x, y], max: [Math.min(x + 32, wall.max[0]), Math.min(y + 32, wall.max[1])] };
      objects.push({ depth: project([(x + block.max[0]) / 2, (y + block.max[1]) / 2])[1], draw: () => drawBlock(block) });
    }
  }
  if (snapshot) {
    for (const resource of snapshot.resources) objects.push({ depth: project(resource.position)[1], draw: () => drawResource(resource) });
    if (snapshot.pve) {
      for (const chest of snapshot.pve.chests ?? []) objects.push({depth:project(chest.position)[1],draw:()=>drawChest(chest)});
      for (const monster of snapshot.pve.monsters.filter(m => m.hp > 0)) objects.push({depth:project(monster.position)[1],draw:()=>drawTarget(monster)});
    } else if (snapshot.target) {
      const target = { ...snapshot.target, position: pose('target', now) ?? snapshot.target.position };
      objects.push({ depth: project(target.position)[1], draw: () => drawTarget(target) });
    }
    objects.push({ depth: project(player)[1], draw: () => drawPlayer(player, snapshot, now) });
  }
  objects.sort((a, b) => a.depth - b.depth).forEach(object => object.draw());
  for (const wall of bounds.walls) text('静态障碍', [(wall.min[0] + wall.max[0]) / 2, wall.max[1]], 18, '#95aeb6', 9);
  state.floating = state.floating.filter(item => now - item.at < 1100);
  for (const item of state.floating) {
    ctx.globalAlpha = Math.max(0, 1 - (now - item.at) / 1100);
    text(`−${item.amount}`, item.position, -45 - (now - item.at) / 35, item.own ? '#ed9e91' : '#f3ce9d', 15); ctx.globalAlpha = 1;
  }
  requestAnimationFrame(draw);
}
requestAnimationFrame(draw);

render();
try {
  const catalog=await api('/api/catalog');
  $('scenario').replaceChildren(...catalog.profiles.map(profile=>{const option=document.createElement('option');option.value=profile.scenario;option.textContent=profile.label;return option;}));
  await loadScenario(catalog.defaultScenario); render();
  const previous = await api('/api/current');
  if (previous) {
    await loadScenario(previous.scenario,previous);
    restore(previous); await recover(previous);
  }
} catch (error) { fail(error); } finally { state.booting=false; render(); }
