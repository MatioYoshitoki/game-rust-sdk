# 多成员私有增量契约 v6

0.22.0/协议6/schema7/机制37，[DP01～DP12](../batches/SDK-5-20-member-delta.md)在实现前冻结。CreateV6沿固定名单1～8成员，配置members与新增replication；原协议1～5保留原full输出，v6金融/责任仍复用*_v4 DTO，只有发布使用publication_v6。

PublicationV6为{recipient_id,connection,stream_epoch,revision,payload}，外层Output带key/id/logical_ms。payload是{kind:full,snapshot,reason}或{kind:delta,base_publication_id,base_revision,delta}。full原因有限：initial、periodic、expired、failure、timeout、resync、lifecycle、terminal、size。delta为{logical_ms,state?,own?,members?,monsters?,resources?,projectiles?}；state完整替换{phase,active_at_ms,extraction_point,extraction_open}，own完整替换本人块，避免null与省略混淆。每个集合变化为{upsert,removed,order}，members/monsters键是完整EntityRef，其余为String ID；order恰为修改后全部唯一键，禁止重复、缺项、未知移除或同键同时移除/更新。不存在字段不变，空order代表空集合。

Rust先生成该接收者权限视图再diff。每成员只允许一个已drain未终态帧，新变化合并到最新目标，不依赖未Sent帧；未drain帧可被最新替换。Sent建立该publication_id/revision的基线，Enqueued不算。失败/取消/超时/基线过期或request_full重置本人流epoch，补最新full，旧epoch所有输出/回执失效；周期及体积策略可发同epoch新full。ready只能引用当前epoch已Sent full。

客户端Envelope为{key,publication_id,recipient_id,connection,stream_epoch,revision,payload}。SDK Rust与JS解码器验证所有者、连接代、epoch、revision和完整base身份；只有full允许新连接/新epoch起步。精确重复幂等，旧帧/冲突/非法更新拒绝且旧缓存保持。丢基线走request_full{stream_epoch}，复用原Input的认证/顺序/去重；不允许请求别人的流或任意World patch。客户端只解码显示，Node转发Rust原始payload，不能追加一个隐藏full冒充增量。

终局full可在Root Closed后接受传输回执/重同步；只发送冻结的退出视图，不补生存者新状态。terminal_grace_ms后私有publication_expired明确放弃未送达视图，释放瞬时缓存，不宣称Sent；所有金融责任独立继续。已过期拒绝发布恢复。检查点不含发布基线，重启只恢复Closed金融账，不热恢复World。

replication严格字段：ack_timeout_ms 100～60000，baseline_ttl_ms ack_timeout_ms～60000，full_interval_ms 100～baseline_ttl_ms，terminal_grace_ms 2×ack_timeout_ms～60000。客户端输出是不可变事实，不提供认证凭据或其它成员私有库存/SP/CD/金额。

终局显示的grace期限从成员开始settling/terminal或Root关闭的较早时刻计算；失败/重同步不得延长。到期只结束该成员瞬时发布，不暂停同伴或改变金融责任。

退出视图从成员进入settling时即冻结；后续只更新本人确认终局阶段、房间阶段与逻辑时间。延迟/未知结算期间的request_full也不得读取生存者新位置。

## 定时区域检查点

配置 schema 7、机制 105 和协议 6 支持 `members.checkpoint_schedules`。每组由宿主声明 `id`、`first_delay_ms`、`interval_ms`、`window_ms`、`radius`、`required` 和有序 `centers`。每个中心开启一次，首窗相对 World 激活计时。同一组的各窗互不重叠，最后一窗的截止时刻不得晚于 World 终局。 [配置 Schema](member-piercing-v1.schema.json)

公共 `checkpoint_zones` 发布当前活动窗口，包含计划 ID、从 1 开始的窗口序号、世界坐标、半径及权威起止时刻。配置启用时，空数组表示当前没有窗口。增量携带完整替换数组，省略表示保留前值。

