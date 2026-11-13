# 普攻冷却跟进接入

SDK 0.38.0，机制 53，schema 7，协议 6。多人 PVE 对局可声明以下根字段。

```json
{"basic_followup":{"grace_ms":150}}
```

宽限为固定步的整数倍，介于 0 和 1000 ms。普通攻击成功提交后，在它自身冷却内发送原 `{"kind":"attack"}` 可缓冲下一击。Rust 选择当时最近且合法的 NPC。每人只保留最新有效请求。

本人快照的 `self.basic_followup` 包含 `action_id`、`target` 的 `id` 和 `generation`、`promotes_at_ms`、`expires_at_ms`。首个释放固定步和截止均由 Rust 给出。目标在释放和提交时重验，失效则通过本人 `action_rejected` 报告。进入准备后观察原 `pending_action`，并用 `action_committed` 的 `skill:false` 确认实际下一击。

取消发送 `cancel_action`。有效技能接管和连接切换清除旧队列。移动与停止保留队列。H5 用「排队普攻」提交原 `attack`，用「取消排队普攻」清除。样例在 `examples/basic-followup-host`，交付目录为 `samples/basic-followup`。 [批次范围](../batches/SDK-5-36-basic-followup.md)

SDK 0.105.0 的机制 105 对局可启用 `basic.settle_on_end`。投送期间 `basic_ready_at_ms` 暂时表示投送截止，结束后更新为从结束时起算的实际冷却。下一击缓存随实际冷却重新排程，届时重新校验目标。一次有效普攻至多按 `params.basic_spirit_on_hit` 回灵一次，发放时检查回灵封锁及当前缺口。落空仍结算已释放动作的冷却。保护或重连会撤销在途回灵资格并保留已释放动作的冷却责任。[穿透普攻配置](member-piercing-v1.md)

## 定向与径向普攻波面

