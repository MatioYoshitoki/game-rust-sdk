# CONFIG-1 配置格式、参数目录与使用方式

机制105的[通用战斗配置](combat-rules-v1.md)在schema7 / 协议6下统一初始化数值关系、恢复公式、复合状态与有界触发；旧版本定义与证据保留。

SDK-5-1新增机制19：[定时DEF状态](statuses-v1.md)，仅schema4/协议3的PVE支持。以下机制1～18历史参数及原解释保持。

SDK-3-2 新增 schema 4，必须声明 result_contract 与 asset_contract，使用协议 3；两种已注册结算策略与业务结果 ID 独立绑定，见 [结果契约](results-v3.md)。

SDK-3-1 新增 schema 3：沿用以下机制/数值配置，额外声明成本/收益资产 ID，使用协议 2；schema 1/2 继续协议 1。绑定格式、余额事实与事务校验见 [资产契约](assets-v2.md)。

2026-11-04 更新。适用配置 `schema_version=2`、`mechanism_version=1` 至 `18`，清单格式为 1。类型定义为 `src/config.rs`、`src/config/parameters.rs`、`src/config/population.rs`，约束为 `src/config/validate.rs`、`src/config/catalog.rs`。本目录覆盖 B1 和 B3-1 至 B3-19 的机制。

## 机制、配置与状态

Rust 提供碰撞/导航、动作提交、弹道命中、伤害/治疗、AI、概率掉落、拾取/堆叠、即时容器/重生、撤离/死亡和外部责任状态机。以下字段只初始化这些机制。当前HP/MP、CD截止时间、仇恨、背包、资源、箱代和结算证明都在 Rust 实例中，不能通过配置或浏览器回写。

当前注册机制：玩家普攻/主技能与怪物弹道为 `projectile`，怪物预算分摊自疗为 `budget_heal_channel`，普通容器为 `immediate_ground_loot`。基础PVE字段定义一个兼容模板；版本3另允许additional_templates登记多个 `melee_projectile_heal` 模板，静态实例或本代守护者引用模板。怪物仍有弹道、自疗两个能力角色，玩家在机制1～25有一个主技能，机制26可增加一个已有弹道附加技能；同局不同模板使用相同机制和独立实例状态。可改模板/技能ID和全部已声明参数，不提供任意技能图或任意数量技能槽。版本2另注册分层封印容器，版本3新增代守护（见下文）；地形DOT、高差和多人竞争需新增机制。

新配置字段显式必填，只有 `pve`、箱点位、宝物规则、奖池chance等被类型标注可选。未知字段/机制、缺字段、错误类型、非有限浮点、未登记物品、非法区间/预算均启动失败；不静默用基线数值补齐。ID是非空、无控制字符字符串；内容ID通常最多64 UTF-8字节，配置/清单别名最多128字节，箱实例ID最多64字节且不重复。

所有 `_ms` 为整数逻辑毫秒；I/O、drain、Client调用期限为真实经过毫秒，验证加速不缩短这些期限。距离为等距投影前的二维世界单位，速度为世界单位/秒。比例为有限浮点0～1；`*_bps` 为整数万分比（10000=100%），怪物crit/dodge为整数百分数。时间/动作在离散步上判定，非步长整数倍的截止时间在之后第一个处理步生效；引导每步最多处理一份到期治疗，调参时应考虑步长。

## 参数目录

来源分三类：S为原资料包规则/模板，F为已验收切片的显式基线，R为服务运行基线。`config/parameters.v2.json` 是导出器的F/R输入；技能/怪物/物品等S数据由 `scripts/export_b*.py` 提取，`scripts/config_schema.py` 规范化。生成的每个配置包含完整数据，运行时不读取导出默认文件。持久库存/角色事实是外部事实，不能被改配置重置；本地受控存储只用 `params.initial_player` 初始化测试属性。