本人 `self.checkpoint_progress` 包含每组的 `collected`、`required`、`total_windows` 和 `last_window_index`。最近窗口序号为 0 表示尚未获得进度。每次有效到达向该成员发布私有 `checkpoint_reached` 事件，携带新进度与逻辑时间。重连直接读取 Rust 快照。退场快照保存本人最终进度并清空活动窗口。

`qualification` 缺省为 `reach`。选择 `avoid` 时，公共窗口携带 `avoid: true`。World 在开启后的第一个有效步、处理移动后登记仍就绪且暴露在危险范围内的成员，在到期的首个有效步重验就绪与站位。最后一窗需在终局前留出一个有效结算步。边界计入危险范围，中心到成员之间的移动阻挡可提供遮蔽。只有已登记、到期时离开危险范围或进入遮蔽的成员获得进度和私有 `checkpoint_avoided`。进入保护会结束本次躲避资格，本窗口内恢复就绪不会重新登记。躲避结果可用于检查点资格或反制，危险区自身的伤害由宿主通过既有机制另行配置。

宿主可用 `collected >= required` 读取计数是否达标。检查点进度随同一 World 保留，阶段胜负及奖励由模式集成裁决。 [协议 Schema](publications-v6.schema.json) · [验收标准](../batches/SDK-5-88-checkpoint-zones.md)

### 检查点反制

检查点计划可附加 `counter`。`target_ids` 按 `centers` 的顺序分别绑定同局原生 NPC ID。宿主提供 `range`（0.1～1000）和 `base_damage`（1～1000000）。`activation` 支持达标当步结算的 `on_reach`、手动提交的 `manual` 和随首次直接接触结算的 `first_contact`。结算检查目标代次、存活、距离、遮挡和免疫。

`manual` 和 `first_contact` 配置 `window_ms`，至少一个 World 步，最长 60000ms。本人 `checkpoint_progress[].counter` 附带 `window_index`、`target {id,generation}`、`opens_at_ms` 和 `expires_at_ms`。重连保留原时点，退场清理。手动模式接受 `counter_checkpoint {schedule_id,window_index,target}`，失败保留仍合法的窗口，成功立即消耗。

`first_contact` 的私有窗口携带 `on_contact: true`。客户端继续提交普通攻击或敌对单位、弹道技能。World 只接受窗口开启后受理、由本人命中绑定目标代次的直接接触。排队、追击和蓄力沿用最初的受理顺序。旧弹道、手动反制请求、召唤体、持续效果和派生接触无法使用该窗口。合格接触消耗一次资格，先结算普通命中，再尝试追加反制伤害。同一接触遇到多个合格窗口时按配置顺序选择一个。

可选的 `window_extension` 和 `damage_bonus` 使用 `{id,by_level}` 引用已有局内加持。等级数组长度须一致，数值单调。延长量以毫秒计，在开启时冻结，窗口总长至多 60000ms 且不跨终局。增伤以基点计（0～10000），按结算时持有等级读取。专属伤害先按配置计算原始值，再走物理防御、护盾和死亡归属。成功向本人发送 `checkpoint_counter_resolved`，含原始伤害、吸收量和扣血量。确认反制后免除随机闪避与暴击，普通命中、吸血和战斗触发器也保持隔离。 [配置 Schema](member-piercing-v1.schema.json) · [协议 Schema](publications-v6.schema.json)

## 顺序模式

配置 schema 7、机制 105、协议 6 可启用顶层 `match_mode`。宿主声明 `stages[]`、`results`，也可声明 `respawn` 和 `disconnect_forfeit_ms`。最多八个阶段，每阶段使用唯一 `id`、`duration_ms` 和 `objective`。所有时长对齐 World 步长，阶段预算之和不超过 `terminal_ms`。模式结算替代撤离读条，未启用模式的对局沿用撤离与死亡结算。 [配置 Schema](member-piercing-v1.schema.json)

