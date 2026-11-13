# Node 接入快速上手（成员模式，协议 6）

目标：十分钟内用 Node 跑通一场完整对局——创建对局 → 全员就绪 → 移动/战斗/拾取 → 撤离结算 → 账本落账。本文是 [SDK 接入指南](sdk-integration.md)（六调用契约与恢复语义）的可运行前置篇；协议细节、错误分类、崩溃恢复以该指南为准。

## 0. 边界声明

按 [AGENTS.md](../../AGENTS.md)，SDK 不携带游戏配表。本文内嵌的 JSON 是**机制验证用合成输入**（与 Rust 合成测试同性质：验证内核机制，不代表任何玩法数值）；生产配置由接入方按 [configuration-v2](../interfaces/configuration-v2.md) 与 [members-v5.schema.json](../interfaces/members-v5.schema.json) 自行维护。`adapters/node/` 的宿主库是**参考实现**：协议封装、发布流回放、业务闭环调度可直接复用，生产接入方用自己的存储与传输替换 `TwoMemberDatabase`/autoAck 部分。

## 1. 前置条件

- **Node ≥ 22**（宿主库使用内置 `node:sqlite`；22/23 会有实验警告，Node 24 起稳定）。
- 内核二进制：`cargo build --locked --release --bin kernel-host`（产出 `target/release/kernel-host`）。
- 无需 `npm install`：适配层只用 Node 标准库。

## 2. 准备三份输入文件

放到一个工作目录（下文记作 `$WORK`）：

**`$WORK/catalog.json`** —— 配置清单（schema 1），把场景别名映射到配置文件：

```json
{"schema_version": 1, "default_scenario": "quickstart",
 "active": {"quickstart": "arena.json"}, "archives": []}
```

**`$WORK/arena.json`** —— 最小成员对局配置（schema 7 / mechanism 105，2 席 2 怪）。字段逐项含义见 [interfaces/](../interfaces/)；这里给出的是能通过内核校验链的完整最小值：