| 字段 | 类型、单位、合法范围/关系 | 来源；消费者 |
|---|---|---|
| `id,source_commit,purpose,schema_version,mechanism_version` | 字符串/版本整数；schema2，机制1/2/3/4/5/6/7/8/9/10/11/12 | S/F；Catalog身份、审计、能力选择 |
| `step_ms` | u64，1～100ms | R；Runtime、World逻辑步 |
| `input_capacity,output_capacity,history_capacity` | usize；输入/历史16～1024、输出32～1024 | R；Runtime启动mailbox、Engine输出/去重/发布历史 |
| `geometry.min/max/walls`、`spawn,extraction_point` | 有限二维点/矩形；最小边界严格小于最大边界，坐标绝对值≤32768；墙≤128，位于边界内；出生/撤离完整足迹合法 | F/S；碰撞、地图校验、H5绘图 |
| `target.id/position` | B1目标身份/点位；足迹合法 | F；B1目标、H5 |
| `target.hp/atk/armor` | u32；hp 1～1000000，其余0～1000000 | F；B1伤害/存活 |
| `target.speed/radius/aggro_range/attack_range/interval_ms/drop_quantity` | 速度0～10000；半径0.1～128；范围0～10000；攻击间隔1～3600000ms；掉落0～10000 | F；B1移动/感知/普攻/奖励 |
| `basic.mechanism/windup_ms/interval_ms/range/projectile_speed` | projectile；前摇0～3600000ms；间隔1～3600000ms；射程/弹速0.1～10000 | S；两种World玩家普攻 |
| `skill.mechanism/id/cost/windup_ms/recovery_ms/cooldown_ms` | projectile；成本0～1000000；三种时间0～3600000ms | S；动作预留/提交、技能身份、H5 |
| `skill.damage_bps/range/speed/radius` | 伤害0～100000bps；射程/弹速0.1～10000；弹道半径0～128 | S；玩家弹道和伤害 |
| `item_id,carried_quantity,stack_limit,inventory_capacity` | 主物品ID；携带u32；每格1～10000、背包1～128格；携带≤格数×堆叠 | S/F；准入扣费、拾取、结算/恢复 |
| `extraction_unlock_ms/extraction_channel_ms/terminal_ms` | 前两项0～3600000；终局1～3600000；开放+读条≤终局 | S/F；World撤离/绝对终局、H5 |
| `kill_score/extraction_score/cultivation_coefficient/death_multiplier` | 分数u32；成长系数0～1000、死亡倍率0～10；最大收益计算不得溢出u32 | S；Config共享成长公式、恢复 |
| `defender_realm_atk/defense_cap` | 1～1000000、减伤上限0～1 | S；防御公式 |
| `params.player_radius/pickup_range/extraction_range` | 半径0.1～128；范围0.1～1000 | F；足迹、拾取/撤离、H5范围 |
| `params.movement.base/bonus/reference/log_base` | 速度基值/增量0～10000；身法参考0.001～1000000；对数底1.001～1000000 | S/F；玩家/怪物同一移动曲线 |
| `params.regen.interval_ms/near_radius/near_ratio/far_ratio` | 1～60000ms；近敌半径0～10000；比例0～1 | S/F；玩家/怪物自然回复 |
| `params.basic_spirit_on_hit` | u32，0～1000000 | S/F；普攻命中回蓝（B1基线0、PVE基线2） |
| `params.orb_hp_ratio/orb_spirit_ratio` | 比例0～1 | S；击杀生成光球，按角色上限取整/钳制 |
| `params.defense_scale/minimum_damage/critical_multiplier/metal_multiplier` | 防御系数0.001～100；最小伤害u32 0～1000000；暴击/金属性倍率0～10 | S/F；伤害机制（暴击/金属性为PVE能力） |
| `params.resource_ttl_ms/drop_min_radius/drop_max_radius` | PVE资源寿命1～3600000ms；掉落半径u32，0≤min≤max≤75 | S/F；怪物/容器掉落、清扫；B1保留其原资源生命周期 |
| `params.basic_projectile_radius/player_projectile_ttl_ms` | 0～128；1～3600000ms | F；普攻弹道体积、玩家弹道寿命 |
| `params.entry_years` | u32，1～10000。机制 105、协议 6 支持 0 | F；原子入场与原金额补偿 |
| `params.death.slot_ratio/quantity_ratio/minimum_quantity` | 两比例0～1，最小数量u32 0～10000 | S/F；随机选非空格、格内损失、通用恢复校验 |
| `params.initial_player.hp_points/spirit_points/atk_points/def_points/spd_points` | u32，HP 1～1000000，其余0～1000000 | F；本地存储初始化可信属性；World按读取事实建角色 |
| `pve.map_id/width/height/cell_size/cells_rle/palette` | 宽高4～512格、格边4～128；RLE索引合法、计数正且总和=宽×高；调色板1～64项 | S；Catalog预处理、Grid、H5可见格缓存 |
| `pve.palette[]` | 9个显式字段；地形名、通行/弹道/视线bool；速度0～10，可走必须>0；elevation 0～10000且全表相同；DOT/爬升/下落均0 | S；地形能力与导航校验 |
| `pve.template_id/monster_positions/stats` | 0～32个可达点；七属性hp/spirit/atk/def/spd/crit/dodge，HP>0、属性≤1000000、crit/dodge≤100；拒绝未实现hit等字段 | S；复用AI实例、隔离状态、权威快照 |
| `pve.leash/help_range/memory_range` | 距离0.1～10000 | S；牵引/呼救/记忆 |
| `pve.combat.radius/attack_interval_ms/aggro_range/melee_range/return_distance` | 半径0.1～128；间隔1～3600000ms；各范围0.1～10000 | S/F；怪物近战、感知、移动/归位 |
| `pve.combat.cast_chance/lifesteal_ratio/mark_max/mark_duration_ms` | 比例0～1；印记上限u32 0～100；持续0～3600000ms | S/F；AI施法决策、实际伤害吸血、印记生命周期 |
| `pve.combat.projectile.*` | mechanism=projectile；独立ID；cost≤1000000；前摇/CD/后摇0～3600000ms，ttl_ms 1～3600000；range/speed 0.1～10000、radius 0～128、damage_bps≤100000 | S/F；怪物弹道能力，数值不依赖模板名 |
| `pve.combat.heal.*` | mechanism=budget_heal_channel；ID与弹道ID不同；cost/三种动作时间同上；hp_threshold 0～1；heal_bps 0～10000、interval_ms 1～3600000、ticks 1～64 | S；低血量自疗、总治疗预算分摊/余数 |
| `pve.items[id].name/stackLimit` | 名称非空；堆叠1～10000；最多64种；主物品必须登记且堆叠与root一致 | S；物品引用、背包、结算/恢复、H5名称 |
| `pve.drop_table`、`pve.treasure_rules.loot` | guaranteed/random各为行数组，总行数≤64；itemId必须登记；0≤quantityMin≤quantityMax≤10000；random必有chance 0～1，guaranteed不得带chance | S；同一独立概率/数量抽取；0不掉、1必掉、数量0不生成资源 |
| `pve.chest_spawns[].id/position` | 0～16个；唯一ID、完整足迹与可达路径合法；有点位必须有treasure_rules | S/F；容器实例/代隔离 |
| `pve.treasure_rules.template.id/name/mechanism/respawn_delay_ms` | 即时落地机制、非空名称；重生1000～3600000ms | S；开箱/重生，不写背包 |
| `pve.treasure_rules.interaction_range/ground_capacity` | 距离0.1～1000；地面容量1～128 | S/F；资格、原子落地/容量拒绝 |