| 目标 | 规则 |
|---|---|
| `wait` | 可放在首段。此段暂停玩法推进，到期进入下一阶段。 |
| `checkpoint` | 引用 `schedule_id` 和 `members_required`。本人在本阶段积累到对应检查点计划的门槛后，提交 `complete_mode_checkpoint {stage_id}`。每席确认一次，确认人数足够即进入下一段。 |
| `clear_targets` | 用 `target_ids` 指定原生 NPC。目标在对应阶段开启前暂停攻击和受击。指定目标全部被消灭时推进，超时仍有存活目标则失败。 |
| `contest` | 只能放在最后一段并要求恰好两席。两席和各自召唤在此阶段敌对，普攻及敌对单位技能可绑定对手当前代次。只剩一席可继续作战时该席胜利，同一步双淘汰判平。到期前仍有两席时，以到期前已结算的生命值及到期时最大生命值交叉相乘，精确判胜负或平局。 |

到期边界优先于下一次战斗步。检查点和清除目标在截止前完成才能推进，最后一段完成后仍在场的成员结算为胜。主动退场、预算用尽的淘汰及断线超时结算为负。配置 `disconnect_forfeit_ms` 可在连接宽限到期前判负，重连并完成就绪确认才能取消判负。断线判负与阶段到期同刻时先处理判负。已判负的席位保留既有终局视图和结算责任，观礼使用独立接入范围。

`respawn` 可配置每席最多八次复活、`delay_ms`、生命恢复比例 `hp_bps` 和灵力恢复比例 `spirit_bps`。倒地清理本席战斗动作、状态、临时加持与已释放的本席效果，保留入场身份、库存与击杀记录。复活复用可信席位的配置出生点，刷新战斗实体代次并恢复生命和灵力，归零动作冷却。出生点遇到活动阻挡时等待下一个有效步。传输输入仍使用该局准入席位的 `entity_generation`，战斗目标取快照中的最新 `EntityRef.generation`。

协议 6 在 `self.mode` 发布本人阶段、逻辑起止时刻、检查点确认、复活预算和终局结果。重新连接后若尚未就绪，`forfeit_at_ms` 给出该席仍在生效的提前判负时刻。阶段开启与复活事件走公共流，终局结果走本人事件。各席的 `win`、`loss` 和 `draw` 绑定唯一结果 ID，平局必须使用 `preserve`。资产事务沿用 `result_settlement`，可使用 `settlement_rewards` 和 `settlement_facts`。启用奖励通道时同时提供原有撤离、死亡和三种模式结果的映射。 [协议 Schema](publications-v6.schema.json)

Rust 调用方对 `Outcome` 做穷举匹配时需处理 `Win`、`Loss` 和 `Draw`。协议 6 消费方在启用模式前更新对应的发布解码器。

### 限时存活与分波清敌

SDK 0.158.0 新增两类阶段目标，沿用配置 schema 7、机制 105、协议 6。Rust 客户端穷举匹配 `ModeObjective` 时加入 `Survive` 和 `Waves`。

| `objective.kind` | 规则 |
|---|---|
| `survive` | 配置 `members_required`，范围为 1 至必需参战席数。截止时先统计就绪且可参战的存活席位，满足人数后由达标席位继续阶段，其余未终局席位结算为负。人数不足时阶段失败。截止时待复活或仍处于连接保护的席位不计入。 |
| `waves` | 配置 1～8 个有序 `waves`。每波包含 `delay_ms` 和 1～32 个唯一的原生 `target_ids`，同一目标不得出现在其他波或清除阶段。延迟为模拟步长的整数倍，可为零。首波从阶段开启计时，其余从前波全部清除计时。阶段截止对全部波次生效，同刻到期优先于开波和命中。 |

未开启的目标在成员及旁观视图中省略，也不参与战斗或占位。若该目标守护宝箱或加持，领取继续受守护条件限制。波次到点时，如果某个配置出生点被地形阻挡，整波等待下一次检查，截止前仍无法出现即按阶段失败。成员 `self.mode.wave` 与旁观 `stage.wave` 包含 `index`、`total`、`opens_at_ms` 和 `opened`。序号从 1 开始，`opens_at_ms` 为最早开启时刻，待开启时间最多显示到阶段截止。