```json
{
  "schema_version": 7, "mechanism_version": 105,
  "id": "quickstart", "source_commit": "quickstart", "purpose": "mechanism regression",
  "step_ms": 30, "terminal_ms": 900000,
  "geometry": {"min": [0, 0], "max": [2000, 2000], "walls": []},
  "spawn": [40, 40], "extraction_point": [1960, 1960],
  "extraction_unlock_ms": 4000, "extraction_channel_ms": 3000,
  "target": {"id": "target", "position": [1000, 1000], "hp": 100000, "atk": 10, "armor": 0,
    "speed": 0, "radius": 2, "aggro_range": 8, "attack_range": 2, "interval_ms": 1000, "drop_quantity": 0},
  "basic": {"mechanism": "projectile", "windup_ms": 30, "interval_ms": 300, "range": 600, "projectile_speed": 300},
  "skill": {"mechanism": "unit_effect", "id": "skill", "range": 600, "cost": 5, "windup_ms": 30,
    "recovery_ms": 60, "cooldown_ms": 300, "damage_bps": 10000, "ignore_armor": true, "unavoidable": true},
  "additional_skills": [],
  "inventory_capacity": 24, "stack_limit": 99, "item_id": "item", "carried_quantity": 0,
  "kill_score": 0, "extraction_score": 0, "cultivation_coefficient": 0, "death_multiplier": 1,
  "defender_realm_atk": 100, "defense_cap": 0.7,
  "input_capacity": 64, "output_capacity": 1024, "history_capacity": 64,
  "asset_contract": {"entry_cost_asset_id": "entry", "reward_asset_id": "reward"},
  "result_contract": {"extraction": {"result_id": "extract", "policy": "preserve"},
    "death": {"result_id": "death", "policy": "attrition"}},
  "members": {"capacity": 2, "spawns": [[40, 40], [64, 40]], "deployment_timeout_ms": 30000,
    "publication_queue_capacity": 8, "projectile_capacity": 8, "area_capacity": 8},
  "replication": {"ack_timeout_ms": 1000, "baseline_ttl_ms": 10000, "full_interval_ms": 1000,
    "terminal_grace_ms": 2000},
  "params": {
    "player_radius": 8, "pickup_range": 24, "extraction_range": 24,
    "movement": {"base": 180, "bonus": 120, "reference": 8, "log_base": 100},
    "regen": {"interval_ms": 1000, "near_radius": 0, "near_ratio": 0, "far_ratio": 0},
    "basic_spirit_on_hit": 0, "orb_hp_ratio": 0, "orb_spirit_ratio": 0, "defense_scale": 2,
    "minimum_damage": 1, "critical_multiplier": 1, "metal_multiplier": 1,
    "resource_ttl_ms": 30000, "drop_min_radius": 0, "drop_max_radius": 0,
    "basic_projectile_radius": 1, "player_projectile_ttl_ms": 1000, "entry_years": 1,
    "death": {"slot_ratio": 0, "quantity_ratio": 0, "minimum_quantity": 1},
    "initial_player": {"hp_points": 1000, "spirit_points": 200, "atk_points": 100,
      "def_points": 0, "spd_points": 8},
    "runtime": {"io_timeout_ms": 500, "drain_timeout_ms": 1000, "reconnect_grace_ms": 500,
      "publish_batched_ms": 60, "nav_budget": 64, "nav_repath_ms": 30, "ai_decision_ms": 30,
      "resource_sweep_ms": 100, "call_timeout_ms": 1000, "max_attempts": 2,
      "max_catchup_steps": 2, "max_lag_steps": 8}
  },
  "pve": {
    "template_id": "npc", "map_id": "quickstart", "width": 50, "height": 50, "cell_size": 40,
    "palette": [{"terrain": "floor", "dotDamagePerSecond": 0, "elevation": 0, "maxClimbHeight": 0,
      "maxDropHeight": 0, "walkable": true, "speedMultiplier": 1, "blocksProjectiles": false,
      "blocksVision": false}],
    "cells_rle": [[0, 2500]],
    "monster_positions": [[240, 40], [300, 90]],
    "stats": {"hp": 600, "spirit": 100, "atk": 20, "def": 0, "spd": 8, "crit": 0, "dodge": 0},
    "combat": {"radius": 2, "attack_interval_ms": 1000, "aggro_range": 120, "melee_range": 2,
      "cast_chance": 0, "return_distance": 40, "lifesteal_ratio": 0, "mark_max": 0, "mark_duration_ms": 0,
      "projectile": {"mechanism": "projectile", "id": "npc-cast", "cost": 10, "windup_ms": 60,
        "cooldown_ms": 600, "recovery_ms": 60, "range": 400, "speed": 300, "radius": 4,
        "ttl_ms": 2000, "damage_bps": 3000},
      "heal": {"mechanism": "budget_heal_channel", "id": "npc-heal", "cost": 0, "windup_ms": 30,
        "cooldown_ms": 300, "recovery_ms": 60, "hp_threshold": 0, "heal_bps": 0, "interval_ms": 30,
        "ticks": 1}},
    "drop_table": {"guaranteed": [{"itemId": "item", "quantityMin": 2, "quantityMax": 3}], "random": []},
    "items": {"item": {"name": "item", "stackLimit": 99}},
    "leash": 200, "help_range": 80, "memory_range": 400,
    "status_rules": {"max_active_per_entity": 6, "defense_bonus_cap_bps": 10000,
      "attack_bonus_cap_bps": 10000, "shield_cap_bps": 10000},
    "status_definitions": [{"id": "burn", "mechanism": "periodic_damage", "damage_bps": 2000,
      "tick_interval_ms": 120, "duration_ms": 2400, "duplicate_policy": "refresh", "source_scoped": true}]
  },
  "combat_rules": {
    "profiles": {
      "member": {"tier": 0, "defense_baseline_attack": 100, "hit_bps": 10000, "critical_chance_bps": 0,
        "dodge_bps": 0, "critical_multiplier_bps": 10000, "power_points_bps": 0},
      "npc": {"tier": 0, "defense_baseline_attack": 100, "hit_bps": 10000, "critical_chance_bps": 0,
        "dodge_bps": 0, "critical_multiplier_bps": 10000, "power_points_bps": 0}
    },
    "default_member_profile": "member", "default_npc_profile": "npc",
    "member_profiles": {}, "npc_profiles": {}, "elements": [],
    "tier_multipliers": {"0": 10000}, "element_multipliers_bps": {}, "weapon_skill_multipliers_bps": {},
    "basic_attack": {"damage_kind": "physical", "fixed": 0, "cultivation_bps": 10000,
      "armor_penetration_bps": 0, "life_steal_bps": 0, "life_steal_timing": "contact",
      "unavoidable": false, "ignore_armor": false, "can_crit": true},
    "skills": {"skill": {"damage_kind": "true_damage", "fixed": 0, "base_power": 60,
      "cultivation_bps": 10000, "armor_penetration_bps": 0, "life_steal_bps": 0,
      "life_steal_timing": "contact", "unavoidable": true, "ignore_armor": true, "can_crit": false}},
    "npc_attacks": {"npc": {"damage_kind": "physical", "fixed": 40, "cultivation_bps": 10000,
      "armor_penetration_bps": 0, "life_steal_bps": 0, "life_steal_timing": "contact",
      "unavoidable": false, "ignore_armor": false, "can_crit": false}},
    "periodic_damage": {}
  }
}
```