移动公式：身法为0时不移动；否则 `base + bonus × clamp(log(points/reference)/log(log_base),0,1)`。死亡机制对非空堆叠格随机无放回选择 `ceil(格数×slot_ratio)` 格；每个被选格损失 `min(该格数量,max(floor(数量×quantity_ratio),minimum_quantity))`。改变系数不改变公式和取整机制。恢复按各物品完整堆叠和尾格重建可行选格数，跨物品合并验证总选格数，不能用总量除二或每种物品一格代替。

## 服务参数和工程上限

| `params.runtime`字段 | 范围/单位 | 消费者 |
|---|---|---|
| `io_timeout_ms/drain_timeout_ms` | 各100～60000真实ms | Engine外部责任期限/World有限保留 |
| `reconnect_grace_ms` | 100～3600000逻辑ms | 初次就绪、断线/换代就绪窗口 |
| `publish_batched_ms` | 1～60000逻辑ms | Engine非实时合并发布 |
| `nav_budget` | 1～262144节点 | Grid有界A* |
| `nav_repath_ms/ai_decision_ms/resource_sweep_ms` | 各1～60000逻辑ms | 重寻路、施法决策、资源清扫 |
| `call_timeout_ms` | 100～10000真实ms | Client 同步调用等待，构造 Runtime 时从清单默认场景固定 |
| `max_attempts` | 1～3 | 有限自动写尝试；3为现有检查点/责任目录工程上限 |
| `max_catchup_steps/max_lag_steps` | 各1～10、1～100步 | Runtime追赶与积压判断 |

清单 `default_scenario` 必须引用 `active` 中的别名。旧清单省略该字段时使用 `b1`。别名用于查找配置，能力校验依据配置内容与机制版本。`archives` 最多 30 项，`active` 与 `archives` 合计最多 32 项。每个配置文件最多 1 MiB，清单最多 64 KiB。PVE 理论瞬时资源预算最多 128，逐物品最大产量为 10 亿，包含终局时限内的箱重生，实际背包最多 128 格。这些上限约束内存、恢复载荷和搜索工作量。

Runtime 的 mailbox 容量与 Client 调用期限在构造时从清单默认场景固定。`Create` 选择的版本决定 Engine 输出和历史容量、逻辑步长、玩法和 I/O 参数。协议行和检查点有固定编码上限，Node 会话、I/O 与所有者数量受宿主工程策略约束。SQLite 测试初始库存 100 用于受控账本验证。故障注入延迟、UI 动画、颜色、投影和轮询由宿主设置。秒和万分比换算、圆周常量、归一化、采样和合法状态转换属于算法和协议定义。

## 调参、校验与版本恢复

1. 在接入方目录建立自己的配置。可以参考 `config/b3-2.json` 的字段，使用自己的 ID、地图和参数。清单 `active` 引用新文件，`default_scenario` 指定默认别名。仓库 `config/` 中的基线样例由导出器生成。
2. 使用现有二进制检查：`./target/debug/kernel-host --configs /绝对路径/catalog.json --catalog`。输出包括准确原文哈希和Rust规范化配置；大型地图输出较大，可重定向保存。
3. `node adapters/h5/server.mjs --configs /绝对路径/catalog.json --state /绝对路径/lab.sqlite` 重启采用新数据，无需重新编译。`npm run h5` 仍会先检查Cargo构建。`GET /api/catalog`列举启动场景，`/api/config`提供展示数据；已有session始终读取原版。
4. 活跃World不热改。配置原文、configId/hash与冻结责任一起保留；换参数不重算旧金额。重启只恢复Closed资产责任/结果，无终局的中断局按原请求退款，不恢复World。

三个已交付schema1原文保存在 `config/versions`。旧格式由 `src/config/legacy.rs` 和冻结的 `legacy-v1.json` 按历史语义解释，保留旧raw/hash；只接受原已支持语义指纹（含其合法地图变体）。这是旧责任兼容层，不能据它给新数值配表；新数据必须用schema2，schema2没有数值指纹白名单。原本未消费的hit从旧格式规范化结果移除；新格式填写hit会明确拒绝。

完整验收：`python3 scripts/verify_parameters.py`。A/B/格边变化、跨堆叠结算、提交前后强杀、新旧死亡策略和schema1旧责任恢复使用同一个二进制；结果与源码哈希在 `artifacts/parameters/verification.json`。

## B3-3 机制版本2增量

`schema_version`仍为2。`mechanism_version=2`允许 `pve.treasure_rules.template.mechanism=layered_seal_ground_loot`，必须显式提供 `template.seal`：

| 字段 | 类型/单位/约束 | 来源/消费者 |
|---|---|---|
| `layers` | u32，1～32 | 来源玉匣基线2；LayeredProgress、开启资格、H5 |
| `layer_duration_ms` | u64逻辑ms，step_ms～60000；layers×时长≤terminal_ms | 来源基线1200；同一进度时钟、产量恢复边界 |
| `interrupt_retention_ratio` | 有限f64，0～1 | 来源基线0；中断时当前层进度乘比例向下取整，已完成层保留 |

版本1与即时容器保持原解释/哈希；即时机制不得携带seal，分层能力不得使用版本1，也不得遗漏seal。旧配置不会被静默补入封印。层数/时间/比例可在同机制下修改，版本1/2不能携带guardian；未知成本/优先期字段仍严格拒绝。