成员终局时冻结该席的阶段视图。其他成员继续推进和重连时，已终局席的阶段、波次与结果保持冻结。公开旁观阶段继续反映当前 World。 [协议 Schema](publications-v6.schema.json)

### 击杀进度与固定截止

SDK 0.161.0 可使用 `objective.kind: kill_quota`，按原生目标 ID 声明正整数 `target_weights` 和可达的 `required`。当前阶段的有效击杀累加到门槛后推进。可声明 `advance_on_timeout: true`，未达标到期也进入下一阶段。需要存在下一阶段，到期步优先推进阶段。

阶段可声明 `absolute_deadline: true`。其截止等于 World 激活时间加上从首阶段到本阶段的配置时长之和。提前推进时，后续阶段可提前开始，固定截止保持原值。默认以本阶段实际开启时间加 `duration_ms` 计时。

成员 `self.mode.progress` 和旁观 `stage.progress` 在击杀进度阶段发布 `current`、`required`。Rust 客户端对 `ModeObjective` 穷举匹配时需加入 `KillQuota`。

## 共享击杀计数与强度档位

SDK 0.161.0 可在配置 schema 7、机制 105、协议 6 启用顶层 `encounter_progress`。宿主提供初始计数 `initial`、按原生目标 ID 的 `kill_weights` 和受档位影响的 `affected_templates`。成员及所属召唤实际击倒原生目标时，计数按权重递减，最低为零。

- `tiers` 按 `remaining_at_most` 严格升序配置，取首个覆盖当前计数的档位。没有命中时沿用原属性和掉落。每个 `modifier` 用 `stats_bps` 修饰基础生命、攻击和防御。可选的 `loot_pools` 按模板引用既有 `world_loot.pools`，沿用模板的掉落优先和落点策略。
- `surge` 可声明 `duration_ms` 和同结构的 `modifier`。首次归零覆盖常规档，后续击杀保留原截止。到期回到零值常规档。触发击杀使用击杀前的掉落池。换档按整数比例保留在场目标生命，后续登场使用当前档位。
- 成员和旁观通过 `encounter_progress` 获取 `initial`、`remaining`、`stats_bps`、`surge_started`、`surge_ends_at_ms`。正在生效的强化给出截止，其他时候截止为 `null`。这些字段是当前 World 的公开状态。 [协议 Schema](publications-v6.schema.json)

## 撤离局成员对抗

SDK 0.157.0 起，配置 schema 7、机制 105、协议 6 的普通撤离局可设置 `members.member_conflict: true`。要求 2～8 个参战席和 `combat_rules`，与 `match_mode` 互斥。省略时成员沿用既有友方规则。启用后不同成员及其召唤互为敌对，同一成员和自己的召唤继续友好。宿主参战 ID 与配置原生 NPC ID 必须互异。

客户端仍通过既有 `cast_unit`、`attack_unit` 和方向型动作提交意图。启用时成员全量快照包含固定的 `member_conflict: true`，增量继承此值，解码器据此识别敌对成员目标。单目标和通用弹道沿既有敌我与接触链结算。无目标普攻继续自动选怪，定义为 NPC 专用的目标选择仍使用原范围。成员受击中断撤离沿用既有规则，被击倒走死亡及配置的遗物结算。成员击倒可触发战斗内事件，成长与掉落的原生 NPC 统计沿原来源过滤。