两个易错点已经替你踩过：`npc_attacks` 若缺怪物模板键，NPC 攻击为零；伤害规则里 `cultivation_bps: 0` 会把整段伤害归零（只剩 1 点保底）。

**`$WORK/account.json`** —— 两名玩家的账本初始态（由宿主业务系统提供；此处为示例值）：

```json
[
  {"participant_id": "p0", "principal": "principal-p0", "revision": 0,
   "stats": {"hp_points": 1000, "spirit_points": 200, "atk_points": 100, "def_points": 0, "spd_points": 8},
   "assets": {"entry": 100000, "item": 0, "reward": 0}},
  {"participant_id": "p1", "principal": "principal-p1", "revision": 0,
   "stats": {"hp_points": 1000, "spirit_points": 200, "atk_points": 100, "def_points": 0, "spd_points": 8},
   "assets": {"entry": 100000, "item": 0, "reward": 0}}
]
```

## 3. 十分钟脚本

存为 `$WORK/quickstart.mjs`，把开头三个路径改成你的实际路径：

```js
// Node ≥ 22。最小成员模式接入：建账本 → 建对局 → 就绪 → 战斗拾取 → 撤离结算。
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';

// —— 改这三行 ——
// 仓库接入：SDK=检出目录、二进制在 target/release/；交付包接入：SDK=包根、
// 二进制在 bin/kernel-host、宿主库在 hosts/node/（包内文档已自动改写路径）。
const SDK = '/absolute/path/to/game-rust-sdk';          // SDK 检出目录
const WORK = '/absolute/path/to/work';                  // 放三份输入文件的目录
const BINARY = `${SDK}/target/release/kernel-host`;

const { TwoMemberHost, TwoMemberDatabase } = await import(`${SDK}/adapters/node/two-member-host.mjs`);
const read = (name) => JSON.parse(readFileSync(`${WORK}/${name}`, 'utf8'));
const manifest = `${WORK}/catalog.json`;
const arena = read('arena.json');
const accounts = read('account.json');

// 1) 配置内容哈希：create 必须携带，配错即 CONFIG_MISMATCH。
const configHash = execFileSync(BINARY, ['--configs', manifest, '--config-hash']).toString().trim();

// 2) 宿主账本（参考实现 = 本地 SQLite；生产替换为你的账户存储）。
const database = new TwoMemberDatabase(`${WORK}/ledger.sqlite`);
const key = { id: `quickstart-${Date.now()}`, incarnation: 1 };
const record = {
  id: key.id, scenario: 'quickstart', config: arena, accounts, manifest, generation: 0,
  originalCreate: {
    protocol_version: 6, key, request_id: 'create',
    config_id: arena.id, config_hash: configHash, seed: 7, publication_mode: 'interactive',
    participants: accounts.map((a, i) => ({
      participant_id: a.participant_id, authenticated_principal: a.principal,
      connection: { id: `c${i}`, generation: 1 },
    })),
  },
};
database.writeRoom(record);
for (const a of accounts) {
  database.writeState(key.id, a.participant_id, {
    items: {}, warehouse: 0, cultivation: 0, revision: a.revision, years: 0,
    admissions: 0, settlements: 0, compensations: 0, reconciliations: 0,
    balances: a.assets, requests: [], journal: [], fenced: [], audit: [],
  });
}

// 3) 起宿主。autoAck=true 自动回报发布回执；业务请求（读取/扣费/结算）
//    由 pump() 经 database.execute 自动闭环——生产宿主在这里换成你的存储适配。
const events = [];
const host = new TwoMemberHost({
  binary: BINARY, manifest, database, record, autoAck: true,
  onEvent: (_pid, ev) => events.push(ev.kind === 'private' ? ev.event : ev), // 私有事件带信封
});
await host.start();   // create
await host.pump();    // 排空首批输出（触发读取/扣费业务闭环）
await host.joinAndReady(); // join → 收全量快照 → 回执 → combat_ready → active
await host.pump();

const pid = 'p0';
const snap = () => host.member(pid).snapshot;               // V6 回放快照
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const dist = (a, b) => Math.hypot(a[0] - b[0], a[1] - b[1]);
const until = async (ok, label, timeout = 30000) => {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    await host.pump();
    if (ok()) return;
    await sleep(60);
  }
  throw new Error(`${label}: timeout`);
};
// 移动是速度制的：方向输入后每个 30ms tick 滑行 speed*dt（约 9 单位）。
// 远距每 120ms 校正一次方向；进入 60 单位内改用"短脉冲+停步"逐 tick 蠕进，
// 否则惯性滑行会在目标两侧来回振荡，永远进不了拾取/撤离圈。
// 关键：每轮都要 pump()——输入只是意图，快照与事件要靠排水才向前推进；
// 不 pump 的话快照永远停在旧位置，脚本会以为"卡住"。
// 超时与停滞双保险：任何循环都必须有界，卡住就抛错给人看。
const move = async (target, tol = 20, timeout = 30000) => {
  const deadline = Date.now() + timeout;
  let last = null, lastMoved = Date.now();
  for (;;) {
    if (Date.now() > deadline) throw new Error(`move timeout -> ${JSON.stringify(target)}`);
    await host.pump();
    const p = snap()?.self?.position;
    if (!p) { await sleep(60); continue; }
    const d = dist(p, target);
    if (d <= tol) return;
    const key = `${Math.round(p[0])},${Math.round(p[1])}`;
    if (key !== last) { last = key; lastMoved = Date.now(); }
    else if (Date.now() - lastMoved > 3000) throw new Error(`move stuck at ${JSON.stringify(p)} -> ${JSON.stringify(target)}`);
    await host.input(pid, { kind: 'move', direction: [(target[0] - p[0]) / d, (target[1] - p[1]) / d] });
    if (d < 60) {
      await sleep(40);
      await host.input(pid, { kind: 'move', direction: [0, 0] }).catch(() => {});
    }
    await sleep(d < 60 ? 60 : 120);
  }
};

// 4a) 战斗：走向怪群，灵力够就放技能，否则普攻，直到打空。
// 注意 UnitTarget 只接受 {id, generation}（deny_unknown_fields）：
// 快照里 monsters[i].entity 是 {kind,id,generation}，直接整包传会被拒。
await move([240, 40], 380);
const kills = () => events.filter((e) => e.kind === 'monster_defeated').length;
const unitTarget = (m) => m.unit_target ?? { id: m.entity.id, generation: m.entity.generation };
for (let guard = 0; guard < 200 && kills() < 2; guard++) {
  const s = snap();
  const m = (s.monsters ?? []).find((m) => m.hp > 0);
  if (!m) break;
  const action = s.self.spirit >= 5
    ? { kind: 'cast_unit', skill_id: 'skill', target: unitTarget(m) }
    : { kind: 'attack_unit', target: unitTarget(m) };
  try { await host.input(pid, action); } catch { /* 冷却中/灵力不足：下轮再试 */ }
  await host.pump();
  await new Promise((r) => setTimeout(r, 150));
}
await until(() => kills() >= 2, 'kill both monsters');
console.log('kills:', kills());

// 4b) 拾取：走向掉落，逐个拾取（expected_revision 用快照里的库存版本，冲突则重读）。
// 快照里 self.inventory 是 {"item": 数量} 映射（成员 V6）；
// 单人 V4 的 pve.inventory 是 [item, 数量] 数对——两种都兼容。
const invOf = (v) => Array.isArray(v)
  ? v.reduce((n, e) => n + (e[1] ?? e.quantity ?? 0), 0)
  : Object.values(v ?? {}).reduce((n, q) => n + q, 0);
const inv = () => invOf(snap().self?.inventory);
for (let guard = 0; guard < 12 && (snap().resources ?? []).length > 0; guard++) {
  const r0 = snap().resources[0];
  await move(r0.position, 15);
  for (let rev = snap().self.inventory_revision; ; rev++) {
    try { await host.input(pid, { kind: 'pickup', resource_id: r0.id, expected_revision: rev }); break; }
    catch (e) { if (!/revision|conflict/i.test(String(e.code ?? e.message))) { rev = -2; break; } }
  }
  await host.pump();
  await new Promise((r) => setTimeout(r, 400));
}
console.log('inventory:', inv());

// 4c) 撤离：等解锁 → 走到撤离点 → 停步（滑行会冲出范围报 OUT_OF_RANGE）→ 读条。
await until(() => snap().extraction_open, 'extraction unlock');
await move([arena.extraction_point[0], arena.extraction_point[1]], 18, 40000);
await host.input(pid, { kind: 'move', direction: [0, 0] });
await host.input(pid, { kind: 'extract_start' });
await until(() => ['settling', 'terminal'].includes(snap().self.phase ?? snap().phase), 'extraction');
// 收尾判定用账本事实：pending() 投影的是"已持久检查点"里的未决操作，
// 终局后内核不再产出新检查点，它可能永远停在旧值（needsDrain 竞态下还会返回 null）。
await until(() => { host.pump(); return database.state(key.id, pid).settlements >= 1; }, 'settlement', 20000);
await host.pump();

// 5) 收尾：先 close（幂等）再释放进程；之后仍需把剩余输出处理完。
await host.close();
await host.pump();
const ledger = database.state(key.id, pid);
console.log('phase:', snap().phase ?? 'closed');
console.log('ledger:', { settlements: ledger.settlements, balances: ledger.balances, items: ledger.items });
await host.dispose();
```