开始/继续、取消与完成都由Rust状态机处理；解封不发奖、成功开启才开始原重生计时，下一代重置全部进度。一个角色同时最多一个交互；正常每层至少一个模拟步，层数上限约束事件与状态规模。恢复产量上界采用首轮最低破解时长及“重生+再次破解”轮次，仍只恢复Closed资产责任。

B3-3导出器从原包取封印/稀有奖池，使用独立验证模板和原已验证森林双点；没有把带守护/优先领取的完整玉匣偷换为无守护成品。范围见[B3-3](../batches/B3-3-layered-container.md)。

## B3-4 机制版本3增量

schema仍为2。版本3继承即时/分层容器，增加混合模板与代守护。旧配置省略以下可选字段时保留旧语义和raw/hash；旧版本填写新能力字段启动失败。

| 字段 | 类型/单位/约束 | 消费者 |
|---|---|---|
| `pve.additional_templates` | 可选对象，最多16项；key为唯一模板ID，不能覆盖基础template_id | Catalog只读Arc模板缓存 |
| `additional_templates[id].mechanism/name` | `melee_projectile_heal`、非空名称 | 复用现有AI和H5展示 |
| `additional_templates[id].stats/combat/drop_table/leash/help_range/memory_range` | 与基础模板的相应字段同类型、单位、范围；stats仍为七属性 | 每实例独立战斗/碰撞/归位/掉落 |
| `additional_templates[id].kill_credit/drop_orb` | 必填bool；是否计一次普通击杀、是否生成光球 | 原kill_score成长公式、死亡/掉落；不提供独立积分倍率 |
| `pve.extra_spawns[]` | 可选数组；每项必含id/template_id/position；ID唯一，不能与基础实例重名或以guardian-开头；引用已登记模板 | 静态混合实例，完整自身足迹/可达路径检查 |
| `pve.treasure_rules.template.guardian` | 可选严格对象，版本3才允许；有守护时基础模板ID不能以guardian-开头 | 本代计划、生成、门禁 |
| `guardian.template_id` | 必须引用additional_templates中的模板 | 守护者属性/能力/奖励 |
| `guardian.chance` | 有限f64，0～1；0不生成、1必生成 | 每代一次概率决策 |
| `guardian.min_count/max_count` | u32；1≤min≤max≤4，闭区间随机数量 | 每代冻结计划、预留槽位 |
| `guardian.spawn_offsets` | 有序二维偏移数组；max_count～16项，分量有限且绝对值≤1000；任意两点距离≥2×该模板半径 | 相对箱中心的候选点；全组可落地才提交 |

基础模板继续默认计普通击杀、有原掉落表/光球；若需独立奖励策略，登记额外模板并用extra_spawns引用。额外模板的drop_table必须显式提供，空表可关闭物品掉落；自疗hp_threshold=0可关闭该条件触发，其他能力仍需提供合法完整结构。

基础静态数+额外静态数+每箱max_count预留数≤32，不随重生累积槽位；终局内计分击杀上界≤4096。每箱每代只在首次冻结时抽概率/数量；受阻重试不重抽、不额外消耗随机数。候选点在启动时检查地形/路径/足迹，运行时另避让玩家及活怪；失败保持pending且不提交部分守护者。

恢复按模板汇总静态实例和有界守护代数；不计分怪的物品独立纳入上界，计分怪在冻结击杀预算内按各物品最大可能产量约束，再加有界容器奖励。该上界只验证冻结责任的可能性，不重抽击杀或物品；零击杀也可合法有不计分怪/箱奖励。资源预算仍≤128，逐物品总量与死亡/容量/守恒约束继续生效。

`python3 scripts/export_b3_4.py --check` 验证独立导出；`python3 scripts/verify_b3_4.py` 检查混合模板、概率/数量/落点、代隔离、公开战斗、资产强杀与旧版责任。来源资料只给玉匣守护参数，未导出独立守护怪模板；当前verification-guardian是显式配置的验证模板，见[B3-4范围](../batches/B3-4-guarded-container.md)。

## B3-5 机制版本4增量

`pve.encounters`为可选严格对象数组，只有mechanism_version≥4允许。省略/空数组保留原语义与哈希；旧配置不自动加入触发器，字段/嵌套结构和未知输入严格校验。

| 字段 | 类型/单位/约束 | 消费者 |
|---|---|---|
| `encounters[].id/name` | 非空无控制字符字符串，各≤64 UTF-8字节；id在遭遇间唯一；最多8组 | 实例、事件与H5名称 |
| `trigger.center/radius` | 二维有限世界坐标，分量绝对值≤32768；半径有限0.1～1000；中心玩家完整足迹可走/可达 | 权威位置按步采样，距离≤半径触发 |
| `monsters[]` | 1～32项，每项必填id/template_id/position，与extra_spawns同严格结构；引用基础或额外模板 | 预留槽位、整组动态启用 |
| `required_monster_ids` | 非空去重字符串数组，每项必须属于本组monsters | 指定实例实际死亡完成条件，不依赖计分数 |
| `reward_chest_ids` | 非空去重字符串数组；每项必须登记在chest_spawns，每箱最多绑定一组 | 遭遇完成后解除容器资格锁 |

所有怪物实例ID在基础、额外静态和全部遭遇之间唯一，手工ID不能以guardian-开头。各敌人自身足迹/路径和同组点间距启动校验；运行时避让玩家/现存活怪。全部敌人可落地才一起启用，受阻按ai_decision_ms重试，不使用RNG。

