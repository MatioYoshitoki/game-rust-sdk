# 配置手册：技能与机制速查（config cookbook）

按"**需求 → 机制 → 字段 → 最小示例 → 版本门槛**"组织。每节示例取自本仓库实测跑通的配置（战斗、技能、开箱、拾取、撤离、战败全流程断言过），不是凭空拼凑的字段罗列。

覆盖标注：

- ✅ **全链路实测**：本仓库驱动经真实协议栈跑通并逐事件断言
- 🔧 **配置已验证**：通过内核校验链并在对局中运行（施法路径未逐帧断言）
- 📄 **见契约**：字段以对应 interface 契约文档为准（本文只给门槛与要点）

一页读不完所有机制（111 篇契约在 [interfaces/](../interfaces/)）；本文解决的是**主线路径**：让一个新接入者用最小配置把"移动→战斗→掉落→拾取→开箱→撤离→结算"整条链配出来，再按需扩机制。

## 配置骨架

所有对局配置都长在两套骨架之一上（字段逐项含义见 [configuration-v2](../interfaces/configuration-v2.md)）：

- **单人 PVE**：`schema_version: 4` + `mechanism_version: 34`（技能门槛 26，线场 34）——支持宝箱/储物袋（`chest_spawns` + `treasure_rules`），**成员模式不支持**
- **成员**：`schema_version: 7` + `mechanism_version: 105`——1~8 席（`members.capacity`）、协议 6 delta 发布、`combat_rules` 战斗公式、逐席构建

两套骨架的完整最小可运行版本直接复制 [Node 快速上手](node-quickstart.md)（成员 2 席版，✅ 实测）——它就是一份合法配置。

## 版本门槛速查（机制 → 最低 mechanism_version）

> 完整机器可读清单：`kernel-host --sdk-info`（`support.json` 同源）。`schema_version` 与 `mechanism_version` 的配对规则见 [validate.rs](../../src/config/validate.rs)：`37..=105 → schema 7`、`36 → schema 6`、`35 → schema 5`、单人技能族 `26..=34 → schema 4`。

<!-- GATES-BEGIN（generated，勿手改；漂移请运行 --fix） -->
**单人族（schema 4 / 协议 3）**

| 门槛 | sdk_info 键 | 上限 |
|---|---|---|
| 19 | `timedStatus` | — |
| 22 | `absorptionShield` | — |
| 23 | `periodicEffects` | — |
| 25 | `timedStatus.healingReceived` | — |
| 26 | `playerSkills` | 上限 2 |
| 27 | `playerSkills.selfStatus` | — |
| 28 | `playerSkills.unitProjectile` | — |
| 29 | `playerSkills.groundCircle` | — |
| 30 | `playerSkills.groundField` | — |
| 31 | `playerSkills.contactPolicy` | — |
| 32 | `playerSkills.piercingProjectile` | — |
| 33 | `playerSkills.projectileRepeat` | — |
| 34 | `playerSkills.lineField` | — |
| 105 | `memberSkills.trustedLoadouts` | — |

**成员族（schema 7 / 协议 6，1~8 席）**

