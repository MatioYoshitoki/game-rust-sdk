# 局内加持与机会点

SDK 0.151.0 在机制 105、schema 7 和协议 6 支持可选的 `world_boons`。使用通用战斗配置和状态属性上限。

| 字段 | 契约 |
|---|---|
| `capacity` | 每席 1～3 格，槽位编号从 1 开始。 |
| `interaction_range` | 0.1～1000 个 World 单位。领取检查视线及完整角色足迹的可达路径。 |
| `definitions` | 最多 32 个加持 ID。每项含 1～3 个 `levels`，每级最多 8 个 `attribute`、`modifier_bps` 条目。逐级保留或增加先前的属性量。首级为空时，须有费用、护盾、战斗触发或检查点反制绑定。 |
| `opportunities` | 最多 32 个机会。每项含唯一 `id`、`position`、`scope` 和 `offer`。ID 至多 64 个 UTF-8 字节。 |

`scope` 为 `per_member` 或 `shared`，同一来源对每席最多成功一次。共享来源的 `shared_uses` 省略时为全局一份，也可显式声明 1～8 份且不超过本局座位数。授予使用 `offer.kind: grant` 和 1～8 个已登记的 `choices`。强化使用 `offer.kind: upgrade`，只能升已持有能力一级。可选 `guardians` 使用本局怪物实体 ID。可选 `human_only` 隔离自动席位。可选 `cost` 包含 `item_id` 和 `quantity`，从本局拾取的堆叠库存扣除。自动席位可使用相同规则获取和升级，满槽时由人工请求选择替换。

客户端发送 `claim_boon`，提供 `opportunity_id`、`boon_id`、`expected_revision` 和 `expected_inventory_revision`。授予时优先选择最小空槽。满格须提供 `replace_slot`，替换后新能力从一级开始。强化和有空槽时省略 `replace_slot`。SDK 在一个提交里扣费、消耗机会并写入槽位。失败保持机会和库存。成功向本人发布 `boon_claimed`。

每条增益的 `modifier_bps` 为 1～10000。支持 `atk`、`def`、`spd`、`attack_speed`、`max_hp`、`max_spirit`、`skill_power_bonus`、`healing_received`、`recovery_strength` 和 `damage_reduction`。SDK 将各槽的量加入既有属性计算和上限，槽位独立于有时限的状态。升阶、替换沿用已推进的普攻时钟和当前生命、灵力，属性上限下降时按新上限裁剪。

本人 `self.boons` 包含槽位、修订和近距可见机会，随本人全量与增量发布。机会的 `available` 反映守护、资格、使用和占用状态，费用在提交时验证。显式配置 `shared_uses` 的机会另发布 `shared_remaining`。重连保持槽位和已消耗记录。进入结算阶段时回收加持。 [发布与输入 Schema](publications-v6.schema.json)

`requires_checkpoint` 可引用同一 World 的计时检查点计划。领取时校验本人达到该计划的 `required` 进度。原生检查点使用时间窗、位置和通行性确认事实。客户端继续发送领取意图。

检查点反制可分别引用加持等级来延长新窗口或提升该次专属伤害。 [检查点反制](publications-v6.md#检查点反制)

可选 `interaction_ms` 至少为一个 World 步，至多 60000ms，并须早于本局终止。启用时 `claim_boon` 先发布私有 `boon_claim_started`，本人 `pending_claim` 给出机会 ID、能力 ID 和预计完成时点。共享来源在读条期间最多由一席占用。主动取消、开启其他战斗或交互、离距、受击、材料修订变化、连接保护和退场会释放占用。中断发布 `boon_claim_interrupted`，成功才扣材料和共享份额。完成时重验原条件与修订。

## 条件触发

`combat_rules.triggers` 可加 `requires_boon`。`id` 引用本局加持定义。`npc_only` 为真时要求战斗事实的另一方是原生 NPC。成员必须持有该加持且具备战斗资格。召唤体、周期与派生触发均排除。可用事实为 `direct_hit`、`basic_hit`、`damage_dealt`、`damage_taken`、`shield_absorbed` 和 `kill`，沿用各事实原有时点和效果校验。

规则的 `effect` 用于一级。可选 `higher_level_effects` 依次给出其余各级同类型效果。可用 `apply_self_status`、`apply_other_status`、`heal_from_damage` 和 `restore_spirit`。条件、计数、概率、冷却及 `combat_trigger_fired` 继续走原生触发器。升阶与重连保留原计数和冷却。替换立即停止新触发并回收该成员用旧加持施加的状态。普通技能及其他成员的同定义状态保留各自实例。 [战斗配置 Schema](combat-rules-v1.schema.json)

多个加持对同一目标施加的负向 `healing_received` 取最强。它与普通状态已结算的减疗合计再取较强者，正向受疗修正按原规则相加。适用状态到期、被清除或失去来源后，即时以仍有效的状态重算。

## 灵力成本与护盾

- `spirit_cost_bps` 可按等级配置 1～9999 的费用保留比例，逐级持平或下降。多项同时持有时取最低比例。原价为零仍为零，正费用向下取整且最低为 1。普通施法、AI 与预约使用同一公式。已预约动作沿用预约金额，弹道和锚点重施读取当次提交价格。生命费用与冷却沿用各自规则。
- `shield.capacity_bps` 按等级配置 1～10000 的最大生命比例，逐级持平或上升。`shield.rebuild_ms` 从耗尽起计时，至少一个 World 步，至多 3600000ms。须启用普通护盾上限，且加持 ID 与已有状态定义分开。
- 每席最多持有一项护盾加持。首次领取即时尝试建盾。普通盾先吸收，加持盾接收原生 NPC 来源的剩余伤害。成员、召唤及无 NPC 来源的伤害绕过加持盾。普通盾仍受原上限约束，总量上限取普通与加持容量中的较高值。
- 重建读取当前级别和最大生命。升阶保留旧盾余额与耗尽时钟，保护期间保留时点，恢复战斗资格后最多重建一次。无状态槽或护盾额度时，等待后续步重新尝试。替换和退场移除旧盾。护盾余额及事件继续通过协议 6 原有护盾字段发布。

加持定义的 `boonDefinition` 片段可用于字段校验。等级长度、顺序、状态引用与每步输出预算由 Rust 目录加载器校验。 [配置 Schema](combat-rules-v1.schema.json)