SDK 0.106.0，schema 7，机制 105。配置 `basic.settle_on_end: true`，然后在 `basic.wave` 声明 `plan_id`、`geometry`、`speed`、`target_cap`、`primary_damage_bps` 和 `additional_damage_bps`。 `geometry` 接受 `{"kind":"directional","cone_degrees":84}` 或 `{"kind":"radial"}`。速度单位为地图像素每秒，伤害倍率单位为万分比。范围、前摇和间隔沿用 `basic` 的字段。`wave` 与 `piercing` 互斥，波面速度取 `wave.speed`，兼容所需的基础 `projectile_speed` 保留在配置里。样例中任选一段 `basic` 替换同版本对局配置的对应段。[扇、盾、塔参数样例](../guides/config-cookbook.md#combat-rules)basic-wave-profiles.json) · [配置 Schema](member-piercing-v1.schema.json)

宿主继续发送 `attack` 或 `attack_unit`。释放冻结原点、几何、候选和到达时间。定向按轴向投影，径向按直线距离。服务器在达到计划时间的第一个固定步重验来源和目标身份，再进入伤害结算。

- `basic_waves` 是公有数组，含波面 ID、计划 ID、来源、原点、几何、范围、速度、释放时刻和剩余的 `{target, at_ms}`。启用波面的配置以空数组表示当前无计划，其他配置省略字段。
- `basic_wave_started` 带初始计划。`basic_wave_contact` 带波面 ID、来源、目标与 `scheduled_at_ms`，表示计划接触进入伤害裁决。实际伤害以 `damage` 等战斗结果事件为准。`basic_wave_ended` 带波面 ID 和 `resolved` 或 `source_invalid` 原因。
- 协议 6 增量用整个 `basic_waves` 数组替换本地值，重连全量帧恢复剩余计划。波面有独立发布字段和容量约束。[发布 Schema](publications-v6.schema.json)

## 逐成员普攻配置与附带状态

SDK 0.107.0 起，schema 7，机制 105。宿主可以在 `combat_rules.basic_profiles` 注册命名普攻配置，SDK 0.113.0 起最多 32 份。每份配置使用 `basic` 的字段并开启 `settle_on_end`。可信账号记录和 Create 席位可直接用 `basic_profile` 选择一个 ID。两种武器选择均省略时，成员沿用对局的 `basic`。当前有效配置由本人快照的 `self.basic_profile` 发布。

`basic.effects` 或命名配置的 `effects` 可以引用 `pve.status_definitions`。

- `on_release_self_status` 在计划成功释放时施加自身状态。
- `on_end_damage_self_status` 在本次投送造成实际 HP 伤害后，于终态施加一次自身状态。
- `on_end_damage_target_status` 提供 `choices` 和可选的 `target_profile_bps`。动作结束时按首伤顺序向每个仍有效的受伤目标应用一个随机候选。品阶映射使用 `combat_rules.npc_profiles` 或默认 NPC 档位，修正值向零取整，最小非零幅度为 1 万分点。候选使用已有的叠层、刷新、抗性和状态容量规则。

可把样例 `basic_profiles` 合入 `combat_rules`，把 `status_definitions` 合入 `pve`。例中的 `elite` 与 `boss` 要对应对局已有的战斗档位。盾系只在实际扣血后授予吸收，塔系提交时授予减伤、终态才抽取镇压。[武器配置样例](../guides/config-cookbook.md#combat-rules)basic-weapon-profiles.json) · [配置 Schema](member-piercing-v1.schema.json) · [本批范围](../batches/SDK-5-88-basic-profiles.md)

## 同刻接触与固定延迟普攻

SDK 0.108.0，schema 7，机制 105。`basic.impact` 接受 `plan_id`、`damage_bps` 和 `timing`，并要求开启 `settle_on_end`。配置了 `impact` 的普攻使用自己的接触计划。`timing` 支持以下两种形式。

- `{"kind":"hitscan","refraction":{"radius":220,"damage_bps":6500}}` 在提交固定步处理主目标，成功后接触冻结的副目标。省略 `refraction` 可只处理主目标。
- `{"kind":"fixed_delay","delay_ms":350}` 在释放后固定延迟到达锁定单位。发布在途计划时保留冻结原点及目标代次，目标移动不改变到达时间。

`basic_impacts` 是协议 6 的公有数组，记录尚未到达的固定延迟计划。它含 `id`、`plan_id`、`source`、`kind`、`origin`、`target`、`released_at_ms` 和 `impact_at_ms`。增量用整个数组替换，结束后发送空数组。重连全量恢复剩余计划。即时计划通过 `basic_impact_started`、`basic_impact_contact` 和 `basic_impact_ended` 观察。接触事件表示目标进入裁决，实际结果仍取伤害和恢复事件。

`basic.effects.on_end_damage_self_heal_max_hp_bps` 只在本动作造成过实际气血伤害时结算一次，使用来源当前有效最大气血，经过受治疗倍率和缺口。对应事件为 `heal_resolved`，其 `heal_kind` 是 `basic_attack`。镜、鼎的原参数已合入[武器配置样例](../guides/config-cookbook.md#combat-rules)basic-weapon-profiles.json)。[本批范围](../batches/SDK-5-88-basic-impact.md) · [配置 Schema](member-piercing-v1.schema.json)

## 同刻多段与主目标爆破

SDK 0.109.0，schema 7，机制 105。`basic.impact.timing.kind` 为 `hitscan` 时，可在 `timing.followup` 配置一种多段模式。该字段与旧 `timing.refraction` 互斥。

- `alternating_refraction` 使用 `radius`、`damage_bps`、`max_contacts`、`repeat_decay_bps`、`minimum_damage_bps`。首段计入上限 2 至 8。往返以主目标为枢纽，各目标重复接触按上一段系数独立衰减。
- `target_burst` 使用 `radius`、`damage_bps` 和 `max_targets`。主目标计入上限 2 至 8，副目标各接触一次。几何中心为释放时锁定的主目标位置。

这两种模式的 `basic_impact_contact.segment` 包含从 0 开始的 `index`、`role`（`primary`、`secondary`、`return` 或 `blast`）、本段冻结的 `origin` 和 `target`。结束事件的 `segments` 数组重述实际进入裁决的段，顺序和索引一致。读取实际扣血仍使用 `damage` 事件。旧单次折射和固定延迟省略这些段字段。

宿主通过可信的 `basic_profile` 选用样例中的原始器型 ID。[武器配置样例](../guides/config-cookbook.md#combat-rules)basic-weapon-profiles.json) · [本批范围及其余器型分组](../batches/SDK-5-88-basic-hitscan-patterns.md) · [配置和事件 Schema](member-piercing-v1.schema.json)

## 开放式方向普攻引导

SDK 0.110.0，schema 7，机制 105。`basic.beam` 接受 `plan_id`、`interval_ms`、`half_width`、`target_cap` 和 `damage_bps`，与其他普攻投送配置互斥，并要求 `settle_on_end`。射程读取 `basic.range`，时钟从成功启动开始，无额外前摇或自动释放上限。贯世明光按源参数配置 440 射程、22 半宽、150 ms 周期、8 目标和每跳 1800 基点。[武器配置样例](../guides/config-cookbook.md#combat-rules)basic-weapon-profiles.json)

输入及顺序如下。

1. `{"kind":"start_attack_beam","direction":[1,0]}` 启动会话。
2. 从 `basic_beam_started` 或 `basic_beams` 取得本人的服务端 `action_id`。转向发送 `{"kind":"aim_attack_beam","action_id":"…","direction":[0,1]}`。
3. 松手发送 `{"kind":"end_attack_beam","action_id":"…"}`。首跳前松手保留最后合法方向的首跳，后续松手停止未来脉冲。

`basic_beams` 是全量和增量的可选公有数组。当前 action 出现在其中时，客户端继续发送移动或停止意图，并暂停本地位置预测。每跳先收到 `basic_beam_pulse`，其后可能有 `basic_beam_contact` 和 `damage`。脉冲与接触的 `at_ms` 是实际结算时间。结束事件 `basic_beam_ended` 包含同一 action ID 和实际跳数。保护或重连后的快照移除该 action，服务端在下一固定步发送保护终态。自然松手、取消和眩晕中断仍按实际已结算的接触进入终态，死亡及离场直接清理。[本批范围](../batches/SDK-5-88-basic-beam.md) · [配置和输入 Schema](member-piercing-v1.schema.json)

## 波面位移与交替普攻

SDK 0.111.0，schema 7，机制 105。定向波面可声明可选的 `basic.wave.behavior`，攻击输入仍用 `attack` 或 `attack_unit`。

- `push` 使用 `distance`、`rehit_immunity_ms`、`resistant_profiles` 和 `reduced_status`。普通怪物沿冻结方向受推。抵抗档位改用配置的 `movement_slow` 状态，要求同源刷新，多个来源取有效减速中的最强值。
- `alternate` 使用 `pull_distance`、`resistant_profiles`、`mark_duration_ms`、`marked_damage_bonus_bps` 和 `alternate`。内层配置径向波面的 `plan_id`、`range`、`speed`、`primary_damage_bps` 和 `additional_damage_bps`，沿用外层 `target_cap`。初始定向波牵引普通怪物并给有效接触且存活的怪物加门印，之后径向波面对本来源门印提高伤害并消费。

接触后的位移沿地图可行走层裁剪，怪物位置随原快照和增量同步。用 `self.basic_wave_phase` 读取下一次准备的形态，`primary` 表示定向，`alternate` 表示径向。该字段仅发给配置交替波面的席位。成功释放后立即切换。以 `basic_waves[].plan_id` 识别已释放的形态，已释放的波面在同局重连后保留。门印和击退间隔由服务端持有。[样例配置](../guides/config-cookbook.md#combat-rules)basic-weapon-profiles.json) · [配置 Schema](member-piercing-v1.schema.json) · [本批验收标准](../batches/SDK-5-88-basic-wave-controls.md)

## 普攻实体弹道续段

SDK 0.112.0，schema 7，机制 105。开启 `basic.settle_on_end`，在 `basic.projectile_followup` 声明 `plan_id`、`primary_damage_bps` 和 `pattern`。输入仍为 `attack` 或 `attack_unit`。范围、速度和前摇来自同一普攻配置。

- `companion` 使用 `delay_ms` 和 `damage_bps`。主段有效接触后等待配置时间，再从角色当时的位置发出一枚续段。原目标失效时，按距离与 ID 从射程内的合法候选改选一次。
- `volley` 使用 `hits`、`interval_ms`、`damage_bps` 和 `attack_speed_bps_per_hit`。有效主段接触累计进度，按已存数量参与普攻时钟。触发后，小剑从主剑释放点飞向触发接触时的目标中心，飞行距离以原射程为上限。目标代次失效时弹道仍执行到端点或障碍。所有续段结束后清空储剑。

公有弹道的 `basic_segment` 记录 `action_id`、`plan_id`、`index`、`launched_at_ms` 和 `aim_point`。主段索引为 0。同动作多个在途段可同时发布。目标失效后继续飞行的段会省略 `unit_target`。订阅 `projectile_started`、`projectile_contact`、`projectile_ended` 和 `damage` 区分发射、有效接触、终止和实际伤害。本人 `self.basic_projectile_progress` 提供储剑数量、上限及配置的攻速加成。连接恢复时，以全量帧重建当前在途段与进度，后续发射继续通过事件接收。[样例配置](../guides/config-cookbook.md#combat-rules)basic-weapon-profiles.json) · [配置 Schema](member-piercing-v1.schema.json) · [本批验收标准](../batches/SDK-5-88-basic-projectile-followups.md)

## 普攻弹射、驻留与召回

SDK 0.113.0 增加两种 `basic.projectile_followup.pattern`，沿用原普攻输入。

- `seal_recall` 使用 `required_seals`、`overtravel`、`corridor_half_width`、`additional_damage_bps`、`recall_damage_bps`、`settle_delay_ms`、`interval_ms` 和 `slow_status`。已落地的剑印在 `members[].basic_seals` 公开。每次落剑发布 `basic_attack_sword_landed`，包含 `source`、`action_id`、`position`、`stored`。召回发射时移除对应剑印，单条路径按 `seal_outbound` 或 `seal_recall` 投影。
- `ricochet` 使用 `max_targets`、`search_radius`、`damage_bps`、`mark_duration_ms`、`hits_to_stun`、`stun_status` 和 `resistant_profiles`。外抛段的 `basic_segment.role` 为 `ricochet`。可选 `return_to_owner` 包含 `speed_bps`、`shield_status`、`base_max_hp_bps`、`per_extra_max_hp_bps` 和 `max_hp_bps`，回程角色为 `return_to_owner`，只发布开始和结束。

连接恢复读取公有剑印及在途弹道，状态效果仍使用统一状态和护盾协议。配置带召回时需为同时召回的弹道和怪物数量预留输出预算。样例的回光叠影使用可信配置 `shield_double_quake_return` 表达基础档的接触、伤害和回程参数。[样例配置](../guides/config-cookbook.md#combat-rules)basic-weapon-profiles.json) · [配置 Schema](member-piercing-v1.schema.json) · [验收标准](../batches/SDK-5-88-basic-projectile-paths.md)

## 普攻生成的固定领域

SDK 0.114.0 起，可在 `basic.wave.field` 声明独立领域，并在 `members.basic_field_capacity` 配置 1 至 12 个领域席位。技能仍使用 `members.area_capacity`。冷载入根据两类计划、地图 NPC 数量和输出队列一起校验预算。

- `trigger: {"kind":"primary_contact"}` 在主目标有效接触位置创建领域。 `trigger: {"kind":"landing","impact_radius":150}` 配合径向波配置使用。波面发布 `geometry.kind: "landing"` 与 `geometry.from`，`origin` 是已冻结的落点，`range` 是落地伤害半径。落地后创建领域。
- `sampling: "live"` 在每跳读取有效攻击力，`"frozen"` 在原普攻释放时冻结伤害预算。 `plan_id`、`radius`、`duration_ms`、`interval_ms`、`damage_bps`、`target_cap` 和 `max_per_source` 都来自配置。周期需整除持续时间，期限当步执行最后一跳。可选 `owner_reduction_bps` 要求该配置每个来源只留一个领域。
- 领域随 `areas` 发送，`mechanism` 为 `basic_field`，`skill_id` 对应领域 `plan_id`，`basic_action_id` 关联触发它的普攻。周期和替换仍由 `area_tick` 与 `area_ended` 发布，替换原因为 `replaced`。全量快照包含仍存续的领域。达到来源上限或全局容量时，只有该来源同类旧场可替换。

样例中风眼留痕和落塔镇岳分别使用实时预算与冻结预算。宿主合入其 `basic_profiles` 时，也将所需 `basic_field_capacity` 合入房间 `members`。[样例配置](../guides/config-cookbook.md#combat-rules)basic-weapon-profiles.json) · [配置 Schema](member-piercing-v1.schema.json) · [验收标准](../batches/SDK-5-88-basic-world-fields.md)

## 延时附状态与地面领域

SDK 0.115.0 起，协议 6、schema 7、机制 105。延时普攻可在 `basic.impact.contact_status` 配置叠层周期减益及其致死扩散。仅附状态的配置要求 `damage_bps: 0`。继承层数按死亡时层数计算并向上取整，新状态使用传播时的来源数值。普通叠层使用 `stacks_per_application`，缺省为一层。

配置 `basic.impact.timing.kind: "ground_field"` 后，可发 `{"kind":"attack_ground","position":[460,440]}`，也可沿用 `attack_unit` 或最近目标 `attack`。协议 6 的 `basic_impacts` 中，`ground_field` 计划携带 `position`，单位延时计划携带 `target`。Rust 消费者通过 `Option<UnitTarget>` 读取目标。领域生成后沿用 `areas`，NPC 状态按来源投影给对应成员。

将样例的 `basic_profiles`、`elements`、`periodic_damage` 合入 `combat_rules`，将 `status_definitions` 合入 `pve`，将 `basic_field_capacity` 合入 `members`。状态持续至到期前，领域包含期限当步的一跳。[配置样例](../guides/config-cookbook.md#combat-rules)basic-burning-profiles.json) · [验收标准](../batches/SDK-5-88-basic-burning-fields.md)

## 普攻蓄力会话

协议 6、schema 7、机制 105 支持 `basic.charge`。先发 `{"kind":"start_basic_charge"}`，读取本人快照中的 `basic_charge.session_id`。用 `release_basic_charge` 携带会话 ID 和单位 `target` 或冲阵 `direction`，或用 `cancel_basic_charge` 携带同一 ID。匹配的释放失败仍结束会话。保护、眩晕和退场都会移除会话。活跃冲阵配置在本人快照发布 `basic_charge_ready_at_ms`。

蓄力档位由 `upgrades.after_ms` 决定，前摇和追击沿用释放时选定的档位。若启用通用追击，准入范围为该档射程加上原普攻允许的额外追击距离，并受对局追击上限约束。架盾使用共享减伤上限、护盾和扣血链，示例的反击配置采用真实伤害。冲阵沿地形裁剪后的路线选取敌人，普通盾击仍可使用 `attack` 或 `attack_unit`。

将所需 `basic_profiles` 合入 `combat_rules`，对应 `status_definitions` 合入 `pve`。样例引用的 `elite` 和 `boss` 必须在战斗档位中存在。 [样例配置](../guides/config-cookbook.md#combat-rules)basic-charge-profiles.json) · [验收标准](../batches/SDK-5-88-basic-charge.md)

## 蓄力飞行与叠层终击

SDK 0.117.0 新增 `basic.charge.behavior.kind: "flight"` 和 `basic.stacked_wave`。释放飞行体时，`release_basic_charge` 携带 `target`、`position` 或 `direction` 中的一项。Rust 调用者构造 `Action::ReleaseBasicCharge` 时显式设置可选的 `position`。飞行快照沿用 `projectiles`，`basic_segment` 的 `charged_flight` 角色携带权威 `tier` 和冻结 `aim_point`。沿途接触和落点爆炸分别发布 `basic_flight_contact` 和 `basic_flight_burst`。

叠层终击从本人状态视图读取 `stack_status` 对应的层数与期限。普通波终态有实际扣血才加层，达到该状态上限后在下一次成功释放时选用 `finisher`，结束后清除当次选用的状态实例。样例复用现有灼烧定义、`fire` 元素及周期伤害规则，以及塔系终态镇压定义。 [配置样例](../guides/config-cookbook.md#combat-rules)basic-flight-and-stack-finisher.json) · [验收标准](../batches/SDK-5-88-basic-flight-and-stack-finisher.md)

## 装备实例与局内换装

SDK 0.118.0 起，宿主可把 `weapon_catalog` 合入 `combat_rules`，并在可信账号或 Create 席位传入 `weapon: {instance_id, model_id, morph_id?}`。同一席位只选 `weapon` 或 `basic_profile`。目录的 `models` 将模型映射到家族，`families` 为家族声明 `base_profile` 和可选的 `morphs`。模型未知时拒绝。器型缺失、未知或属于其他家族时使用家族基础配置。 [样例目录](../guides/config-cookbook.md#combat-rules)weapon-catalog.json) · [配置 Schema](member-piercing-v1.schema.json)

使用完整样例目录时，把四组配置的 `basic_profiles`、状态及依赖字段按本页各节的说明合入同一场景。 [武器配置](../guides/config-cookbook.md#combat-rules)basic-weapon-profiles.json) · [灼烧配置](../guides/config-cookbook.md#combat-rules)basic-burning-profiles.json) · [蓄力配置](../guides/config-cookbook.md#combat-rules)basic-charge-profiles.json) · [飞行与终击配置](../guides/config-cookbook.md#combat-rules)basic-flight-and-stack-finisher.json)

可信宿主调用 Rust `Client::set_equipped_weapon`、JSONL `set_equipped_weapon` 或 Node `TwoMemberHost.setEquippedWeapon`。请求含对局身份、成员及代次、`operation_id`、`expected_revision` 和可选 `weapon`。省略 `weapon` 即卸下。首次修订为 0，每次接受加 1。已接受操作可用同一请求重试，128 项为单局记录上限。弹道、波面等仍在结算时，换装会返回 `INVALID_STATE`，可在终态后重试原操作。换装结束开放式射线并按旧配置结算，终态 `reason` 为 `equipment_changed`。本人私有快照 `self.equipment` 和 `weapon_equipment_changed` 提供修订和实际装备。

宿主负责确认实例所有权及持久库存。需要跨进程核对回执时，先持久保存含该操作的检查点。旧 Rust 调用方构造 `ParticipantSeat` 时为新增字段设置 `weapon: None`，并按需处理新增事件。 [协议 Schema](publications-v6.schema.json) · [验收标准](../batches/SDK-5-88-weapon-equipment.md)