| 门槛 | sdk_info 键 | 上限 |
|---|---|---|
| 38 | `multiplayerPeriodic` | — |
| 39 | `multiplayerShields` | — |
| 40 | `multiplayerCombat` | — |
| 41 | `memberAreas` | — |
| 42 | `memberLineAreas` | — |
| 43 | `memberPiercing` | — |
| 44 | `memberSkills` | 上限 8 |
| 45 | `skillHpCost` | — |
| 46 | `castMovement` | — |
| 47 | `directionCharge` | 上限 5000 |
| 48 | `chargeReaim` | — |
| 49 | `fixedChannel` | 上限 30000 |
| 50 | `projectileRecast` | — |
| 51 | `skillFollowup` | — |
| 52 | `recoveryCancel` | — |
| 53 | `basicFollowup` | — |
| 54 | `lockedBasicTarget` | — |
| 55 | `basicPursuit` | — |
| 56 | `autoBasicPursuit` | — |
| 57 | `memberLootDrop` | — |
| 58 | `memberItemUse` | — |
| 59 | `memberItemCooldown` | — |
| 60 | `memberItemSpirit` | — |
| 61 | `memberItemCooldownGroup` | — |
| 62 | `memberItemDualRecovery` | — |
| 63 | `memberItemRatioRecovery` | — |
| 64 | `memberItemQualityRecovery` | — |
| 65 | `memberItemSelfStatus` | — |
| 66 | `memberItemStatusQuality` | — |
| 67 | `memberItemDamageReduction` | — |
| 68 | `memberPeriodicDamageReduction` | — |
| 69 | `memberPeriodicHealingReceived` | — |
| 70 | `memberItemHealingReceived` | — |
| 71 | `memberRecoveryOrb` | — |
| 72 | `memberRecoveryStrength` | — |
| 73 | `memberSpiritRecoveryLock` | — |
| 74 | `memberNpcSpiritRecoveryLock` | — |
| 75 | `memberOverhealShield` | — |
| 76 | `memberPeriodicOverhealShield` | — |
| 77 | `memberInstantSelfHeal` | — |
| 78 | `memberAllyHeal` | — |
| 79 | `memberAllyHot` | — |
| 80 | `memberAllyShield` | — |
| 81 | `memberAllyOverhealShield` | — |
| 82 | `memberAllyHotOverhealShield` | — |
| 83 | `memberNpcSilence` | — |
| 84 | `memberControlImmunity` | — |
| 85 | `memberControlDurationReduction` | — |
| 86 | `memberFirstControlImmunity` | — |
| 87 | `memberItemCleanse` | — |
| 88 | `memberNpcRoot` | — |
| 89 | `memberNpcStun` | — |
| 90 | `memberNpcMovementSlow` | — |
| 91 | `memberMovementHaste` | — |
| 92 | `memberCooldownReduction` | — |
| 93 | `memberArmorPenetration` | — |
| 94 | `memberCriticalChance` | — |
| 95 | `memberCriticalDamageBonus` | — |
| 96 | `memberDodgePenalty` | — |
| 97 | `memberDodgeChance` | — |
| 98 | `memberAccuracy` | — |
| 99 | `memberNpcHit` | — |
| 100 | `memberDamageImmunity` | — |
| 101 | `memberNpcUnavoidable` | — |
| 102 | `memberUnavoidable` | — |
| 103 | `directIgnoreArmor` | — |
| 104 | `memberSkillPower` | — |
| 105 | `basicBeam` | — |
| 105 | `basicImpact` | — |
| 105 | `basicProfiles` | — |
| 105 | `basicProjectileFollowup` | — |
| 105 | `basicWave` | — |
| 105 | `basicWorldFields` | — |
| 105 | `castLifecycle` | — |
| 105 | `characterBuild` | — |
| 105 | `checkpointZones` | — |
| 105 | `combatRules` | — |
| 105 | `contestedEquipment` | — |
| 105 | `encounterProgress` | — |
| 105 | `extractionLifecycle` | — |
| 105 | `matchMode` | — |
| 105 | `memberCastAnchor` | — |
| 105 | `memberConflict` | — |
| 105 | `memberGroundTrap` | — |
| 105 | `memberSummons` | — |
| 105 | `npcGroundField` | — |
| 105 | `npcGroundStrike` | — |
| 105 | `npcMovingEnterArea` | — |
| 105 | `paidResourceEffects` | — |
| 105 | `resourceCommit` | — |
| 105 | `resourceSteal` | — |
| 105 | `settlementProgression` | — |
| 105 | `spectators` | — |
| 105 | `trainingTargets` | — |
| 105 | `unitEffects` | — |
| 105 | `weaponEquipment` | — |
| 105 | `worldGeometry` | — |
| 105 | `zeroEntryCost` | — |
| 105 | `zoneExposure` | — |
<!-- GATES-END -->