运行：

```sh
node quickstart.mjs
```

预期输出（掉落量 2~3 随机，总数在 4~6 间浮动）：

```
kills: 2
inventory: 6
phase: active        ← 本地回放快照停在终局前最后一次发布；权威终态看账本与 Closed 事件
ledger: {
  settlements: 1,
  balances: { entry: 99999, item: 6, reward: 0 },
  items: { entry: 99999, reward: 0 }
}
```

全程约 10~30 秒。`settlements: 1` 证明撤离结算已原子落账；`entry 100000→99999` 是入场扣费；拾取的 6 件在 `balances.item` 里随 `preserve` 结果策略保留入仓。

## 4. 这条路径里宿主承担了什么

对照[接入指南](sdk-integration.md)的六调用，脚本里每一句的真实含义：

| 脚本动作 | 内核侧发生的 | 生产宿主要替换的 |
|---|---|---|
| `new TwoMemberDatabase` + `writeRoom/writeState` | —— | 你的账户存储（原子提交 + 幂等凭据） |
| `--config-hash` → `originalCreate.config_hash` | 目录版本注册 | —— |
| `host.start()` → create | 读取/扣费业务请求排队 | —— |
| `host.pump()` | 排空输出并持久化，业务请求自动闭环 | 换成你的存储执行 + 回执提交 |
| `joinAndReady()` | join → 全量发布 → 回执 → combat_ready | 生产客户端走同一握手，参考 [web/app.js](../../web/app.js) |
| `host.input(...)` | 提交玩家意图（sequence/连接代自动管理） | —— |
| `autoAck: true` | 发布即报 Sent | 换成真实发送边界的回执 |
| `settlements >= 1`（账本事实） | 结算回执已闭环，责任推进 | `host.pending()` 投影已持久检查点、可能滞后；生产收尾以持久账本 + `Closed.pending_operations` 为准 |