其他席位可继续撤离。遗物详情只回给打开或领取的成员，旁观者可见公开伤害与成员阶段。公开投影继续使用原 Schema。 [旁观发布](#旁观发布)

## 撤离时限与读条广播

SDK 0.162.0 起，配置 schema 7、机制 105、协议 6 可在撤离局声明 `extraction_lifecycle`。其中 `pressure` 和 `broadcast_channels: true` 可独立启用。

- `terminal_ms` 是从 World 激活起算的硬截止时长。配置 `pressure.grace_ms` 后，宽限从硬截止前该时长开始。`warnings` 可声明至多八个 `{id, remaining_ms}`，剩余时长严格递减，且位于硬截止总时长和宽限时长之间。以距硬截止的剩余时间取最近已跨越的阈值。
- 宽限开始满 `damage_interval_ms` 后，首个到期的模拟步按 `damage_max_hp_bps` 对在场成员结算。每个模拟步最多一次。按当时最大生命取整并至少为一，复用护盾和免疫。实际扣血打断读条。硬截止优先于伤害周期和读条完成，仍在场的成员和原生目标归零，成员走既有死亡结算。
- 成员和旁观的 `extraction_pressure` 提供 `grace_at_ms`、`ends_at_ms`、`phase`，探索期可能附带 `warning {id, index}`。部署期间省略。成员退出视图冻结其离场时的阶段。
- 启用广播后，在场成员和授权旁观者通过 `members[].extraction_channel` 获取读条的起始位置和起止时刻。`extraction_channel_changed` 公共事件报告 `started`、`interrupted` 或 `completed`。本人仍可使用 `self.extraction_finishes_ms`。终局成员的快照省略其他成员的在途读条。普通局保持本人可见。 [协议 Schema](publications-v6.schema.json)

## 每席训练目标

SDK 0.159.0 起，配置 schema 7、机制 105、协议 6 可用 `members.training_targets` 声明每席目标组。数组按参战 ID 的字节序对应席位。每席 `presets` 包含 1～8 个自定义组，每组引用 1～16 个本局已有原生 NPC ID。省略 `initial_preset` 时该席开局没有训练目标。每个原生目标必须且只能分给一席，同席的多个组可复用同一目标。

训练目标采用现有模板的属性、受击和状态规则，默认静止。不同席位和其训练目标、召唤之间的战斗与支援互相隔离。已选目标在成员快照的 `monsters[].training_owner_id` 标注归属。未选目标省略。

| 本人输入 | 行为 |
|---|---|
| `set_owned_targets {preset_id}` | 核对本人声明的组和目标落点后，清理本人运行时并重建所选目标。 |
| `clear_owned_targets {}` | 清理本人运行时和所选目标。 |

两种操作均复用参战身份、连接代次、请求 ID、输入顺序和就绪门禁。取消本席进行中的动作、弹道、区域、召唤和状态。目标换代后旧代不能命中。保留本人的生命、灵力、技能冷却和参战身份，其他席位的目标与在途状态继续推进。同一运行实例重连后沿用当前训练组。

训练配置要求零费用、零战备及零成长，并关闭成员对抗、顺序模式、环境资源、局内加持和会触发业务奖励的选项。训练目标击倒不会生成普通掉落或成长收益。切换和清除不产生业务资产写入。退场沿用既有零资产结算凭据。 [协议 Schema](publications-v6.schema.json)

### 目标行为、复活与个人统计

SDK 0.160.0 起，每席可选 `behaviors`，按本席目标 ID 配置。省略的目标沿用原固定靶规则。

| 行为 | 配置与结果 |
|---|---|
| `dummy` | 可用 `motion {end, speed}` 从出生点直线往返，速度单位为世界坐标每秒。运动使用同一套足迹和地形碰撞。可启用 `reset_on_defeat`，在下一次落点合法的步重建目标并推进代次。 |
| `combat` | 复用模板的 NPC 战斗逻辑，只选择所属席位和该席召唤。可由现有的选组操作重建。 |

`members.training_runtime` 可独立开启下列能力，需要同时声明 `training_targets`。

- `respawn {delay_ms, points}` 按席提供复活落点。倒地后清理本席旧生命运行时，在首次到期且合法的步恢复满生命和灵力。保留技能冷却、个人统计和连接状态。退场取消倒计时。权威全量和增量在本人 `self.training.respawn_at_ms` 提供截止时间，平时为 `null`。
- `metrics {window_ms, publish_interval_ms}` 在本人 `self.training.metrics` 发布实际扣血总量、命中、暴击、最高单次和来源伤害。来源分普攻、技能、周期效果、召唤和其他，召唤优先按归属分类。滚动每秒伤害使用配置窗口，平均每秒伤害使用首末次命中间隔，最小分母为发布间隔。伤害率向下取整。有效扣血滑出窗口后，按发布间隔归零。
- 本人可发送 `reset_training_metrics {}` 清空统计。统计和复活排程随同一运行实例重连恢复。旁观投影只含既有公开战斗事实。

## 旁观发布

配置 schema 7、机制 105、协议 6 可通过 `members.spectators` 开启旁观。宿主提供 `capacity`（1～8）和 `terminal_retention_ms`（至少两倍发送回执超时，至多 3600000ms）。旁观席位从 1 编号，独立于参战名单和资产账本。可信宿主先认证再调用 Rust `Client::spectate` 或 JSONL `spectate`。 [旁观配置和协议 Schema](publications-v6.schema.json)

| `action.kind` | 契约 |
|---|---|
| `attach` | 提供 `authenticated_principal`。首次连接代为 1，换代或重新分配时递增 1。已占用席位只允许同一身份换代，新身份先由宿主解绑。相同连接与身份的重复请求幂等。 |
| `detach` | 释放该席订阅。保留最近连接代以拒绝旧连接重入。 |
| `request_full` | 传入当前 `stream_epoch`，清空本席基线并重发全量。 |
| `transport` | 传入 `publication_id` 和 `outcome`。只有当前连接和流内已 drain 的帧能改变发送状态，`sent` 推进基线，失败或取消重开本席流。 |

每条请求都带 `key`、`slot` 和 `connection`。`spectator_publication_v6` 只发到对应席位和连接，内含独立 `stream_epoch`、`revision` 与 `payload`。成员输出和旁观输出使用各自的接收路径。旁观解码器使用 `SpectatorReplicaV6`，宿主在新连接上新建解码器。差量集合按同一套 `upsert`、`removed`、`order` 规则应用。

Rust 对 `OutputBody` 做穷举匹配的宿主需要处理 `SpectatorPublicationV6` 分支。

公开投影包含成员与怪物位置、血量，已释放弹道和公开区域的位置，当前模式阶段以及对抗结果。地面对象只投影无归属公开对象的位置和生命周期。公共战斗事实保留最近 32 条伤害、成员阶段变化和复活，使用独立序号。玩家库存、装备、技能准备、状态细节、私人来源及结算金额留在原有权限范围。

终局保留期从 World 关闭开始，快照当时冻结。保留期内可首次旁观或换代，逾期释放所有旁观缓存。判负成员由宿主另行授权旁观，参战身份仍按原终局规则处理。Node `TwoMemberHost` 通过 `onSpectatorPublication` 分路转发，旁观回调独立调度。旁观流留在传输层，成员和责任输出沿用 SQLite 存储。

## Boss 阶段投影

SDK 0.141.0 的 `monsters[].boss_phase` 随怪物记录一并更新。阶段从 1 开始，`epoch` 对应怪物实体代次。同代次只允许保持同一阶段或前进。归位更新实体代次，投影重新从第 1 阶段开始。重连读取当前权威投影。 [协议 Schema](publications-v6.schema.json) · [Boss 阶段](npc-combat-ai-v1.md#boss-阶段)

SDK 0.142.0 的 `containers` 发布该接收者可见的容器集合。增量出现该字段时完整替换集合，空数组表示当前无可见容器。私有容器掉落沿接收者过滤后的 `resources` 发布。 [协议 Schema](publications-v6.schema.json) · [资源与容器](world-loot-v1.md)

## 实例物品与死亡遗物

SDK 0.144.0 支持由宿主启用的实例物品和死亡遗物。地面实例的 `instance` 与本人 `self.instances` 保持同一实例 ID 和品质。全量或增量的 `relics` 只含公开摘要。遗物内容和领取结果按对应成员发送私有事件。 [库存与遗物契约](world-loot-v1.md)

### 局内装备竞夺与追猎

SDK 0.163.0 起，配置 schema 7、机制 105、协议 6 的多人撤离局可启用 `contested_equipment`。宿主声明 `interaction_range`、`pursuit_range`、`retire_delay_ms` 和 1～8 个 `nodes`。节点包含 `id`、`position`、`basic_profile`、`secured_item_id` 及 `pursuit_ids`。分别引用 `combat_rules.basic_profiles`、专用的 `world_loot.instances` 和最多四个互不共享的 `pve.extra_spawns`。追猎出生点不占共享目标或守护资格。

- `claim_world_equipment` 只接收 `node_id` 与 `expected_revision`。范围、遮挡、就绪、库存预留和在途普攻由 World 校验。首次领取与掉落重领走同一动作。每个节点首次开启一次，持有者离场后才允许重领掉落。
- 成员和旁观者都可看到 `contested_equipment` 的节点 ID、交互坐标、修订、状态和持有者实体。状态为 `available`、`held`、`dropped`、`secured` 或 `lost`。持有期间通过 `holder` 关联实时位置。持有者的 `self.world_weapon_profile` 给出最近领取的生效普攻档案，原 `self.equipment` 和 `self.basic_profile` 继续报告可信装备。换档沿用普攻切换规则，已有普攻冷却继续推进。
- 追猎目标为处于配置半径内且可战斗的持有者。其他席可攻击追猎者。追猎者不发放击杀积分或掉落。持有者退场时清空追猎者的施法、在途效果和目标，按消散时长移除。若期间换主，则以新实体代次立即接续。世界硬截止清除剩余掉落和追猎者。
- 每个持有实例占用一个局内预留格。死亡或退出时释放该格并在权威位置掉落。撤离完成将实例放进原结算事务，公开状态进入 `secured`。持久入账以事务回执为准。连接重入保留同一 World，进程恢复只处理已冻结的资产事务。

### 区域停留与共享交互

SDK 0.164.0 起，配置 schema 7、机制 105、协议 6 的多人撤离局可启用 `zone_exposure`。宿主提供二维投影向量 `axis`、1～8 个 `bands`、`max_stacks` 和 `reduction_per_stack_bps`。各区带有 `id` 和 `interval_ms`，除最后一带外还要提供严格递增的 `below`。World 将位置与 `axis` 做点积，取首个投影值严格小于 `below` 的区带，或落入最后一带。等距地图可由宿主用其瓦片行方向转换投影和阈值。

- 每个模拟步按步开始的位置将经过时长除以区带间隔，累积到一层后扣去相应进度。每层进度精度为十亿分之一，同带保留除法余数，换带保留进度。满层时丢弃余量，连接保护期间继续计时，硬截止先于同刻累积。
- 可选的 `interaction` 包含 `range`、`cooldown_ms`、`remove_stacks`、`boost_bps`、`boost_duration_ms` 和 1～16 个 `nodes`，节点只声明 `id` 和 `position`。客户端提交 `use_exposure_node` 的 `node_id`、`expected_revision` 和 `choice`，由 World 核对距离、遮挡、修订与共享冷却。 `reduce` 移除现有层数，零层时拒绝。 `boost` 刷新增益期限。同一节点的两种动作共用冷却。
- 攻防按现有属性结果乘以 `(10000 - 层数 × 每层削弱)` 和 `(10000 + 当前增益)`，再除以 `100000000` 并向下取整。固定威力与独立元素抗性按原契约取值。
- 成员仅在 `self.zone_exposure` 收到本人区带、层数、上限、可选增益截止和可达节点的修订及就绪时刻。旁观视图省略个人状态。成功重连恢复同一 World 的累积和节点状态，终局回收个人状态，资产事务仍走原结算。