## 移动与寻路 ✅

- 输入：`move{direction}`（方向速度制，惯性滑行）、`stop{}`、`navigate{destination}`（单人 PVE 点地寻路）
- 速度公式：`base + bonus × clamp(ln(spd_points/参数.reference)/ln(参数.log_base), 0, 1)`，配置在 `params.movement`
- 实测坑：贴近目标用"短脉冲+停步"，120ms 校正周期的滑行会在目标两侧振荡（见 [Node 快速上手](node-quickstart.md) 常见坑 2/9）

## 普攻（basic）✅

`basic.mechanism` 决定形态，公共字段 `windup_ms/interval_ms/range`：

| 形态 | mechanism | 门槛 | 契约 |
|---|---|---|---|
| 方向弹道 | `projectile` | 默认 | [basic-projectiles 契约见 basic-impact 等](../interfaces/) |
| 蓄力 | `charge` | 47 | [direction-charge-v1](../interfaces/direction-charge-v1.md) |
| 波面 | `wave` | 88 批次 | [member-piercing-v1](../interfaces/member-piercing-v1.md) |
| 光束 | beam 系 | 88 批次 | [interfaces/ 同族](../interfaces/) |

成员模式伤害走 `combat_rules.basic_attack`（damage_kind/cultivation_bps/armor_penetration_bps/life_steal/can_crit）；单人走老公式 `stats.atk × damage_bps`。

✅ 实测：`projectile` 普攻 + `combat_rules.basic_attack{cultivation_bps:10000}`，伤害数字与公式逐发对账（100 = atk 100 × 100%）。

<a id="player-skills"></a>
## 玩家技能族（skill + additional_skills）

- **单人**：主技能 `skill` + `additional_skills` **恰好 1 条**（共 2，机制 ≥26）
- **成员**：`skill` + `additional_skills` 共 **1~8 条**（机制 ≥44）；运行时换装用 `set_skill_loadout`（带 `expected_revision`，成员模式）

公共字段：`id/cost(灵力)/windup_ms/recovery_ms/cooldown_ms/damage_bps/range`。按 mechanism 分：

| mechanism | 输入 | 门槛 | 覆盖 | 契约 |
|---|---|---|---|---|
| `projectile` | cast（方向） | 26 | 🔧 配置验证 | [player-skills-v1](../interfaces/player-skills-v1.md) |
| `self_status` | cast_self | 27 | 📄 | [self-skills-v1](../interfaces/self-skills-v1.md) |
| `unit_projectile` | cast_unit | 28 | ✅ S1 全链路 | [unit-targets-v1](../interfaces/unit-targets-v1.md) |
| `unit_effect` | cast_unit | ≥40（成员） | ✅ M1 全链路 | [unit-effects-v1](../interfaces/unit-effects-v1.md) |
| `ground_circle` | cast_ground | 29 | 📄 | [interfaces](../interfaces/) |
| `ground_field` | cast_ground | 30 | 📄 | [interfaces](../interfaces/) |
| `line_field` | cast | 34 | 📄 | [line-fields-v1](../interfaces/line-fields-v1.md) |
| `piercing_projectile` | cast | 32（成员 43） | ✅ 25 分钟长稳连发 | [piercing-projectiles-v1](../interfaces/piercing-projectiles-v1.md) |
| `ally_heal` | — | 78 | 📄 | [member-ally-heal-v1](../interfaces/member-ally-heal-v1.md) |
| `ally_shield` | — | 80 | 📄 | [member-ally-shield-v1](../interfaces/member-ally-shield-v1.md) |

✅ 实测片段（成员模式主技能，600 射程内杀 600 血怪，伤害 = base_power 60 × 100%…见 [combat-rules-v1](../interfaces/combat-rules-v1.md) 公式）：