## 5. 常见坑（全部实测踩过）

1. **`npc_attacks` 缺键或 `cultivation_bps: 0`** → NPC 伤害恒为 1（保底）。伤害规则是“模板键 → 规则”查表，不是全局默认。
2. **撤离前不停步** → 移动有惯性滑行，走到点立刻 `extract_start` 可能已滑出 `extraction_range`，报 `OUT_OF_RANGE`。到位后发一个 `move [0,0]`。
3. **私有事件带信封** → 成员模式私有事件是 `{"kind":"private","event":{...}}`，计数前先解包；且成员模式击杀事件叫 `monster_defeated`（单人协议叫 `monster_killed`）。
4. **`host.events` 只保留最近 512 条** → 要统计全程事件，用 `onEvent` 回调自己累计。
5. **准入到 combat_ready 之间有宽限**（`runtime.reconnect_grace_ms`，默认 500ms）→ 就绪握手必须紧跟创建；`joinAndReady()` 已按此编排，自研客户端别在中间做耗时工作。
6. **`pickup` 等输入需要 `expected_revision`** → 以快照的 `inventory_revision` 为起点；收到 `REVISION_CONFLICT` 说明期间有别的提交落账，重读快照拿新版本再试。脚本用"从快照版本起小步探测"的简化写法，生产客户端应每次重读最新快照。
7. **进程退出时机** → `close` 之后仍要继续 `pump` 并处理剩余业务输出，`pending_operations` 归零才算正常收尾；直接退出会留下未决责任。
8. **`UnitTarget` 只接受 `{id, generation}`** → 反序列化开了 `deny_unknown_fields`，把快照的 `entity` 对象（多一个 `kind` 字段）原样塞进 `cast_unit/attack_unit` 会静默收到 `INVALID_MESSAGE`。战斗脚本长时间"无伤"先查这里。
9. **不 pump 就没有新快照** → `submit_input` 只是把意图交给内核；回执、发布、事件全部要在 `drain_outputs`（宿主库的 `pump()`）里才落地。自建驱动的循环忘了排水，快照就会永远停在旧位置，表现为"世界不动了"。
10. **`host.pending()` 可能滞后或返回 null** → 它投影的是"最近一次持久化的检查点"里的未决操作，终局后不再有新检查点时不会归零；`needsDrain` 未清时还会返回 null。运行期用它看水位没问题，**收尾判定要用账本事实**（如 `settlements`）加 `Closed.pending_operations`。

## 6. 下一步

- 六调用完整语义、输出处理顺序、崩溃恢复责任账：[sdk-integration.md](sdk-integration.md)
- 配置字段逐项契约（含 JSON Schema）：[interfaces/](../interfaces/)，从 [configuration-v2](../interfaces/configuration-v2.md) 与 [members-v5](../interfaces/members-v5.md) 开始
- 库行为细节与边界用例：[adapters/node/two-member-host.test.mjs](../../adapters/node/two-member-host.test.mjs)（测试即样例）
- Rust 进程内接入：[examples/minimal-host](../../examples/minimal-host/)，指南见 [sdk-integration.md](sdk-integration.md)「运行最小消费方」
- 能力/上限的机器可读清单：`kernel-host --sdk-info`