基础静态+额外静态+全部遭遇预留+全部守护预留≤32；单次遭遇只生成一次，终局种群/产量上界加入所有遭遇配置实例，资源/物品/成长预算继续使用同一检查。区域触发没有重置/计时参数，也没有重复刷怪；完成不发奖励或自增修为。箱重生仍只看原容器时长，已完成遭遇资格保留。

本批独立导出 `config/b3-5.json` 使用原森林三狼点位和双封印箱，触发中心为第一狼点、96世界单位半径为验证布局参数。所有数值写入配置；Rust无该具名遭遇或96阈值常量。运行 `python3 scripts/export_b3_5.py --check` 和 `python3 scripts/verify_b3_5.py`，范围见[B3-5](../batches/B3-5-encounter-loop.md)。

## B3-6 机制版本5增量

`encounters[].waves`为可选严格对象数组，非空时必须mechanism_version≥5，且不能同时填写非空的旧monsters/required_monster_ids。旧单组字段原解释保留；新波次形式不填旧字段，旧DTO/快照不附加wave。每遭遇仍一次触发，各遭遇独立。

| 字段 | 类型/约束 | 消费者 |
|---|---|---|
| `waves` | 1～8项；显式顺序列表，不支持空波 | 顺序状态机、有界预留 |
| `waves[].id` | 非空、无控制字符，≤64 UTF-8字节，本遭遇内唯一 | 波事件/快照与怪物绑定 |
| `waves[].delay_ms` | u64逻辑ms，0～60000，全部波间隔总和≤terminal_ms | 本波被安排后的生成前间隔，含首波 |
| `waves[].monsters` | 非空、同原MonsterSpawn结构；模板/ID/足迹/路径/同波间距校验 | 整组生成与独立实例 |
| `waves[].required_monster_ids` | 非空去重的本波怪物ID子集，不能引用前后波或别组 | 本波真实死亡完成条件 |

所有波的怪物ID与静态/其他遭遇全局唯一，不得手工使用guardian-前缀；同波不能重叠点位，跨波可复用点位。所有波次一次性预留和原静态/守护共同≤32，不按单波峰值忽略未来实例；资源/成长/物品恢复上界包含所有波次。非计分目标仍可完成，每波积分/物品/光球由原模板标记控制。

间隔只在安排时写入绝对ready_at，不随离开、刷新或重试顺延。逻辑钟在保护中继续，生成/推进暂停；恢复后按原截止时间继续。后波delay=0也最早在当前波完成之后的模拟步生成。最终reward_chest_ids沿用父遭遇，前波和等待状态不能领取，无每波额外奖。

基线导出 `config/b3-6.json`复用三狼点位，分2只/1只两波，delay_ms=0/1500为验证布局数值，全部在配置初始化。运行 `python3 scripts/export_b3_6.py --check` 与 `python3 scripts/verify_b3_6.py`，完整边界见[B3-6](../batches/B3-6-wave-encounter.md)。

## B3-7 机制版本6增量

`pve.additional_templates[id].boss`为可选严格对象，提供阶段能力，要求mechanism_version≥6；缺省时普通模板与旧DTO不变。属性/技能/成本/前后摇/CD/掉落继续使用模板原字段，阶段只覆盖近战间隔和决策施法概率。能力不自动附加Boss类型积分、移速或光球；kill_credit/drop_orb/drop_table仍分别配置。

| 字段 | 类型/约束 | 消费者 |
|---|---|---|
| `boss.phases` | 1～8项，顺序列表 | 每实例独立单向阶段 |
| `phases[].id/name` | 非空、无控制字符、≤64 UTF-8字节，ID在模板内唯一 | 阶段身份/事件/H5 |
| `phases[].hp_threshold_bps` | u32万分比；首项10000，之后严格递减且1～9999 | 实际正伤害后以HP×10000≤maxHP×阈值判定 |
| `phases[].attack_interval_ms` | u64逻辑ms，1～3600000 | 下次近战发生后重新排期使用 |
| `phases[].cast_chance` | 有限f64，0～1 | 下次既有AI施法决策 |

跨多个阈值直接进入最深满足阶段，只发一个变更事件；致死不切阶段。回血不倒退，切阶段不清动作/CD或重算弹道。仅真正到家完成归位满血/初始阶段/epoch+1，并排初始近战间隔，MP/CD/recovery不返还。静态、波次、守护均可引用此模板，阶段不增加槽位/计分/资源上界。

基线 `config/b3-7.json`采用两狼+HP450守将；2阶段阈值10000/5000、间隔2000/900ms、概率0.1/0.45均显式配置。运行 `python3 scripts/export_b3_7.py --check` 和 `python3 scripts/verify_b3_7.py`，详见[B3-7](../batches/B3-7-boss-phases.md)。

## B3-8 机制版本7增量

新能力仅PVE可用，`pve.kill_score`及`pve.additional_templates[id].kill_score`可选，出现需mechanism_version≥7。值为0～1000000整数，拒绝null/负数/小数/其他类型；省略继承root kill_score，显式0不回退。kill_credit=false时有效积分0，显式非零override拒绝；true且0分合法。名称、Boss能力和ID不隐含类型倍率。

新版本累计实际死亡、计分击杀、每模板次数与积分；PveView.kills保留旧计分数语义，scoring提供独立权威状态/两种结果的修为预览。全部未来波次/静态/守护重生纳入加权积分与修为预算，不能仅用首波数量校验。新结算冻结scoring摘要，以模板实际死亡次数校验物品上界，零积分/零系数也不丢真实掉落；旧八raw及原责任正文不附加此字段。