```json
{"mechanism": "unit_effect", "id": "skill", "range": 600, "cost": 5, "windup_ms": 30,
 "recovery_ms": 60, "cooldown_ms": 300, "damage_bps": 10000, "ignore_armor": true, "unavoidable": true}
```

通用扩展（各机制可选字段，见契约）：`charge`（蓄力,47）、`channel`（引导,49）、`recast`（再施放）、`recovery_cancel`、`cast_movement`、`resource_commit`（预扣-提交-退款）、`hp_cost`（生命成本,45）、`on_commit_self_statuses` / `on_hit_target_statuses`。

> ⚠️ `UnitTarget` 只接受 `{id, generation}`（deny_unknown_fields）——快照里 `entity` 多一个 `kind` 字段，整包传会被静默拒成 `INVALID_MESSAGE`。

## NPC 与怪物

- 布点：`pve.monster_positions`（≤32）；模板字段 `stats{hp,spirit,atk,def,spd,crit,dodge}` + `combat{radius,attack_interval_ms,aggro_range,melee_range,cast_chance,...}`；AI（仇恨/追击/leash 回位/支援）见 [npc-combat-ai-v1](../interfaces/npc-combat-ai-v1.md)
- **NPC 伤害两条铁律**（实测踩过）：
  1. 成员模式：`combat_rules.npc_attacks` 必须有怪物 `template_id` 键，缺键 = 零攻击
  2. 伤害规则里 `cultivation_bps: 0` 把伤害清零（只剩 1 点保底）——固定伤害也要给 10000
- 单人模式 `melee_range` 是绝对距离，必须 ≥ 双方半径和（8+2 → 给 16），否则怪贴脸也打不到

✅ 实测：怪群围殴致死（M2）、护卫怪打断撤离读条（M3）、leash 回位回满血（S1 前置探针）。

## 掉落、资源与宝箱

- **击杀掉落**：`pve.drop_table{guaranteed,random}`（`itemId/quantityMin/quantityMax/chance`）；`params.drop_orb` 类回复球默认开。✅ 实测：3 态齐验（掉落可见→拾取入包→TTL 过期清扫）
- **地面资源寿命**：`params.resource_ttl_ms` + `params.resource_sweep_ms`（清扫周期）——0.185.0 起随世界时钟正确推进
- **拾取**：输入 `pickup{resource_id, expected_revision}`（用快照 `inventory_revision`），`params.pickup_range` 圈内
- **宝箱（仅单人 schema 4）**：`chest_spawns[{id,position}]` + `treasure_rules`：

```json
{"template": {"id": "cache", "name": "Supply Cache", "respawn_delay_ms": 1000,
  "mechanism": "layered_seal_ground_loot",
  "seal": {"layers": 2, "layer_duration_ms": 150, "interrupt_retention_ratio": 0.5}},
 "loot": {"guaranteed": [{"itemId": "item", "quantityMin": 3, "quantityMax": 3}], "random": []},
 "interaction_range": 48, "ground_capacity": 16}
```

✅ 实测全链路（S1）：`open_chest`（开始破解）→ `chest_layer_completed`×2 → `chest_unlocked` → 二次 `open_chest`（领取地面掉落）→ 拾取。可选守卫 `template.guardian`（`GuardianPending/GuardianAlive` 门）见契约。单人 `melee_range` 同样适用于怪打玩家；`interaction_range` 圈内才能开箱。

<a id="statuses"></a>
## 状态（statuses）

`pve.status_definitions[]` + `pve.status_rules`（容量与上限）：

```json
{"id": "burn", "mechanism": "periodic_damage", "damage_bps": 2000,
 "tick_interval_ms": 120, "duration_ms": 2400, "duplicate_policy": "refresh", "source_scoped": true}
```