基线普通10/精英30/Boss100均是显式验证分值；140分按原root转换得到撤离127/死亡52。导出 `python3 scripts/export_b3_8.py --check`，专项 `python3 scripts/verify_b3_8.py`，精确字段与验收见[B3-8](../batches/B3-8-template-scoring.md)。

## B3-9 机制版本8增量

`pve.additional_templates[id].area`可选严格对象，mechanism=locked_circle，出现要求mechanism_version≥8，null同省略。id/name、range/radius、windup_ms/cooldown_ms/recovery_ms、cost、damage_bps、cast_chance与min_phase_index均必填，约束见[B3-9字段表](../batches/B3-9-area-telegraph.md)。ID不得与本模板两种既有技能重复；无Boss能力时min_phase_index只能1，有阶段时不得超过阶段数。能力可被静态/波次/守护复用，非Boss专属名称开关。

原AI决策先尝试新能力再回退原行为，开始要求仇恨、距离、视线、灵力、CD、阶段与空闲；锁点、扣费和CD在开始时完成，取消/miss不返还。当前足迹与圆相交且落点到玩家有效果通路才命中；视线使用blocksVision，效果使用blocksProjectiles，完整移动仍用walkable。旧无area模板不额外抽RNG，不改变既有技能成本时点。

验证配置位于原地图附近可视空地，Boss点位[3376,1072]；原点的树林阻视线，不作为忽略视线的理由。radius96/range550/windup1800/cooldown6000/recovery900/cost12/damage20000bps/chance1/min_phase1均显式配置。旧九raw不变，积分/掉落/责任预算沿用；每实例最多一圈，≤32实例，取消身份列表有界。运行 `python3 scripts/export_b3_9.py --check`、`python3 scripts/verify_b3_9.py`。

## B3-10 机制版本9增量

可选`encounters[].requires`声明全部前置遭遇，ID须存在/去重、不可自依赖/成环，支持前向引用。EncounterView附加可省略的blocked_by列表；未满足时不会触发。地图目标存在时允许节点不绑定箱，旧配置保留非空奖励箱约束。

可选`pve.map_objective`为每局一个主目标：id/name、required_encounter_ids（非空去重且依赖闭合）、reward_chest_id、attune_duration_ms（step_ms～60000，≤终局）、cost{item_id,quantity}（注册物品，0～1000000）。主目标奖励箱本局只开一次，其他箱仍按原代/重生语义。旧十raw/hash保持，字段省略不改变旧机制。

公开attune_objective{objective_id,expected_revision}开始权威调谐，cancel_attunement{}取消。完成所有目标、交互距离/通路、修订和材料须满足；无并行动作/撤离/破解。开始不扣费；成功后原子扣材料并修订+1，取消不扣。正伤害/移动/导航/后台/换代/死亡/关闭中断；Stop保留。调谐到期在当步伤害后，完成仍须显式破解/开箱/拾取。视图map_objective给出权威已完成节点、可用/调谐/领取状态、finishes_at_ms、duration_ms、材料和中断原因，H5不自行完成读条。

新增可选冻结map_progress={completed_encounter_ids,attuned,reward_opened,consumed:[{item_id,quantity}]}，只在有地图目标的配置下要求/允许。完成ID严格排序/依赖闭合、击杀摘要支撑目标数量；consumed严格等于原配置成功调谐的成本。逐物品入场+累计拾取=保留+掉落+消耗，ItemSettlement.looted包含已消耗量；消耗后为0的物品仍保留结算行。死亡抽格只基于剩余背包，恢复按原hash原成本核对，不重建World。主目标奖池在未领取证明中不计入可得物品上界。

导出`python3 scripts/export_b3_10.py --check`，验收`python3 scripts/verify_b3_10.py`；配置来源与路线适配边界见[B3-10](../batches/B3-10-forest-objective-chain.md)。

## B3-11 机制版本10增量

`additional_templates[id].support`可选，严格字段与区间见[B3-11](../batches/B3-11-support-heal-interrupt.md)。机制`ally_heal_channel`，同遭遇（含跨波次）或同箱同代为治疗组，无组只允许显式自疗。最低HP比例优先，平手按稳定ID；开始固定目标身份/代、预算与时点，首跳才提交MP/CD/成功次数。后续按固定份额钳上限，不重定向、不复活，取消不退已提交效果。max_casts=0禁用，无旧能力字段时不改变旧AI随机消费。

窗口内新接受的Attack/Cast首次直接命中可打断，含合法0伤害、不含闪避；接受时间由Rust记录并随真实弹道冻结，不能由输入提供时间或打断结果。打断冷却为当前玩家全局逻辑冷却，准备中断不扣费，已提交引导只停止后续跳。归位/目标死亡或换代/保护/终局清理，聚合取消受原32身份上限约束；次数与MP/CD不因归位/重连返还，新实例独立初始化。

MonsterView新增可选support，含技能名、次数、正常/打断CD与cast的目标、身份、准备/提交/下跳/窗口时点及跳数。事件SupportStarted/Pulse/Completed/Interrupted、聚合SupportCancelled；无直接打断/改次数/发治疗的公开命令。资产协议不增加治疗责任；进程重启仍只恢复旧冻结正文，不重建治疗。

版本10玩家Navigate优先按当前活怪足迹绕行；无法绕行时保留原静态接近路径，实际每步碰撞仍有效；目标足迹内目的地保留旧接近语义。使用原导航预算，版本1～9路线保持。H5只提交移动目标/普攻/技能和展示权威连线/读条。