- 机制门槛：periodic 38 / shield 39；`status_definitions` 必须配 `status_rules`
- 施加途径：技能 `on_hit_target_statuses` / `on_commit_self_statuses`、`combat_rules` 接触规则、NPC 施法
- 🔧 配置验证通过（burn 定义随实测对局装载）；触发链路逐条见 [statuses-v1](../interfaces/statuses-v1.md)、[periodic-damage-v1](../interfaces/periodic-damage-v1.md)、[absorption-shields-v1](../interfaces/absorption-shields-v1.md)

## 撤离与终局结算 ✅

- 解锁：`extraction_unlock_ms`（激活后逻辑时间）→ 快照 `extraction_open`
- 读条：`extraction_channel_ms`，站进 `extraction_point` 的 `params.extraction_range` 圈内 `extract_start`；**受击打断**（`extraction_interrupted`）——M3 实测打断→反杀→二次撤离
- 终局：`result_contract{extraction,death}` 各自 `result_id` + `policy`（preserve=背包入仓 / attrition=折损）；结算请求经业务闭环原子落账（宿主账本 `settlements +1`），失败走补偿/对账
- ✅ 实测：成功撤离入仓（S1/M1）、战败结算（S2/M2）、账本余额逐分对账

## 成员席位与逐席构建

- `members{capacity 1~8, spawns[], deployment_timeout_ms, publication_queue_capacity, projectile_capacity, area_capacity}`
- create 的 `participants[]` 每席可带 `combat_build` / `skills` / `weapon` / `basic_profile` 覆盖默认面板（[character-build-v1](../interfaces/character-build-v1.md)、[combat-rules-v1](../interfaces/combat-rules-v1.md)）
- 运行时：`set_skill_loadout` / `set_equipped_weapon`（宿主 RPC，非玩家输入）；重连保留已提交 CD/计划
- 📄 换装与逐席 build 未在本仓库驱动实测，字段以两份契约为准

## 校验错误速查

| 错误 | 含义 | 处置 |
|---|---|---|
| `CONFIG_*` | 配置校验拒绝（创建前） | 按 [configuration-v2](../interfaces/configuration-v2.md) 修字段；`--config-hash` 须重新生成 |
| `CONFIG_MISMATCH` / `VERSION_MISMATCH` | id/hash/协议版本不匹配 | 用目录登记的精确版本，禁止静默换图 |
| `REVISION_CONFLICT` | 乐观锁冲突 | 重读快照拿新 `*_revision` 重试 |
| `INVALID_STATE` / `NOT_READY` | 动作与阶段不符（CD/后摇/未就绪） | 等快照就绪重试，勿换 ID 重放 |
| `OUT_OF_RANGE` / `NO_RESOURCE` / `CAPACITY` | 玩法拒绝（射程/资源/背包） | 展示给玩家，按新快照重操作 |
| `INVALID_MESSAGE` / `UNSUPPORTED` / `STALE_SEQUENCE` | 报文非法 | 修 schema/序列号，勿原样重试 |

完整错误语义与宿主策略表：[sdk-integration.md](sdk-integration.md)「业务结果和错误处理」。

## 事件名速查（接线用）

| 事件 | 单人协议 | 成员协议（注意） |
|---|---|---|
| 击杀 | `monster_killed` | `monster_defeated`（私有事件包 `private` 信封，先解包） |
| 掉落/拾取 | `resource_spawned` / `picked_up` | 同名（`resource_taken` 同现） |
| 宝箱链 | `chest_unlock_started → chest_layer_completed → chest_unlocked → chest_opened` | 单人专属（成员模式无宝箱） |
| 撤离 | `extraction_started` / `extraction_interrupted` | 同名（私有信封） |
| 结算 | `result_settlement_applied` / `asset_settlement_applied` | 同名 |

## 下一步

- 主线路径跑通后的机制扩展：直接读对应 [interfaces/](../interfaces/) 契约（每篇都有"本批不做什么"防止过度期待）
- 接入闭环与恢复语义：[sdk-integration.md](sdk-integration.md)
- 十分钟可运行最小路径：[node-quickstart.md](node-quickstart.md)