导出`python3 scripts/export_b3_11.py --check`，专项`python3 scripts/verify_b3_11.py`。本路线完成湿地后采用来源削弱版治疗：250距离、1800ms准备、600bps、20MP、12秒CD、4次，打断冷却5秒；并非新增动态侧路技能替换。所有值由配置初始化。

## B3-12 机制版本11增量

`additional_templates[id].heavy`可选，严格字段/区间见[B3-12](../batches/B3-12-heavy-counter.md)，类型为`src/config/heavy.rs`。mechanism=locked_line_counter，range/half_width/windup_ms/travel_ms/cooldown_ms/recovery_ms/cost/damage_bps/cast_chance/min_phase_index/counter_window_ms/counter_damage_bps全部显式初始化。速度由range/travel_ms推导，拒绝额外speed字段；ID与模板其余能力互异。开始固定轨迹/伤害、准备不扣成本，释放时提交一次MP/CD，提交后取消不退。

原始危险资格、真实移动、整次接触与正常完成共同决定单人反击窗口。窗口目标/实例代/动作/时点/追加量冻结，窗口内新接纳的合法直接Attack/Cast接触才消费；追加物理伤害复用正式死亡链，不循环触发。旧能力没有heavy时不额外消耗AI随机流。旧raw/hash、资产恢复正文及Node边界保持。

## B3-13 机制版本12增量

`encounters[id].reset`为可选严格对象，字段为mechanism=whole_encounter_round、reward_policy=first_defeat_per_spawn、pursuit_radius、delay_ms。类型与范围见[B3-13](../batches/B3-13-encounter-reset.md)和`src/config/reset.rs`。真实死亡按终局时限/延迟推导并限4096，重复光球按资源TTL/清理周期计入既有128地面资源预算；收益/物品上界仍按原逻辑出生槽位计算。示例新场景使用5000ms、节点半径+180，资源TTL显式60000ms，原十三配置不变。

追击圆包含边界，实际玩家位置决定脱战，前后台不构成空间离场。计时期间只回防，不单独补满/清冷却；提前回来继续本轮。到期才从首波整组恢复HP/MP/阶段/CD/治疗次数，玩家数值/背包/已得战果与其它完成节点保留。首次死亡按槽位只结算一次物品/积分/成长，重复死亡只推进本轮目标及原恢复光球。已完成遭遇不再复位。

## B3-14 机制版本 13

`additional_templates[id].support.on_encounter_completed` 指定遭遇完成后的治疗版本。可选对象包含 `encounter_id`、`id`、`name` 和 `heal_bps`。触发遭遇须存在，技能 ID 须与模板的其它技能不同，治疗量为 0 至 100000 的整数万分比。

Rust 在支援动作开始时根据当前遭遇状态选择版本，固定技能身份和治疗预算。两套版本共用灵力成本、次数、冷却和打断规则。空闲时 `SupportView` 发布下次可用版本，施法中发布该次动作的版本。[B3-14 范围与验收](../batches/B3-14-wetland-support.md)

## B3-15 机制版本 14

`pve.timed_position` 配置一个限时站位机制。必填 `id`、`name`、`encounter_id`、`source_id`、`interval_ms`、`warning_ms`、`position`、`success_damage_bps`、`failure_damage_bps`。核心来源须属于目标遭遇。站位判定 `near_living_member` 配置 `radius`，读取当前角色位置与存活成员中心距离。

`warning_bonus_ms` 和 `failure_scale_bps` 是可选的节点完成规则，均由 `encounter_id` 和 `value` 组成。最长预警须小于周期。Rust 与森林支援共用节点完成查询，动作开始时冻结参数。成功走玩家物理伤害，失败走核心造成的玩家伤害。活动预警通过 `PveView.timed_position` 和站位事件发布。[B3-15 范围与验收](../batches/B3-15-composable-encounter-mechanics.md)

## B3-16 机制版本 15

`pve.timed_position.position.kind=rotating_sites` 提供站位半径 `radius` 和有序 `sites` 数组。数组包含 1 至 8 项，每项都有 `id`、`name` 和世界坐标 `position: [x, y]`。ID 和坐标在数组内各自唯一。每个圆须完整落在遭遇触发范围内，其中心须容纳角色足迹且能从出生点到达。

每次开始预警时按数组顺序锁定当前点位，末项之后回到首项。到期时角色中心在该圈内即成功，包含圆边界。按距点位中心最近的存活成员选目标，同距取稳定 ID。取消预警后，同一遭遇轮次继续轮转，整组复位后从首项开始。`TimedPositionWarningView.site` 发布当轮点位，`timed_position_resolved.site_id` 标记结算点位。这两个字段仅用于固定点位模式。原 `near_living_member` 判定仍可使用。[B3-16 范围与验收](../batches/B3-16-rotating-position-sites.md)

## B3-17 机制版本 16

固定点位可用 `pve.timed_position.skip_sites_after` 将点位 ID 映射到侧路遭遇 ID。每个引用都须存在，侧路须与终局不同，至少一个点位须始终可用。选择时从上次点位之后按原数组顺序跳过已完成侧路的点位，整组复位从首个可用点位开始。活动读条继续用冻结点位。配置筛选规则时，`TimedPositionView.stabilized_sites` 发布当前稳定的点位数。

固定点位可以用 `success_spirit_restore_bps` 代替 `success_damage_bps`，两者选一。回灵比例为 1 至 10000 万分比，冻结最大灵力乘比例后向下取整，到期按实际缺口回复。`timed_position_resolved.spirit_restored` 在镇守成功时发布实际回复量，满灵力时为 0。失败沿用 `failure_damage_bps`。两项新增配置都要求机制版本至少为 16。[B3-17 范围与验收](../batches/B3-17-channel-guard.md)

## B3-18 机制版本 17

`pve.timed_position.direct_hit` 声明以 `source_id` 为目标的直接命中窗口。此模式要求 `success_damage_bps` 和 `direct_hit.miss_heal_bps`，分别表示追加物理伤害倍率与回冻最大生命比例。回冻比例范围为 1 至 10000 万分比。位置、站位失败伤害、点位筛选、回灵与直接命中条件互斥。`warning_bonus_ms` 仍延长下次窗口，`failure_scale_bps` 改为缩放下次回冻量，两项参数在开始时冻结。

首次有效接触须由 Rust 在窗口开始后接纳且在截止前命中。普通伤害先结算，核心仍存活时追加伤害。`TimedPositionView.contact_target_id` 指向核心，活动窗口发布冻结的 `miss_heal` 和 `hit_confirmed`。事件 `timed_hit_triggered` 发布实际追加伤害，`timed_hit_missed` 发布实际回冻量，均包含 `round` 和 `cycle`。开启、取消与复位沿用限时流程。[B3-18 范围与验收](../batches/B3-18-thaw-window.md)

## B3-19 机制版本 18

怪物模板的 `heavy` 可选 `counter_window_bonus_ms` 和 `counter_damage_scale_bps`。每条规则包含已有遭遇的 `encounter_id` 与 `value`，分别表示追加窗口毫秒数和追加伤害乘数。加时为正数，合计窗口至多 60000ms。伤害乘数为 10000 至 100000 万分比，基础倍率与乘数相乘后的总倍率至多 10 倍。

Rust 在开启反击窗口时读取已完成遭遇，冻结 `CounterView` 的 `ends_at_ms` 与 `raw_damage`。追加伤害按攻击力乘两项倍率后一次向下取整。攻击资格、单次消费与关闭规则沿用定向重击流程。[B3-19 范围与验收](../batches/B3-19-sword-trial.md)


## SDK 状态机制19～25

schema4/协议3的PVE可声明状态定义、有限规则及技能提交/命中绑定；数值由接入方配置初始化。机制19为[定时DEF](statuses-v1.md)，20为[共享期限叠层](status-stack-v1.md)，21为[通用ATK/DEF修正](attribute-modifiers-v1.md)，22为[吸收盾](absorption-shields-v1.md)，23为[命中施加的物理DOT](periodic-damage-v1.md)，24为[提交施加的自身HOT](periodic-healing-v1.md)，25为[动态受治疗修正](healing-received-v1.md)。各效果族的字段、重复策略和绑定资格独立严格校验，不能互换字段或使用任意脚本。这里的DOT不扩展palette地形DOT或高度能力。

## SDK 玩家技能机制26

仅schema4/PVE，保留skill，可选additional_skills恰有一个SkillConfig对象；所有ID/数值/绑定严格校验，玩家技能总数最多2。各自CD/成本，共享准备/后摇及灵力。新快照与旧版兼容边界见[双技能接入](player-skills-v1.md)。

## SDK 自身技能机制27

主/副槽可配置self_status，通过CastSelf施放已有合法self状态；字段按执行方式严格分型，沿原事务共享准备/后摇、独立CD，不创建弹道。配置/输入/原子容量和旧版本边界见[自身施法接入](self-skills-v1.md)。

## SDK 严格敌方单位目标机制28

unit_projectile复用弹道字段，CastUnit指定Rust发布的ID/代次；接受与提交重验、不换人/追踪，非目标首接触只阻挡。主/副槽均支持，旧1～27形状保持，详见[接入说明](unit-targets-v1.md)。

## SDK 严格地面单次圆形机制29

ground_circle / CastGround按世界坐标严格落点；提交前重验，提交后冻结中心与攻击量，预警后按当前位置一次结算，两个计划容量。旧1～28保持，详见[接入说明](ground-targets-v1.md)。

## SDK 固定圆形持续区域机制30

ground_field必配duration_ms/tick_interval_ms，复用CastGround严格落点；首跳等待间隔、结束点排除，最多128跳，单次圆形/区域共享2计划槽；提交仅一次扣费，来源失效结束未来周期。旧1～29保持，见[接入说明](persistent-fields-v1.md)。

## SDK 命中预算与重复接触机制31

ground_field可选contact_policy，严格三项整数配置，按真实直接接触消耗周期/总预算，0每身份一次，正间隔按计划时刻；历史≤128，预算0结束。省略策略与旧1～30保持，见[接入说明](contact-policy-v1.md)。

## SDK 方向型直线穿透机制32

piercing_projectile使用Cast方向，必配ttl_ms/max_hits；连续扫掠、墙截断、按路径顺序每身份一次及总预算，提交原子门禁、射程/期限与源失效清理，旧1～31保持。见[接入说明](piercing-projectiles-v1.md)。

## SDK 移动弹道重复命中机制33

piercing_projectile可选repeat_interval_ms，0每身份一次、正数按最新成功逻辑时刻给资格，真实扫掠才接触、每步每身份一次、错过不补跳。首次/重复共用总预算，成本/CD只提交一次，旧1～32和省略字段形状保持。见[接入说明](projectile-repeat-v1.md)。

## SDK 固定线形区域机制34

line_field复用Cast方向输入，length/width初始化固定胶囊几何，提交取当前起点/重验足迹并在首墙/地图边界截断；warning/duration/tick及必填contact_policy复用原调度/预算。与圆形共用两计划容量，Rust发布实际line端点/宽度，旧圆形省略line。见[接入说明](line-fields-v1.md)。
