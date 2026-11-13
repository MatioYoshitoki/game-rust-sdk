# 通用战斗配置 v1

SDK 0.90.0，机制105，schema7，协议6。此能力由配置顶层显式的`combat_rules`对象启用；旧机制与缺省路径保留原结算及随机消费轨迹；属性聚合超过u32边界时现改为饱和，避免原先的整数回绕。完整示例见[四人](../guides/config-cookbook.md#combat-rules)arena-a.json)与[八人](../guides/config-cookbook.md#combat-rules)arena-b.json)。所有对象拒绝未知字段与非法引用。

## 初始化与数值表

`profiles`登记有界战斗配置，`default_member_profile`、`default_npc_profile`为默认引用。`member_profiles`按本局参与者ID覆盖，`npc_profiles`按怪物模板ID覆盖。命中/暴击/闪避等使用配置组的bps值，替代旧NPC模板中对应的辅助字段；HP/MP/ATK/DEF/SPD沿用经认证事实或NPC模板。宿主仍提交经认证的属性点事实，Rust取用；这些配置引用不授予账号权限。永久装备与成长记录的完整派生仍归R5.1。

| 配置 | 语义与边界 |
|---|---|
| `profiles` | 最多64组，包含`tier`（0～31）、可选`element`/`element_resistance`、`defense_baseline_attack`及命中/暴击/闪避/暴伤/威力点倍率；属性、倍率范围装载时检查 |
| `elements` | 最多16个唯一元素ID，Rust不固化元素名或克制循环 |
| `tier_multipliers` | 攻方tier减守方tier的整数差到倍率，值为整数bps或精确`{numerator,denominator}`；分母须为1～1000000。所需阶差必须存在，最多63项 |
| `element_multipliers_bps` | 攻击元素→目标元素→倍率；任一缺失元素或未登记关系按10000处理 |
| `weapon_skill_multipliers_bps` | 武器元素→技能元素→倍率，只有攻击规则显式提供武器元素时应用 |
| `basic_attack` / `skills` | 普攻规则和按已登记伤害技能ID绑定的规则；未覆盖技能采用普通物理规则 |
| `npc_attacks` / `periodic_damage` | 按NPC模板与伤害状态定义绑定规则；NPC近战/弹道、直接/周期效果共用适用结算阶段 |
| `healing` | 按即时治疗技能或HOT状态ID绑定`fixed`、`source_attack_bps`、`target_max_hp_bps`；最多128项 |
| `dodge_cap_bps` / `critical_multiplier_cap_bps` | 闪避最终概率上限0～10000（缺省5000）与最终暴伤倍率10000～1000000（缺省30000）；不改变旧配置的上限 |
| `periodic_sampling` | 按周期状态ID选择`source_snapshot`、`target_snapshot`、`source_live`、`target_live`；目标ATK取持有该状态的实体，发布来源仍是施加者 |
| `ground_target_policies` | 地面技能的`strict`或`clamp_to_range`；缺省strict。夹距后仍检查边界、阻挡和视线，不搜索替代落点；接受后冻结该点，提交重验失败免费取消 |

每个`AttackRule`显式声明`damage_kind`（`physical`/`elemental`/`true_damage`）、固定量、威力来源、修为乘区、破甲、吸血、不可闪避、忽略护甲及是否暴击。元素伤害须有元素引用；真伤不允许暴击或元素乘区。伤害/威力/倍率以整数bps表示，金额运算使用宽中间量并饱和到u32。

技能预算依次为：`floor(输入威力 × 威力增益bps × (10000+威力点bps) / 100000000)`，再计算`fixed + floor(威力 × damage_bps / 10000)`，最后乘`cultivation_bps`取整。输入威力可由规则指定，缺省当前有效ATK。普攻不应用技能威力增益和威力点。直接技能预算在提交时冻结，引导后续脉冲使用已提交预算。周期效果按下述采样策略执行。

预算、费用和恢复使用宽整数；普通伤害乘区沿源公式使用f64并在规定阶段floor，分子/分母输入避免先量化到整数bps。普通伤害顺序为资格/保护/免疫、直接接触闪避、阶差/元素/武器乘区、防御与减伤、直接伤害暴击、最终floor/最小伤害、护盾、HP、实际伤害吸血及触发/死亡。元素抗性缺失回退当前DEF；抗性状态在接触时读取。破甲超过100%时有效护甲为0。周期伤害不额外闪避或暴击。真伤使用显式预算，跳过普通乘区/护甲/减伤/暴击及最小1规则，仍受资格/免疫和护盾约束。

即时治疗在提交时读取来源ATK和目标上限，DOT/HOT缺省施加时冻结基础量；两者均计算`fixed + floor((来源ATK×source_attack_bps + 目标maxHP×target_max_hp_bps)/10000)`。受疗倍率在实际执行时应用，实际HP回复钳到缺口；过量转盾仍需显式转换配置。吸血以实际HP伤害为基数，不把护盾吸收或过杀算入，不能复活。

SDK 0.104.0 的成员普攻和伤害技能 `AttackRule` 可配置 `life_steal_timing: "action_end"`。缺省或 `"contact"` 逐接触执行。启用后，SDK 记录整次原生动作的实际扣血，在最后一个子计划结束时按基础吸血和当时有效的状态、受疗倍率与生命缺口执行一次。治疗先于同一动作的终态被动。每个动作占 `max_pending_action_totals` 一行，整组预算不足时清除该动作的终态累计。来源保护或退场时清理待结算吸血。现有 `heal_resolved` 事件和原生输入继续可用。NPC 与周期规则使用 `contact`。

显式`periodic_sampling`同时决定ATK标尺和采样时点：snapshot在施加/refresh时读取所选实体ATK及目标maxHP；live在每次实际执行时重读（迟到补跳以执行时属性为准）。没有恢复公式的HOT仍按目标maxHP比例计算；没有攻击规则的DOT仍按所选ATK比例计算。目标的防御、减伤、护盾和受疗均保持执行时读取。`PeriodicView.power_sampling`声明策略，live模式的`raw_damage/raw_heal`显示最近一次采样结果，tick事件携带本跳确切值。保护会消耗本跳机会；目标/来源退场按现有状态生命周期移除。

## 一个身份承载复合状态

`timed_compound_modifier`使用`modifiers`数组（1～8个不同数值属性），共用原状态的来源、目标、身份、重复策略、层数和期限。可表达一个狂暴状态同时ATK增加、DEF降低；客户端观察同一个`StatusView`及其修正列表。支持refresh/ignore/stack与来源分组；到期/驱散/来源退出按一个实例移除，属性读取无副作用。

成员与NPC移动状态均取最强减速和最强加速相加，Root额外阻止移动。复合数值项限已支持的数值属性，控制/免疫标记可通过下述flags加入同一实例。`element_resistance`、`max_hp`、`max_spirit`、`spd`是新增数值属性。原值、比例和时长来自配置，单项范围和聚合上限装载时校验/运行时钳制。临时maxHP最低为1，maxSpirit/SPD最低为0；增大上限不附送当前值，到期、驱散或来源退出后钳制超出部分，不复活、不改永久事实。即时/HOT、道具、自然恢复、光球及快照读取当前有效上限；光球烘焙量仍在击杀时冻结。SPD先派生再进入既有移速曲线，加减速在其后应用。

`pve.status_rules.attribute_caps_bps`可按数值属性声明正向聚合上限，缺省沿用旧规则。每属性最多1000000bps，减伤/冷却缩减/控制或有害缩时/暴击概率不得超过10000bps；负向比例最低−10000。单个状态强度与最终聚合上限独立。控制标记、移动加减速不能作为此表键，移动仍取最强规则。普通伤害的最小伤害规则仍生效，100%减伤不等价于伤害免疫。

`harmful_duration_reduction`为有害状态缩时比例。定义可显式声明`harmful`；缺省DOT、控制/减速或含负修正的状态视为有害。控制缩时和有害缩时独立乘算后一次floor，最短1ms；已有状态不会被回溯改写，refresh重新从原定义采样。控制免疫先拒绝控制定义，仍可处理同组未被拒绝的效果。

道具配置`grant_statuses_without_recovery:true`允许满资源时使用纯增益道具，须显式绑定自身状态，保留背包版本、冷却和容量检查。机制105的`on_use_self_status_overrides`支持完整复合`modifiers`替换、单属性强度及期限；覆写通过同一套状态验证，控制状态只能覆写合法期限。只有实际恢复量的旧道具仍拒绝满资源无收益使用。

## 有界被动触发

`triggers`最多32项，每项声明唯一ID、适用配置组、事实、是否接收周期事实、`every`计数门槛、冷却、优先级及一个有限效果。事实为`damage_dealt`、`damage_taken`、`shield_absorbed`、`kill`、`skill_committed`或`ally_skill_committed`；效果为按事实伤害治疗、恢复灵力、施加自身状态、按实际伤害/吸收量反伤。元素反伤须显式绑定合法`element`；真伤反噬不携带元素。触发治疗发布`heal_kind=triggered`，与普通吸血区分。没有表达式、回调或动态脚本。

`counting`缺省`contact`，也可配置`action`：同一来源代次、同一触发、同一施法只接收首次合格事实；区域多目标/多跳和引导多枚弹道共享归属。普攻/NPC弹道各自为一次动作，近战单次接触为一次。该模式使用首个事实的量，不在施法结束后汇总伤害；`include_periodic:true`与action模式的组合在装载时拒绝。去重记录按活跃计划保留，到下一步释放已结束计划，每计数器硬限256条，超限发布容量拒绝，不淘汰活跃记录后重复奖励。

每来源实体代次和触发ID保存独立计数/冷却；达到门槛但尚在冷却时保留门槛，下一合法事实可触发。优先级再按ID稳定排序，触发后计数清零。`max_triggers_per_step`为1～128（缺省32），预算耗尽发布私有`combat_trigger_suppressed`，计数不再推进；接受触发发布`combat_trigger_fired`。触发产生的效果不能递归产生更多触发。来源死亡/退出/复位清理计数与状态。自身治疗/回灵均守住上限、灵力封锁和存活资格。

这些原语只覆盖明确定义的触发语义。未知special、窗口累计恢复、动作汇总的目标依赖效果，以及未覆盖的条件/消费和模式专属反击仍需要逐项规则与执行绑定，不能由加载触发ID签收。


## 普攻攻速时钟

机制105配置可用数值属性`attack_speed`（scalar或compound，支持正负比例及stack）；它不复用ATK或SPD。身份速率10000bps，-3000表示冷却推进速率7000bps；基础间隔仍来自普攻/NPC近战配置。正向聚合帽由`attribute_caps_bps.attack_speed`配置（缺省10000），负向最低-10000，零速率暂停推进，状态有限到期后恢复。

时钟保存尚未完成的普攻冷却工作量，应用/刷新/叠层/清除前先推进到当前逻辑时点；跨过状态到期点分段累计宽整数工作量，只在计算就绪时刻时向上取整。1000ms工作量全程70%速率为1429ms；前300ms为70%之后正常则1090ms。当前冷却不会被新状态重置，也不在读快照时消费工作量。成员普攻与NPC近战共用该时钟；技能CD、准备/后摇、蓄力、引导与已发弹道不变。排队普攻在就绪时刻变化时调整提升与宽限窗口，保留动作ID、目标代次和一次提交。

镇龙压制首个属性按[OBS-05裁定](../decisions/SDK-5-88-attack-speed-mapping.md)归一为攻速-30%。其龙威资源、按层移速和满层控制不由此推定已实现；演示中的`training-tempo`为独立验证配置，1500ms测试期限不覆盖原5000ms投放。


## 同一护盾实例的记录与结束效果

`combat_rules.shield_records`为盾定义ID到规则的映射（最多16项，须引用现有absorption_shield）。类型片段见[JSON Schema](combat-rules-v1.schema.json)；完整配置引用、实时身份、余额合计和生命周期仍由Rust及Node/H5解码器验证。

| 字段 | 语义 |
|---|---|
| `basis` | `incoming`记录护甲/适用减伤后、共享盾池吸收前来袭量；`own_absorbed`只记录此实例实际分到的吸收量 |
| `cap_fixed` / `cap_power_bps` | 上限为fixed + floor(已解析来源威力×bps/10000)，施加/refresh时冻结，饱和到u32；fixed≤1000000、比例≤100000 |
| `damage_reduction_bps` | 盾存续时的减伤，0～10000，仍受正常聚合帽；通过同一实例modifiers派生，到期/破盾/消费同时移除 |
| `refresh` | `reset`清空记录，`preserve`保留且钳到新cap；二者沿现有统一池刷新盾余额，不额外创建状态 |
| `on_depleted/on_expired/on_consumed` | 各结束原因独立启用；自然到期不冒充破盾，主动驱散/退场/复位/关局永不释放 |
| `counter` | 可省略；ratio_bps为记录比例0～10000，种类physical/elemental/true_damage，元素种类须显式合法element；ignore_armor、radius(0,2000]、max_targets 1～32显式配置 |
| `counter_on` | 可选的反击原因列表，须存在 `counter`，且各原因都已由结束开关启用。省略时各已启用的结束原因均可反击。余韵状态仍按结束开关执行。 |
| `counter.on_hit_target_statuses` | 最多2项既有非周期/非盾状态；复用资格/伤害免疫和命中状态入口，可配置减速等 |
| `after_end_target_statuses` | 最多2项非周期、非盾、非敌对控制的既有状态，由原来源施加给原持有者；容量不足按普通StatusRejected反馈，不回滚已结算反击 |

已解析来源威力使用有效ATK、技能威力增益及配置威力点；不隐式借用普攻武器覆盖值。盾授予仍取本次目标有效maxHP，组内状态施加前冻结相关输入。盾池上限、记录上限和当前HP相互独立。incoming是有界观察账；不同来源可各自观察同一次输入，不能把记录解释为第二份吸收。own_absorbed的各实例记录增量合计不超过本次实际吸收。

`ShieldView.record`可选，含`basis/recorded/cap`；没有记录规则时完全省略，旧盾输出不增加字段。recorded不超过cap。剩余盾量继续经过既有统一池排序；记录盾的零授予刷新按reset清理，不留下免费减伤或伪造破盾反击。已授予盾不会因后续maxHP降低而重算，因此Node/H5检查u32余额、granted/remaining关系及自身精确合计，不使用当前maxHP错误拒绝旧余额；NPC仍只公开其合规可见来源及总额。

结束时先移除/消费原实例，再在本次伤害及死亡清理后执行一次。来源与持有者须存活且不在保护/返回态，反击以持有者当前位置为圆心，按距离/ID/代次排序，重验敌对、范围、视线和免疫，归属原状态来源。共享伤害链执行护甲/减伤/盾/HP；无额外闪避/暴击、回灵、被动触发或新增记录。护盾反击与普通反伤都遵守派生阻断，嵌套伤害隔离各自待处理的结束记录。

`max_shield_releases_per_step`缺省32、`max_shield_counter_hits_per_step`缺省128，均为1～128；目标尝试（含免疫拒绝）也消耗命中预算。耗尽后原结束记录仍消耗，发布容量抑制，不在下一步重复结算。`shield_record_ended`只发给原来源/持有者有权观察者，携带reason、recorded、raw_counter、targets_hit及可选suppressed；raw_counter是原始反击预算，实际扣血仍以公共damage事实为准。结束原因未启用的正常清理不是错误，raw_counter为0。

记录盾可由既有溢出治疗转盾直接引用，见下文过量恢复契约；结束/命中后再创建周期或护盾派生链仍装载拒绝。未支持组合不能静默忽略记录配置。完整高阶功法、装备威力和模式池仍依原工作包验收。

协议 6 的记录盾定义可选 `shield_family`，标识由 1～64 个 ASCII 字母、数字、下划线、点或连字符组成。家族成员须来源隔离、使用刷新策略，并配置为不可转移的增益。家族盾通过普通状态授予，溢出治疗仍用独立盾定义。

同一来源和目标的同家族盾共用一个位置。先在统一盾池中为候选计算可授予量，计算时扣除其它盾余额。候选严格大于旧盾剩余量时才更新。同定义沿用原实例及记录刷新策略，换定义时清理旧实例并从新记录起算。较弱或等量的候选保留旧实例、余额、时钟和记录，正常提交的技能仍结算费用与冷却。替换使用 `reset` 原因，清理本身无反击或余韵。 [双家族配置](../guides/config-cookbook.md#combat-rules)source-guard-families.json)

## 条件、概率与原子消费

触发定义可选`conditions`与`chance_bps`（0～10000，缺省必定）；conditions仅含以下有类型字段：`minimum_amount`、`owner_hp_at_most_bps`、`other_hp_at_most_bps`及`status`。生命阈值用宽整数比较当前HP/有效maxHP，包含等号；事实量仍是实际HP伤害/实际吸收量，技能提交为0。条件不满足不推进计数或动作去重。

`status={subject:owner|other,definition_id,at_least,consume}`只匹配由触发者本人创建的对应实例，不借用他人的层数。consume为0只检查存在；其余须满足consume≤at_least≤定义层数上限。部分消费不改applied/expires时间，整份消费产生consumed移除原因，发布私有`status_consumed`事实及剩余层数；记录盾可按on_consumed释放。

顺序为条件→每步预算→动作去重/计数/ICD→效果容量预检→概率→状态消费→计数清零/开始ICD→fired及效果。容量预检考虑整份消费将释放的槽位；容量失败保留就绪计数且不消费、不启ICD。概率失败同样不消费、不启ICD，保留计数门槛；下一合格动作重试，同一动作不重复摇奖，0/10000不消费随机数。效果按已有缺口钳制，已经接受的回灵/治疗不保证产生正收益。此契约不涵盖beforeDead保命、跨目标计数隔离或任意表达式解释器。

## 状态处置和致死保护

`status_policies`按状态定义ID登记`disposition=beneficial|harmful`、必填`dispellable`、`transferable`和`blocks`数组。未登记状态不能由通用处置动作选择。`blocks`最多包含两个不同类别，仅拦截目标新获得或刷新的对应可驱散状态；不依赖属性符号，不移除已有实例。政策表最多128行，并受既有状态定义数量限制。原有道具的显式净化白名单继续保持原契约。

触发效果新增`{kind:"dispel_statuses",subject:"owner"|"other",disposition,max_count}`和`{kind:"transfer_statuses",from:"owner"|"other",disposition,max_count}`。`max_count`为1～16。按绝对到期、施加时间、实例ID稳定选择；整体实例含全部层数。转移到另一个事实参与者，保持实例ID、原来源、层数、数值和绝对期限。允许数值/复合数值、移动属性及原生吸收盾（含记录/过量恢复生成的盾）；控制/免疫及成员灵力封锁可按下述原生控制契约转移；原生DOT/HOT沿其关系与调度契约，窗口/净化载体按下述完整生命周期契约转移。目标受保护/归位、同目标、容量满、重复来源身份、封禁或无合格实例均整体拒绝；无合格实例不会启动ICD/消费条件状态。转移不会按新最大生命重新授予、重置时长或重触发施加奖励；原来源退出仍清理实例。

条件状态消费、处置容量和封禁在同一个预览里验证。整份消费释放槽位，部分消费不释放。通过后一次提交。驱散使用`cleansed`原因，既不触发破盾也不触发自然过期效果。`status_transferred`携带`previous_target`及完整`status`，只发给原来源、原持有成员和新持有成员。原先仅观察NPC的其它成员只收到`status_removed(reason=transferred)`，快照移除原可见项，不公开新持有者的私有内容。

`death_guards`按已注册战斗profile登记`{retain_hp,heal_max_hp_bps,self_statuses}`。保留生命1～1000000，恢复比例0～10000，附带自身状态最多2个，禁止周期/盾/有害控制/灵力封锁。每个实体代最多成功一次，重连保留使用状态；实体换代可重新获得。减伤和盾结算后，只有剩余伤害将当前HP归零时才预检。容量/封禁失败不消费次数，也不部分保命。成功后先保留`min(当前HP,retain_hp)`，伤害事件和吸血仅使用实际扣血；再按当时最大HP比例计算恢复，经受疗倍率和缺口钳制，最后施加自身状态。零受疗仍保留生命，恢复不会抵扣名义费用。

`death_guard_activated`包含伤害来源、保护目标、`prevented_hp_loss`（本次实际阻止的HP损失，不是过量伤害）及恢复后的HP，仅目标及其合法自身状态观察者收到。直接、DOT和反伤共用伤害入口；完全吸收、免伤、断线保护不消费次数。HP费用与强制死亡/关局绕过保护，不能无限延长模式或把死亡奖励伪造成存活。

协议 6 可通过 `combat_rules.status_death_guards` 为最多 16 个状态定义配置窗口保命。载体须为来源隔离、不可转移的自身增益，采用定时数值或复合状态与刷新或忽略重复策略。配置项包含 `cooldown_group`、`cooldown_ms` 和上述格式的 `guard`。冷却至少一个 World 步长，最多 600000 ms，同组的定义使用同一时长。附带状态沿用原生保命约束，且须为可独立结算的来源隔离状态。

实体仅使用自身来源且仍有效的载体。多个可用载体按到期时间、施加时间、实例 ID 选择。没有可用载体时按原战斗档案规则检查。选中载体后先校验附带状态的封禁及容量，成功才保存实体代次与配置组的绝对期限。载体更新或结束、同一 World 重连均保留期限，实体清理会移除记录。事件继续使用私有 `death_guard_activated`。 [磐心守一样例](../guides/config-cookbook.md#combat-rules)source-lin-mi.json)

## 目标计数和原生动作汇总

`counter_scope`缺省`owner`；`target`使用事实的另一实体kind/id/generation，独立保存进度、ICD及首次事实的action去重。不是按裸ID共享状态。`max_trigger_targets_per_rule`为每来源/触发1～128个计数器，缺省32；容量满拒绝新目标，不淘汰仍有效的目标。死亡/退出/换代清理旧目标；仍有该目标的待汇总时保留到汇总完成，防止致死一击抹掉已有进度。读取与重连不重置。

`counting: "action_total"`与旧`action`分开：不使用首个事实的伤害，而是累计整次动作的所有实际量/接触次数，到最后原生计划结束时才检查条件和奖励。支持`damage_dealt`、`damage_taken`、`shield_absorbed`，只接收直接事实；`include_periodic:true`装载拒绝，反伤/记录盾派生仍由递归阻断排除。伤害基数是实际HP损失，盾吸收只进入对应吸收事实；致死和过量攻击不虚增伤害，零实际量不创建汇总。owner范围覆盖整动作所有目标；target范围对每目标分别累计。

原技能提交、普通/穿透弹道、区域和引导子弹保持同一权威动作ID；键包含动作创建者kind/id/generation。两个成员相同输入ID不会互相等待。代表目标是确定接触顺序的首个合格目标，记录完整原身份；其死亡或换代不删除已产生的实际量。区域最后一跳、命中预算耗尽、阻挡/到期或引导结束/中断后，必须等全部已提交子弹结束，在步尾完成一次。取消不会补发未来跳数。来源死亡/退出/保护、复位和强制终局只清理，不恢复或迟到发奖。

终态支持`heal_from_damage`、`restore_spirit`、`apply_self_status`及自身状态/生命条件。依赖代表目标的条件/消费、驱散/转移/反伤和其它事实在此模式拒绝，避免把多个目标的总量隐式作用到一个目标。最小量按汇总总量判断，状态条件和生命在终态实时读取；完整/部分消费与容量预检复用同一事务。每个完成汇总最多推进一次计数/概率/ICD；失败不消费条件状态或启动ICD，不挂起旧汇总，下一新动作可重试。每步预算与旧触发共用。

SDK 0.102.0：`action_total` 可在直接 `damage_dealt` 动作完整结束后执行 `add_status_stacks`。此组合要求 `subject: owner` 和本人计数范围，有本人同源计数器才生效。周期伤害和目标依赖条件仍拒绝；阈值叶子带原生动作归类并遵循既有预算。见[原规则绑定](../batches/SDK-5-88-action-terminal-counters.md)。

总量以u64安全整数累计，上限9007199254740991，溢出整动作拒绝；按比例计算用u128中间值，在比例之后才裁剪输出u32，最后经过受疗/缺口钳制，不逐接触取整。`max_pending_action_totals`1～1024，缺省256，限制整个World待汇总行数。超出后该创建者/动作全部待汇总作废，并记拒绝至动作结束，不能在容量释放后补累计部分量。容量拒绝和保护取消记忆各最多256个动作；极端超限清空并暂停所有汇总，直到在途动作全部结束才恢复，无隐式淘汰或重奖。

私有`combat_action_total`携带`action_id`、`action_source`（动作创建者）、`trigger_id`、`source`（触发归属者）、`other`（原代表目标）、`total_amount`、`contacts`、`fired`。承伤/吸收触发的归属者与动作创建者不同，二者不可混用。该事实是Rust冻结的结果，Node/H5只消费和显示。只发给归属者的合法自身状态观察者，不广播明细。当前形式不签收完整六家族/18器型、窗口累计恢复或其它未知special。

动作ID准入补充：同一实体代仍在准备/排队、原生执行或待汇总的动作ID，不允许被新离散动作复用；返回InvalidState且不扣费。其它成员的相同ID独立；旧动作已完整终结后才可接受同源新动作，不能通过有限输入去重缓存的淘汰制造并行合并或多次取整收益。

## 状态窗口与结束恢复

`combat_rules.status_windows`按状态定义ID绑定观察窗口，最多16条；`max_window_releases_per_step`为每步实际结束恢复预算1～128，缺省32。载体须为`source_scoped: true`、refresh或ignore的数值/复合状态或HOT。窗口与载体共用实例ID、原来源、持有者和绝对期限，不能作为第二套计时器或护盾。盾、DOT、控制、灵力恢复封锁、叠层载体装载拒绝。未配置时状态不发布`window`字段。

```json
{
  "basis": "damage_dealt",
  "include_periodic": false,
  "record_cap": 2000,
  "refresh": "reset",
  "maximum_sampling": "snapshot",
  "on_expired": true,
  "on_consumed": false,
  "hp": {"fixed": 0, "recorded_bps": 1000, "max_resource_bps": 0, "cap_max_resource_bps": 2000},
  "spirit": {"fixed": 0, "recorded_bps": 200, "max_resource_bps": 0, "cap_max_resource_bps": 2000}
}
```

累计主体始终为持有者。`basis`取`damage_dealt`/`damage_taken`/`shield_absorbed`时，只记录实际HP损失或实际吸收；`include_periodic`显式控制DOT事实。过杀、保命阻止量、HP费用、反伤和记录盾派生不进入累计。每个正额事实增加一次contacts，累计到cap后仍计接触数；金额和cap不超过9007199254740991，contacts饱和至u32。`basis: none`用于纯结束恢复，要求cap为0、include_periodic为false、恢复的recorded_bps为0。

`hp`、`spirit`至少一项，显式null拒绝。各项fixed为0～1000000，recorded_bps为0～100000，max_resource_bps为0～10000，cap_max_resource_bps为1～10000；fixed和两个比例不能全零。原始恢复金额为：

`min(fixed + floor((recorded × recorded_bps + maximum × max_resource_bps) / 10000), floor(maximum × cap_max_resource_bps / 10000))`

使用宽整数合并比例项后取整，再饱和至u32。HP和灵力分别使用对应最大属性。snapshot读取施加前的有效最大属性，live读取结束移除载体后的有效最大属性；实际写入始终受当前上限和缺口约束。HP再经过当前受疗倍率，灵力遵守当前恢复封锁；此结束HP恢复不再溢出转盾。refresh reset清空金额/次数并重新采样，preserve保留原金额/次数和首次快照，ignore保持整个实例不变。

仅expired和consumed可分别启用收益。主动驱散、死亡、来源/持有者失效、复位和关局只清理；自然到期在同刻周期伤害前结算，已到期的窗口不记录该时刻伤害。来源和持有者须为原实体代、存活且未处于保护/归位。重连保留记录，保护不暂停期限；到期不合法时消费结束且不补发。结束队列先取出，嵌套派生不提前结算外层窗口；预算满返回CAPACITY，不排队重奖。

状态快照中的`window`包含basis、recorded、cap、contacts、maximum_sampling、max_hp_at_apply、max_spirit_at_apply。私有`window_record_ended`包含实例/定义ID、原source、持有者target、reason、recorded、contacts、raw_hp/hp_restored、raw_spirit/spirit_restored及可选suppressed。被清理或被抑制时原始/实际恢复均为0；只发送给有权来源与持有者。HP恢复另发布`heal_resolved`且heal_kind为`window_recovery`。Node/H5共享解码器严格检查窗口结构、安全整数、cap与载体；宿主只消费事实，不计算权威恢复。

样例`operation-seal`展示两次区域的12次接触、重连后保留累计、到期HP/灵力恢复；`ally-renewal`展示指定友方HOT沿用原调度，结束收益给持有者，归属保留原施加者。两者仅为可更换配置。

## 复合控制与免疫标记

机制105的`timed_compound_modifier`可配置`flags`，同一实例可同时含有控制/免疫标记和原有数值修正。字段缺省不发布，显式空数组或null拒绝；含标记时允许无modifiers的纯标记状态。仍有一个实例ID、来源、目标、槽位和期限，数值与标记一同刷新/驱散/消费/清理。

```json
{"id":"example-bind","mechanism":"timed_compound_modifier","flags":["silence","root"],"modifiers":[{"attribute":"def","modifier_bps":-2000}],"duration_ms":3600,"duplicate_policy":"refresh","source_scoped":true}
```

最多3个唯一标记，只允许`silence`/`root`/`stun`或`control_immunity`/`damage_immunity`/`first_control_immunity`两组，不能混组。它们不作为bps属性参与数值加和。含标记时须source_scoped、refresh、无层数，attribute为空且modifier_bps为0；数值项仍遵循最多8项、唯一属性与既有范围。含标记状态可按下述控制/免疫契约转移，status_windows绑定仍拒绝；周期、盾、标量载体及其它枚举属性不能携带flags。

敌对控制只投向敌对实体；免疫标记只投向自身或既有友方关系。成员/NPC自身提交可绑定免疫复合状态，敌对命中可绑定控制复合状态；装载拒绝自身控制、敌对命中免疫、恢复道具施加自身控制。指定净化道具仍按配置定义ID移除完整实例，不只擦除其中一个标记。此能力不新增PvP或友方复合状态技能种类。

silence阻断技能并取消未提交的技能准备、排队与引导；root阻断移动、寻路和追击；stun阻断动作及移动。已提交的弹道保留其原成本与归属。成员和NPC在原生入口读取同一标记，Rust公开StatusView.has_attribute用于查询标量或复合标记。生效后的属性修正按原聚合链处理；容量不足不施加数值、不产生打断。

控制免疫拒绝整份控制复合状态，数值部分也不进入。首次免控按已有稳定顺序消费整个免疫实例，包括其中数值，多个来袭标记只消费一次；正常免控优先且不消耗首控。首次免控消费后，先重算并钳制当前上限，再为同批其余合法状态采样，防止按已移除的最大属性授盾。控制/有害缩时对整份敌对状态各乘一次，刷新从原始时长重采样。伤害免疫沿统一直接/周期/派生入口生效，免控不免伤。来源、持有者退场或换代清理完整状态；重连保留ID和绝对期限，不延长时长。

快照与status_applied/status_refreshed携带同一flags和modifiers，既有移除/消费事实仍按实例ID报告。Node/H5共享解码器检查枚举、分组、唯一性和载体，拒绝混入周期/盾/层数/窗口；增量失败保持旧缓存。含有友方来源的自身状态只允许免疫组，NPC私有投影继续只展示本人来源或NPC自身来源的实例。示例battle-pose在数值组合中附带免控，compound-lock在同一控制身份内附带防御降低；参数与定义ID均可更换。

## 周期与到期净化

`combat_rules.status_cleanses`最多16条，按状态定义绑定`on_tick`/`on_expired`，至少一项；每项为`{disposition: "beneficial"|"harmful", max_count: 1..16, scope: "all"|"since_application"}`。载体须为来源隔离的refresh/ignore无层数状态，拒绝盾和DOT，on_tick额外要求HOT。可与已支持的数值、复合标记、HOT结束恢复组合；载体可按显式政策整体转移，保持原施加/期限与调度。没有独立时钟、回调或脚本。

```json
{"ally-renewal":{"on_tick":{"disposition":"harmful","max_count":1,"scope":"since_application"},"on_expired":{"disposition":"harmful","max_count":16,"scope":"all"}}}
```

固定作用于载体持有者，沿用status_policies的显式类别和dispellable。只选来源仍有效、未到期、符合类别的整个实例，排除载体自身；按到期、施加时间、实例ID排序后截断。since_application要求候选applied_at_ms≥载体applied_at_ms，包括同固定步和后续刷新，不代表更细的事件顺序。refresh按原状态重置期限与HOT调度，ignore完全保留；不会新增第二套跳数。通过同一选择器执行的旧触发驱散/转移保持原排序与许可。

HOT净化在本跳治疗和过量转盾后执行，满血/零恢复不跳过净化。来源、目标实体代次、存活、保护和归位实时重验；保护/归位跳过消耗本跳机会，不在重连后补发，迟到与最后一跳沿用原HOT规则。普通载体到期先移除并钳制上限，再依次处理到期净化和窗口恢复，同刻 DOT 随后执行。显式包含到期末跳的组合 HOT 在同刻 DOT 之后执行末跳治疗和本跳净化，再移除载体、执行到期净化与恢复。此轮到期效果在该步移动和施法提交前处理。消费、驱散、死亡、退出、复位或关局只清理载体，不执行到期净化。移除记录盾/窗口/另一净化载体使用cleansed原因，不反击、不恢复、不递归。

`max_cleanse_operations_per_step`为全局每步操作预算1～128，缺省32。合格的空选也占预算；到期动作按到期、施加时间和实例ID稳定排序，先于同刻HOT净化。超限发布CAPACITY并消费本次机会，不挂起旧操作。原生治疗、护盾、触发和窗口预算保持独立；属性清理前先推进旧攻速工作量，清理后立即钳制上限。

状态快照的可选`cleanse={on_tick,on_expired}`仅声明能力，两项布尔且至少一项true；不携带额外周期时刻。私有`status_cleanse_resolved`包含载体instance_id/definition_id、原source、持有者target、phase（tick/expired）、tick_index（仅tick必填）、scheduled_at_ms/executed_at_ms、removed_count及可选suppressed（INVALID_TARGET/INVALID_STATE/CAPACITY）。只报告数量，不包含被清理的第三方状态身份。结果发给既有合法观察者；成员来源/持有者分离时仅双方接收，NPC自身载体沿用已有公开NPC观察规则。移除事件仍按被移除实例原有观察者路由。Node/H5共享解码器检查能力字段、HOT/非盾/非DOT及无层数载体，增量拒绝保持旧缓存。

样例指定友方HOT在首跳清除一份实际减益，后两跳空选；中途真实重连保持原期限/调度，在最后一跳后施加的新减益由到期动作清理，随后窗口恢复生命/灵力。额外两次技能提交产生的灵力费用进入同一恢复缺口，不由前端补算。此样例不签收原功法全部周期回灵、种子或特殊策略。

## HOT双资源恢复与治疗余量分配

`combat_rules.periodic_recovery`最多16条，只引用已有HOT定义，至少提供spirit或overheal_spirit。spirit公式为`fixed + floor((选定ATK×source_attack_bps + 持有者maxSpirit×target_max_spirit_bps)/10000)`；三项不能全零，范围分别0～1000000、0～100000、0～10000，使用宽整数合并后取整并饱和至u32。继承同一HOT的source/target、snapshot/live采样策略：snapshot在施加/refresh冻结，live每次执行重读；未声明策略按来源快照。refresh/ignore与HOT身份、原期限和调度保持。

```json
{"ally-renewal":{"spirit":{"fixed":1,"source_attack_bps":0,"target_max_spirit_bps":0},"overheal_spirit":{"bps":5000,"when":"full_before_tick"}}}
```

转换bps为1～10000，when为full_before_tick或any_excess。前者要求本跳治疗前当前HP已达到有效最大HP，后者可使用补满缺口后的余量。机制105统一配置模式下成员与NPC自身HOT均先走受疗与缺口，旧配置保留原路径；基础回灵独立计算、先写入，随后从本跳实际HP余量转换灵力。灵力按当前maxSpirit/缺口/回灵封锁钳制，不乘HP受疗；HP受疗为0时基础回灵仍可执行，但无HP预算用于转换。

假设HP余量E、转换倍率r，则可转换`floor(E×r/10000)`；实际回灵A受剩余缺口约束，消耗`ceil(A×10000/r)`治疗预算，剩余才交给原过量转盾的倍率/容量。转换不合条件、回灵封锁或无缺口时A=0，预算不被占用；舍入余数也可交给转盾。每跳满足`hp_excess = spent_heal + remaining_excess`，没有把同一份HP余量全额转换两次的路径。盾容量不足后的损失不会重新尝试回灵或补发。

例如E=101、转换50%、基础回灵10、初始灵力缺口100：先回10，再转换50、消耗100治疗预算，最后1才可转盾。若初始缺口只有1，基础回灵填满后转换为0，101余量仍按转盾规则处理。倍率3333bps且只缺1灵力时消耗4治疗预算，不会只扣3后无中生有。

完整顺序为HP→基础回灵→余量回灵→剩余转盾→净化；结束仍按净化→窗口恢复。临时最大属性下降先钳制，原来源与实际持有者独立，收益只写给持有者。保护/归位跳过仍消费原跳机会，重连不补发或刷新；死亡/退出/换代/复位/关闭沿原状态清理。NPC自身HOT也可使用此规则；不新增敌对治疗或独立的回灵时钟。

PeriodicView的可选raw_spirit为本跳原始基础回灵额，仅配置了此规则的HOT携带（纯转换为0）。snapshot值保持，live执行时更新。私有periodic_spirit_tick携带instance_id/definition_id、source/target、tick_index、scheduled_at_ms/executed_at_ms、raw_spirit/base_restored、hp_excess、conversion_bps/conversion_eligible、converted_spirit、spent_heal、remaining_excess、outcome。eligible只表示本跳存在符合条件的HP余量，封锁/满灵力仍可能使实际转换为0。事实在HP跳事实之后、转盾和净化之前发布给既有合法观察者；NPC自身载体沿用原公开观察规则。

Node/H5对本人、友方和NPC周期视图使用统一严格校验，识别四种power_sampling及物理/元素/真伤种类，拒绝DOT携带raw_spirit、非法数值/null/未知采样和额外时钟，错误增量不改变旧基线。宿主消费权威实际量，不执行另一套回灵计算。


## 过量恢复生成记录盾

机制105允许原有`overheal_shield.status_id`引用`combat_rules.shield_records`中的盾定义，覆盖即时自身/友方治疗、HOT及恢复道具。无需新增转换字段；仍要求来源隔离的refresh盾。先执行HP恢复与已配置的HOT基础/余量回灵，只有剩余治疗预算进入原转盾倍率，单盾和统一池预算共同限制实际增量。HP余量本身不写入护盾记录。

只有实际增加正额盾量时才准备记录规则：使用此刻有效原来源ATK、技能威力增益和profile威力点计算cap，与配置减伤/refresh策略一同提交给原盾实例。道具的准备在恢复、库存/版本和冷却写入前完成；失败保持业务状态。技能/HOT已经结算的恢复保持，只发布转盾拒绝。状态类别封禁同样检查，来源必须仍为当前存活实体代次。

余额按既有过量转盾方式累加；真正刷新时保留实例ID，更新绝对期限及冻结cap。reset清空记录，preserve保留旧记录并钳制到新cap。没有增量（零余量、满单盾或满总池）不刷新期限、不重采样或清空记录。记录之后的适用来袭或本盾实际吸收，继续遵守适用减伤→记录→统一盾→HP顺序；真伤遵循既有忽略减伤契约。

生成后的记录盾使用原生耗尽/自然到期/消费及一次结束释放；退出、死亡、驱散、复位、关局只清理。派生反击不记账或递归。原来源/持有者私有投影及重连与普通记录盾完全一致；Node/H5不根据治疗量重建记录或盾余额。`--sdk-info.combatRules.shieldRecords.grantSources`声明四种授予来源。示例overflow-shield的数值、基数和比例均可替换，本组不签收完整原功法或模式组合。


## 原生护盾转移的预算与记录

`status_policies.transferable:true`可用于absorption_shield，仍要求dispellable为true。`transfer_statuses`搬移完整实例，不授予/刷新，也不触发结束动作。实例/原来源、granted/remaining、record基数/累计/cap、存续减伤、施加/绝对到期时间原样保留；不按新持有者最大生命重授盾或重采样原来源威力。普通盾、过量恢复生成的盾和记录盾走相同事务。

只在本批含盾时追加完整目标池预检：在条件状态消费后的预览里模拟全部数值/盾转移，再按最终有效maxHP计算统一盾池上限，使用u64累计全部有效余额。历史单盾余额可超过新持有者的单盾授予比例，但完整总池必须容纳它；零余额、同源身份冲突、类别封禁、槽位/总池不足、保护/归位或无效实体均整体拒绝，不裁剪或部分转移，不消费条件或启动ICD。目标已有的盾与临时最大属性必须参与预览，不能借用即将被消费的上限。

转移后旧持有者不再吸收或记录；后续只在新持有者身上运行原统一伤害/记录链。结束反击中心读取新位置，敌友与伤害归属仍按原来源；同一到期/耗尽/消费实例只释放一次。旧持有者不是原来源时，其退场不清理已转走盾；原来源或新持有者退场仍清理，不补发收益。重连保留所有余额、记录和期限。私有转移/结束只给有权来源/持有者；旧NPC旁观者只收到移除，不能借转移看到新私有盾明细。

`--sdk-info.combatRules.statusOperations.transferKinds`包含absorption_shield，shieldTransfer描述preserve余额/记录、final_live_shared_pool、禁止部分/重授。周期与控制转移分别见后续专门契约；窗口/净化载体按后续生命周期契约转移，全原内容投放仍须独立验收。


## 原生DOT/HOT转移

机制105的显式transferable政策支持periodic_damage与periodic_heal，仍要求dispellable。复用transfer_statuses稳定选择、容量/身份冲突/封禁预览及条件消费原子提交。只改持有者，完整保留实例、原来源、施加/绝对到期时刻、interval、已完成/总跳数、next_tick_at_ms、最近raw量与四种采样模式；没有第二时钟、重施加/刷新或补跳。窗口/净化载体转移见下述完整生命周期契约；控制/免疫转移见下述专门契约。

DOT新持有者必须相对原来源敌对，反射至来源或同阵营使整批拒绝；其后沿统一周期伤害链与原归属。HOT的可转移政策显式授权增益搬移/夺取，因此可跨阵营恢复新持有者；普通施加的自身/友方限制保持。非可转移的HOT没有跨阵营恢复权限。

snapshot量不重新采样；source_live仍读原来源，target_live在原下一跳读新持有者ATK及实时最大资源。实际收益读取新持有者受疗/HP/SP缺口/灵力封锁和上限；双资源与余量转盾沿原顺序，已生成的旧盾不隐式搬移。原来源保护/归位同样跳过并消费原跳机会；来源/新持有者退场/死亡/换代清理，旧持有者非来源时退出不误清。重连不重置调度和期限。

status_transferred完整周期元数据与后续原生periodic_tick/heal_tick/spirit_tick按当前来源/持有者投影；旧NPC旁观者只收到移除，不获得新持有者私有状态。Node/H5复用严格周期解码，不在客户端补算或重放。--sdk-info中的periodicTransfer声明上述来源/采样/关系契约。公开配置以既有吸收事实触发NPC自疗并转移，业务ID与数值可替换；本组不代表原功法/模式全量验收。


## 原生控制与免疫转移

机制105的显式transferable政策支持timed_control、复合flags及成员灵力恢复封锁。与原数值/盾/周期事务一样保持实例、原来源、数值/标记、施加/绝对到期时间；剩余时长不重采样或再次应用目标缩时，窗口/净化载体按下述完整生命周期契约转移。

沉默/定身/眩晕与灵力锁的新持有者须相对原来源敌对，原来源须符合生命周期资格（成员存活、非保护且已就绪，NPC存活且非归位）；灵力锁只可进入成员。普通数值移速沿既有转移契约，免疫类收益可由显式政策跨阵营搬移。转移不取消旧持有者已经执行过的中断，也不复活旧动作。

硬控制的完整预览包含条件消费与本批所有搬移。最终目的存在正常/首次免控时整批拒绝，不消费该免疫、条件状态或启动ICD；同批转入免疫也会阻止硬控。显式条件已消费并移走免疫时才可准入。这是转移预检，普通新施加控制仍按原入口消费首次免控。

成功发布原生status_transferred后，重算资源上限并调用与正常施加共用的控制后果：沉默取消技能/引导、定身停止移动/追击、眩晕中断动作/引导，NPC沉默/眩晕清读条。免疫转移不打断；后续真实控制在新持有者消费完整首控实例一次，正常免控优先。失败不提前打断/扣费或部分搬移。

Node/H5支持原来源为友方或NPC的合法免疫视图，严格检查标量免疫modifier=0且无非法混合载体、flags分组/唯一性、来源代次与目标。敌对硬控不能伪装为友方数值状态。私有转移/后续消费及到期保持有权来源/持有者，旧NPC旁观者只获移除；重连保持同一实例/期限。公开PVE通过下述指定友方技能提交事实搬移NPC来源硬控；Boss控制替换及完整功法仍须独立验收。


## 指定友方技能提交事实

机制105新增triggers.when=ally_skill_committed，适用于成功提交的ally_heal（即时或HOT）及ally_shield。owner为施法者，other为接受与提交时均重验的冻结友方目标，amount=0，action沿原创建者代次/ID；只在费用/冷却写入、action_committed及主治疗/状态/盾效果后投放一次。原skill_committed仍是自身事实，保持原时点；两者共用同一步预算。主技能拒绝/取消/源沉默或目标失效、资源/容量不足不投放；满生命的合法0实际治疗仍是成功提交，后续HOT跳/盾吸收不重放。

支持contact/action计数及owner/target作用域、既有条件/概率/ICD、施加自身状态/回灵/驱散/转移。禁止include_periodic、action_total、heal_from_damage和reflect_damage。附带触发失败保留已提交的主技能费用/冷却/收益，但其自身条件消费和ICD保持原子。目标状态条件仍只匹配触发者本人创建的实例，不借本入口扩大来源权限。

transfer_statuses.from=other可承担友方身上的NPC来源硬控，按原来源/敌对关系、资格、最终免控预览和完整实例事务执行，成功后调用原生打断。配置profile可限定承担者，任何业务ID均不参与Rust分支。四人/八人示例使用helper profile、NPC复合控制与已有ally-renew技能；这些ID和数值均可替换。Node/H5仅显示当前持有者的合法私有状态；转移只发给有权来源/旧持有者/新持有者，NPC来源不创建额外成员接收者。


## 窗口与净化载体转移

机制105的status_policies.transferable可声明既有合法窗口/净化载体，仍须dispellable。原载体资格和字段边界保持，窗口仍拒绝盾/DOT/控制/叠层，净化仍拒绝盾/DOT/叠层且on_tick仅HOT。通用transfer_statuses保持完整实例、原来源、数值/flags、原applied_at_ms/expires_at_ms、HOT跳数/下一跳、窗口recorded/contacts/cap及施加时最大属性。转移不触发净化/结束，不重采样或刷新；容量、冲突、封禁、条件消费及混合转移的最终预检整体执行。

窗口的旧累计随实例保留，转移后仅新持有者继续记录。snapshot结束公式保留原施加时最大属性，live读取新持有者，实际恢复受其受疗/资源缺口/封锁限制。净化始终选择当前持有者；since_application保留原施加边界，包括新持有者在转移前、原施加后获得的状态。没有隐式的since_transfer含义。HOT转移前生成的盾仍留在旧持有者。

原来源/新持有者获得后续跳、净化和结束事实；旧持有者只收到本次转移，不继续收到无关私有明细。旧非来源持有者退场不误清，源/当前持有者失效仍按原生命周期清理。重连保留完整记录与调度，保护/归位和预算失败不延长或补发，结束净化先于窗口恢复且最多一次；驱散/退场不产生到期净化/恢复。--sdk-info的statusWindows.transfer、statusCleanses.transfer及statusOperations.lifecycleTransfer描述这些能力。

## 按目标配置组处理控制

机制105的可选`combat_rules.control_policies`按现有profile配置控制时长上限与替换定义，不依赖地图、Boss或技能ID分支。例如：

```json
{"ordinary-control":{"max_duration_ms":1800},"control-resistant":{"replacements":{"compound-lock":"control-softened"}}}
```

每profile可配置`max_duration_ms`（1～3600000）及原硬控定义ID到替代定义ID的`replacements`。最多64个profile、128条映射，每profile最多16条；未知profile/定义、空策略、null或非法载体拒绝。硬控指silence/root/stun及含这些flags的完整状态。先按原请求验证敌对来源，再完整替换定义并钳制原基础期限，最后按实际类别执行既有封禁、容量、免疫及缩时。替代定义自己的预设期限不参与；同批解析为相同定义只应用一次并取最长请求期限。数值替代不消耗首次免控，不打断动作。

原定义须登记harmful政策且不能附带净化hook；替代仅限来源隔离、refresh/ignore、无层数/flags/周期/盾/窗口/净化的有害数值状态，也须登记harmful政策。原生转移保持身份：若新持有者的控制policy要求替换或期限帽短于剩余时长，则原子拒绝，不截断、重授或消耗条件/ICD。

通用属性`basic_interval_delay`表示普攻间隔增加的bps，标量范围1～100000，须source_scoped、refresh/ignore、无层数；复合项允许0～100000并沿原数值聚合帽。它被归为有害状态，默认聚合上限100000，可由attribute_bonus_caps配置。它与攻速组合的间隔倍率为`(10000 + interval_delay_bps) / (10000 + attack_speed_bps)`：1000ms在1500bps延迟下为1150ms，在额外100%攻速下为575ms。只作用于成员普攻和NPC近战间隔，技能时序不变；工作时钟累计整数余数，到期/净化/来源退出保留已完成进度，分母变化时余数向下换算，实际提交按World步长向上对齐。

复合数值状态可同时含`movement_slow`（-9999～-1）和间隔延迟；含减速时须source_scoped且无叠层，移速减速沿既有最强减速规则。两项共用一个实例、来源、刷新及期限。公开两配置以-5000/1500验证50%减速及间隔增加15%；冻结/眩晕的内容映射由接入方配表，完整寒霜叠层/破冰内容另行验收。

## 命中条件与目标状态转化

机制105的`direct_hit`事实在统一直接接触通过命中/敌对/免伤等检查，并完成主伤害、原伤害触发和`on_hit_target_statuses`之后投放。owner是攻击者，other是实际接触目标，amount是本次实际HP损失；全盾吸收或合法零伤害仍有命中事实。闪避/免伤/无效接触、DOT/反伤/记录盾派生不产生该事实，致死接触仍可触发自身收益。普攻、各种弹道、区域和引导复用同一入口与原action身份；支持contact/action和owner/target计数，禁止include_periodic及action_total。旧damage_dealt/damage_taken仍仅接受正额实际HP损失。

`{kind:"apply_other_status",status_id}`让触发归属者向事实另一参与者施加一份配置状态，可用于direct_hit/damage_dealt/damage_taken/shield_absorbed。仅允许source_scoped、登记harmful政策的有害数值/复合数值/硬控定义，禁止免疫、周期/盾/窗口/净化载体。双方须仍为有效原代次且敌对；不能借此给自己或友方施加控制。伤害承受/吸收触发时，归属者为受击者，other为原攻击者，状态原来源明确。

例如命中附带`threshold-mark`叠层后，按本人来源的目标三层消费并转控：

```json
{"id":"mark-transform","profiles":["member"],"when":"direct_hit","include_periodic":false,"counter_scope":"target","conditions":{"status":{"subject":"other","definition_id":"threshold-mark","at_least":3,"consume":3}},"every":1,"cooldown_ms":0,"priority":1,"effect":{"kind":"apply_other_status","status_id":"compound-lock"}}
```

按实际目标profile先解析控制替换/期限帽，再在已消费条件状态的预览上检查类别封禁、普通/首次免控和容量。失败不消费层数、首次免控或ICD，也不回滚已经命中的主攻击及其附带层数；action计数下同一动作仍只尝试一次，下次独立动作可重试。预检成功后原子消费并进入原生状态入口，期限缩减/刷新/ignore/叠层/打断、重连及退出沿原规则。原生命中绑定直接施控的首次免控消费行为保持。

标量movement_slow在机制105及combat_rules下可用stack，max_stacks为1～32；单层modifier_bps为-9999～-1。单实例减速为`max(-9999, modifier_bps × stacks)`，多个来源仍只取最强减速；部分消费不重置期限。复合减速和flags硬控仍禁止叠层。参数/阈值/期限/控制或数值替代由配置初始化，Rust不识别寒意、龙威或雷印等内容名称。公开[三层示例A](../guides/config-cookbook.md#combat-rules)hit-status-a.json)及[示例B](../guides/config-cookbook.md#combat-rules)hit-status-b.json)分别展示普通控制与profile数值替代；完整碎冰增伤、链雷和原内容投放另行验收。

## 消费状态增强本次命中

机制105的AttackRule新增可选`contact_status`与`armor_penetration_cap_bps`。每条规则只有一份状态条件，适用于成员普攻/技能和NPC直接攻击；periodic_damage禁止contact_status，DOT/反伤/记录盾派生也不执行它。两字段缺省时序列化省略，旧配置保持原行为。

```json
{"armor_penetration_cap_bps":5000,"contact_status":{"subject":"other","definition_id":"threshold-mark","at_least":1,"consume_max":3,"damage_bonus_bps_per_stack":2500,"armor_penetration_bps_per_stack":500}}
```

subject选择攻击者自身或当前目标，来源固定匹配当前攻击者的完整实体身份。只采样未到期且达到at_least的对应实例；消费min(现有层数,consume_max)，不借用其它施加者或代次的状态。at_least和consume_max各为1～定义层数上限，彼此独立；可要求至少3层而只消费1层。每层增伤0～100000bps、破甲0～10000bps，不能全0；乘consume_max的最大增量不超过1000000/10000bps。载体仅限source_scoped的有限数值/复合或timed_control，盾/周期/窗口/净化载体拒绝。有每层破甲时须显式设置该攻击的聚合帽（0～10000）；引用/载体、层数和乘积约束由原生装载检查。

取样点在接触通过双方资格、敌对、保护、伤害免疫和闪避之后，进入原伤害结算之前。base_raw保持原提交/计划物化的预算，新raw为`floor(base_raw × (10000 + consumed_stacks × damage_bonus_bps_per_stack) / 10000)`，使用u128中间值，最后钳至u32。每层破甲加入临时规则副本；静态、标记及实时属性破甲合并后按armor_penetration_cap_bps钳制。配置的聚合帽对该攻击所有命中生效，包括没有标记时；原未配置帽的规则保持旧行为。

消费与原status_consumed/移除事实先于本次防御、暴击、减伤、盾和HP结算。部分消费保持实例/来源/施加及到期时间，整份消费同步清理属性/当前上限及普攻时钟；既有吸血/伤害触发只使用最终实际HP损失，不额外制造一次damage或抽取随机数。条件不满足按基础攻击处理；合法零伤害/全盾吸收仍消费，落空/闪避/免伤/无效目标不消费。标记在真实命中时取样，发射后新获得、到期或先被其它攻击消费都会影响此时的结果；不改在途基础预算。

同一命中后才施加的新标记不参与本次增强，区域多跳/重复弹道只使用当时剩余层数；配置决定是否在命中后重新施加标记。原生来源/持有者私有路由、重连和生命周期保持，Node/H5仅显示原伤害与状态消费事实。[配置A](../guides/config-cookbook.md#combat-rules)contact-status-a.json)展示逐层25%增伤/5%破甲和最多消费3层，[配置B](../guides/config-cookbook.md#combat-rules)contact-status-b.json)展示另一组数值与最多消费2层；完整剑痕追击/冷却、碎冰额外威力和其它原内容投放另行验收。

## 有界链式伤害

机制105的触发效果`{kind:"chain_damage",rule:{...}}`在同一原生逻辑时刻执行有限跳跃。复用既有条件、本人来源层数消费、计数、ICD、概率及来源归属；允许`direct_hit/contact_status_consumed/finisher_kill/damage_dealt/damage_taken/shield_absorbed/kill`，不允许`action_total`或自身/友方技能提交。周期事实沿`include_periodic`与计数规则准入，派生事实不触发新链。

```json
{
  "kind": "chain_damage",
  "rule": {
    "formula": {"fixed": 0, "source_attack_bps": 8000, "fact_amount_bps": 0},
    "max_hops": 3,
    "radius": 100,
    "retention_bps": 7500,
    "final_multiplier_bps": 12000,
    "damage_kind": "true_damage",
    "ignore_armor": true,
    "armor_penetration_cap_bps": 5000,
    "on_hit_target_statuses": ["compound-lock"]
  }
}
```

`fixed`取0～1000000，`source_attack_bps`取0～100000，`fact_amount_bps`取0～10000，至少一项为正；`max_hops`为1～8，`radius`为(0,2000]，`retention_bps`为0～10000，`final_multiplier_bps`为1～100000，聚合破甲帽为0～10000。`damage_kind`为physical/elemental/true_damage；元素伤害必须配置已登记`element`，真伤禁止element，显式null拒绝。其余字段必填，附带状态数组可省略。

先预览条件消费后的有效来源ATK，一次采样`fixed + floor((ATK × source_attack_bps + fact_amount × fact_amount_bps)/10000)`，宽整数合并后钳制到u32。正式消费后按相同公式冻结整链初始预算。每次尝试后，下一跳未加末跳倍率的预算为`floor(raw × retention_bps/10000)`；只有配置的第`max_hops`次尝试乘末跳倍率并向下取整、钳制u32。提前结束不把末跳倍率移给最后一个实际目标。上例ATK=100时原始序列为80、60、54。

起点是事实的other，允许刚被击杀的同代敌对原点；存活原点仍须通过保护/退场资格。每跳在半径内按距离、实体kind/id/generation稳定选择最近且视线可达的有效敌对目标，使用地图blocks_vision；排除来源、原点及全链已尝试过的完整实体身份。来源每跳重验、目标每跳重新选择。免伤目标仍占一次尝试及预算，`applied:false`，不扣血或投放状态，之后从其位置继续。

每跳复用统一防御/抗性、减伤、盾、扣血、致死保护、死亡与清理。它不可闪避、不可暴击、不吸血，也不消费contact_status；全链抑制再触发、反伤及记录盾/窗口累计，防止派生递归。成功接触且仍存活的目标可获得最多2个不同、来源隔离且登记harmful政策的数值/复合/硬控状态；复用目标profile替换、免控及容量判断，状态失败不回滚伤害。禁止附带DOT/HOT、盾、免疫及窗口/净化载体。

`combat_rules.max_chain_hits_per_step`为1～128，缺省32，全部来源/触发共用。预检要求有效来源、敌对原点、消费后正预算、第一目标及剩余全局预算，失败不消费条件/ICD/概率。链中无新目标、零预算、来源失效或全局预算不足立即结束，保留已执行前缀，不挂起或在重连时补发。

私有`chain_damage_resolved`发布`trigger_id/source/origin/initial_raw`、最多8项`hops:[{from,target,raw_damage,applied}]`及`reason`（complete/no_target/zero_budget/source_invalid/capacity），仅来源的合法自身观察者收到。raw_damage是本跳统一伤害结算前的预算，实际HP仍读取原伤害事实。没有新增快照状态或客户端伤害计算。状态仍以原生身份/来源/绝对期限快照恢复，摘要不重放。可执行的两组配置见[四人示例](../guides/config-cookbook.md#combat-rules)chain-a.json)及[八人示例](../guides/config-cookbook.md#combat-rules)chain-b.json)。这只验收通用链及原链雷跳数/衰减/末跳系数，不代替原功法威力、初始目标控制、剑痕追击入口或完整内容投放。

## 命中消费联动与技能冷却返还

机制105支持`when:"contact_status_consumed"`，必须同时声明`contact_consumption:{definition_id,at_least}`，限定本次攻击规则实际消费的本人来源定义与层数。at_least为1～匹配攻击规则的consume_max；配置中必须存在可满足门槛的contact_status。其它事实禁止夹带此筛选，新事实禁止周期与action_total，contact/action计数和逐目标ICD仍复用。

本次实际消费记录先被捕获；主伤害、原生命中附带和direct_hit完成后，再投放消费事实。amount是本次消费增强后、进入防御前的u32预算，适合按比例继承攻击预算；实际HP伤害仍由damage_dealt等事实读取。全盾或合法零HP伤害可投放；闪避、免伤、过期及没有实际消费时不投放。门槛比较本次消费数，不比较消费前总层数或命中后新叠层。普通触发条件消费、净化、转移和派生执行不生成此事实。已完成主攻击及消费不因联动失败回滚；联动仍各自进行条件、容量、概率、计数、ICD与预算检查。

chain_damage可选`direction:"forward"`，缺省all保留旧序列。forward第一段方向为当前来源→事实原点，每跳只接受`dot(candidate−point,point−previous)>0`，再沿原半径、视线和距离/身份稳定排序选择新目标；之后以上一实际段推进。同点、垂直边界不算向前。首段没有方向或目标时预检失败，不消费联动自身条件/ICD/概率。方向是当前几何策略，不是对原在途弹道朝向的回放；瞬移、世界锚或其它方向策略仍按原工作包验收。

```json
{
  "id": "consumed-refund",
  "profiles": ["member-profile"],
  "when": "contact_status_consumed",
  "contact_consumption": {"definition_id": "threshold-mark", "at_least": 3},
  "include_periodic": false,
  "every": 1,
  "cooldown_ms": 0,
  "priority": 1,
  "effect": {
    "kind": "reduce_skill_cooldowns",
    "skill_ids": ["break-bolt"],
    "remaining_bps": 3000,
    "fixed_ms": 0
  }
}
```

`reduce_skill_cooldowns`可由合法非action_total事实触发，skill_ids为1～8个不同且已登记的成员技能，remaining_bps为0～10000，fixed_ms为0～3600000，两项不能都0。每技能按执行当刻当前截止计算`R=max(ready−now,0)`、`D=min(R,floor(R×remaining_bps/10000)+fixed_ms)`、`new_ready=ready−D`，使用宽整数。多个效果按触发优先级/ID顺序执行，后一个看到更新后的剩余时间。只修改归属成员已有且有实际正返还的技能，不创建不存在的冷却；NPC或没有实际变化时预检失败，条件消费/ICD/概率不支出。其它技能、成员、普攻、道具、准备/后摇/排队不变，不重新应用原冷却缩减。

私有`skill_cooldowns_reduced:{at_ms,trigger_id,source,changes:[{skill_id,previous_ready_at_ms,ready_at_ms,reduced_ms}]}`只交本人合法观察者；changes仅包含正变化且最多8项。Node/H5直接采用权威player_skills.ready_at_ms，摘要只供显示，不在客户端重新返还；重连恢复原截止，摘要不重放。示例[四人](../guides/config-cookbook.md#combat-rules)post-consumption-a.json)/[八人](../guides/config-cookbook.md#combat-rules)post-consumption-b.json)组合消费事实、forward追击与不同冷却参数；原追击60%/当前冷却30%系数只是局部执行绑定，完整原功法阶段、动态威力、原始在途朝向及所有投放仍开放。

## 接触状态反应

机制105的`combat_rules.contact_reactions`按实际状态定义声明有界反应，适用于“已有有害状态被直接命中后消费，并由原施加者追加伤害”。最多16个定义，示例：

```json
{
  "compound-lock": {
    "when": "hp_damage",
    "attacker": "any_hostile",
    "fixed": 0,
    "source_power_bps": 2500,
    "damage_kind": "true_damage",
    "ignore_armor": false,
    "armor_penetration_cap_bps": 5000
  }
}
```

when为direct_contact或hp_damage：前者允许全盾/零HP的合格接触，后者要求主攻击实际HP损失>0。attacker为source_only或any_hostile，后者允许其它合法敌对攻击者引发，附加伤害仍归状态原来源。fixed为0～1000000，source_power_bps为0～100000，两项不可都0；聚合破甲帽0～10000，元素伤害必须引用已登记element，真伤禁止element，显式null拒绝。

载体须source_scoped、refresh/ignore、无叠层的有限有害数值/防御/复合/控制状态，且登记harmful政策；禁止盾、周期、免疫和窗口/净化载体，也禁止攻击contact_status同时绑定同一载体。先按目标profile解析控制，反应只匹配真正存在的状态定义；替代成另一数值定义不会隐式继承原冻结反应。

顺序为资格/免伤/闪避→攻击规则原消费增强→捕获已有反应实例→主伤害及其damage事实/触发→已有状态反应→命中附带状态→direct_hit→攻击消费联动。候选按到期、施加时间、实例ID稳定排序，本次新投放的状态不会反过来被同一命中消费。执行时重验原实例/来源/持有者/期限、原来源可行动及对目标敌对关系、目标资格/免伤；主伤害导致死亡或先移除状态时不释放。实际攻击者在主伤害后死亡，不改变另一有效原施加者已获得的反应归属。

原来源有效威力在每次实际反应执行时live采样，复用record_power：有效ATK、有效skill_power_bonus及profile.power_points_bps沿已有取整和u32钳制派生，随后`raw=fixed+floor(power×source_power_bps/10000)`宽整数后钳u32；普通威力/连携不再次进入预算。零raw不消费、不排队。原功法的完整永久威力派生、其它采样时点与原始投放仍需相应工作包验收，不能把此live契约当作全部原内容默认值。

成功时整份消费该单实例，先清理原控制/数值及普攻工作时钟，再执行原来源的独立派生伤害。伤害必中、不暴击、不吸血，不产生新触发/叠印、盾或窗口记录；按原伤害类型的适用管线处理护甲/元素抗性/减伤/盾/HP及致死保护。真伤沿既有规则绕过防御与减伤。原主伤害、费用及冷却保持。

max_contact_reactions_per_hit为1～4、缺省1；max_contact_reactions_per_step为1～128、缺省32，跨所有来源/目标共用。每成功消费占一次，两种预算限制实际执行前缀；未消费的状态保留，后续新的真实接触可重试，不存在延迟补发。合法状态转移保留原来源/身份/绝对期限，反应只在新持有者上执行；驱散、自然到期、来源退出仅清理，不释放。

私有`contact_reaction_resolved:{instance_id,definition_id,source,target,provoker,raw_damage,hp_damage,absorbed}`只交原状态合法来源/持有者观察者。provoker是本次实际攻击者，source是原施加者；第三方不会因引发反应获得状态私有明细。公开伤害事实仍承载实际结果；快照沿旧状态身份恢复，已消费摘要不重放，也没有新增客户端计时或伤害计算。两份[四人配置](../guides/config-cookbook.md#combat-rules)reaction-a.json)/[八人配置](../guides/config-cookbook.md#combat-rules)reaction-b.json)用不同模式、比例及profile点数验证25/67预算。

## 生命阈值终结与配置组替代

机制105提供触发效果`finish_other`，作用于事实的敌对other，按目标combat profile选择终结尝试或有限替代伤害。它复用原条件/消费、计数/目标范围、动作去重、ICD、概率和每步触发预算；允许direct_hit、contact_status_consumed、damage_dealt、damage_taken、shield_absorbed，禁止周期、kill、技能/友方提交和action_total。

```json
{
  "kind": "finish_other",
  "rule": {
    "hp_threshold_bps": 5000,
    "comparison": "below",
    "default": {"mode": "execute"},
    "profile_overrides": {
      "resistant-profile": {
        "mode": "power_damage",
        "fixed": 0,
        "power_bps": 10000,
        "cap_power_bps": 10000
      }
    }
  }
}
```

hp_threshold_bps为1～10000，comparison为below或at_most。以条件消费后的有效最大HP和钳制后的当前HP作宽整数交叉比较，分别要求`HP×10000 < maxHP×threshold`或`≤`；不先取整生命百分比。最多64个已登记目标profile覆写，省略profile_overrides时按default。表中是配置组ID，Rust不识别Boss或内容名称。

execute策略只有mode字段；其原始真伤预算为当前HP加全部未到期盾余额，预算和大于u32::MAX时预检拒绝，不作部分终结。power_damage必须声明fixed（0～1000000）、power_bps（0～100000）、cap_power_bps（1～100000），前两项不能都0。预算为`min(fixed+floor(power×power_bps/10000),floor(power×cap_power_bps/10000),u32::MAX)`，零raw拒绝。power沿既有record_power的有效ATK、skill_power_bonus和profile点数派生，在条件消费后的预览/实际状态中采样，不按目标最大生命无界放大。

预检发生在条件消费、ICD和概率之前，验证双方代次/资格与敌对关系、保护/归位/伤害免疫、最终最大HP/当前HP/盾余额/来源威力。失败保留条件状态与ICD/概率；动作计数/去重仍沿原机制，已完成的主攻击和费用不回滚。正式消费后先协调有效上限，再按同一算法物化一次计划，部分/整份状态消费及目标最大生命或盾变化不会使用旧值。

两种模式均从原来源进入统一真伤管线，消耗盾并尊重每实体代致死保护，不直接写HP=0。真伤按原规则绕过护甲/抗性/减伤，不闪避或暴击，不吸血/附带状态，也不生成普通伤害/命中/击杀触发或记录；真实致死仅可由下述显式finisher_kill选择后置叶子。终结尝试可能因致死保护而存活；有限替代伤害也可以正常击杀低血目标。原死亡/击杀归属、掉落与清理保持，终结专用奖励/连携及原功法完整阶段不由此隐式启用。

私有`finisher_resolved:{trigger_id,source,target,mode,raw_damage,hp_damage,absorbed,lethal}`向来源/持有者合法观察者发布；NPC来源针对成员时仅该成员接收，不向旁观者泄漏原始HP+盾预算。mode为execute或power_damage，lethal按实际结算后生命判定。Node/H5采用原权威伤害/死亡与快照，摘要只用于显示；重连不再执行、重放或奖励，没有新增延迟任务。[四人示例](../guides/config-cookbook.md#combat-rules)finisher-a.json)/[八人示例](../guides/config-cookbook.md#combat-rules)finisher-b.json)分别验证50%/25%阈值、100%/150%威力替代、普通死亡及保命留1HP；原系数仅局部绑定，不代替原永久威力、阶段及全部投放。


## 终结成功后的有界联动

机制105支持`when:"finisher_kill"`，必须配置`finisher:{trigger_id:"threshold-finish",mode:"execute"}`；mode可选execute、power_damage、any。提供者必须是同一目录中已声明的finish_other规则，且双方profiles至少有交集。只有该提供者的伤害真正使目标HP归0才产生结果，致死保护或非致死替代不产生，普通主伤害击杀不冒充终结。power_damage杀死目标也不匹配execute，须显式选择power_damage或any。其它事实禁止finisher字段。

后置规则只允许heal_from_damage、restore_spirit、apply_self_status、reduce_skill_cooldowns与chain_damage。禁止周期、action_total、target计数范围、other生命阈值和other状态条件；可以使用本人生命/状态条件、contact或action计数、ICD、概率及原预算。禁止再次发起终结、反伤、目标状态投放、驱散或转移。派生追击仍走既有有限链，不产生新奖励、终结、命中触发或窗口/盾记录。

原主事实的全部规则按优先级/ID执行完后，同步派发局部成功结果；结果按生成顺序，每个结果的规则按优先级/ID执行。保存完整来源/目标代次、提供者ID、终结mode、实际HP损失和原创建者的动作身份；amount仅含HP损失，不含护盾。刚死亡的同代目标可作追击原点，来源死亡/退场或代次失效则丢弃。后置规则共享原触发次数/链命中预算；单条失败不撤销主击杀或其它叶子，也不在下一tick补发。结果不写入World队列、快照或持久化，重连不重奖。

原有restore_spirit新增可选max_spirit_bps，0～10000，缺省0并省略序列化；amount保留且必填，0～1000000，固定量和比例不能都0。请求量为`amount + floor(当前有效maxSpirit × max_spirit_bps / 10000)`，宽整数后饱和到u32，最后沿原缺口与成员回灵封锁限制实际增量。在叶子执行时采样，前序自身状态可以提高这一时刻的上限；若本叶子消费该状态则使用消费后的上限。满资源、封锁或比例取整为0时遵循原固定回灵契约：收益为0，仍可以完成触发并支出本条条件/ICD。此字段也适用于原有合法的回灵事实，成员与NPC均使用自己的有效上限。

```json
{
  "id": "finish-spirit", "profiles": ["member"],
  "when": "finisher_kill",
  "finisher": {"trigger_id": "threshold-finish", "mode": "execute"},
  "include_periodic": false, "every": 1, "cooldown_ms": 0, "priority": 1,
  "effect": {"kind": "restore_spirit", "amount": 0, "max_spirit_bps": 800}
}
```

Node/H5继续读取finisher_resolved、combat_trigger_fired、原状态/资源快照、skill_cooldowns_reduced与chain_damage_resolved；无第二套权威状态或新增结果协议。私有明细沿原权限路由。[配置A](../guides/config-cookbook.md#combat-rules)followup-a.json)与[配置B](../guides/config-cookbook.md#combat-rules)followup-b.json)验证四人/八人：保命/非致死无奖励→真实终结→自身临时复合属性、按当前灵力上限回灵、当前冷却返还及一次追击→重连恢复→提前普通攻击击杀不重奖→状态自然到期→每成员一次结算。两样例的追击基数显式为终结实际HP损失，不代表原技能功法威力；原30%/8%/80%/60%只绑定显式输入下的局部系数算例，完整功法威力、阶段、独尊强化消费及内容投放继续开放。


## 按动作消费增强与终结防自刷新

机制105的contact_status可显式配置`sampling:"action"`（缺省contact并省略）。仅owner状态适用；实际第一份通过资格/免伤/保护/闪避门禁的原生接触，冻结本人来源定义的消费层数，缺少状态时冻结0。同动作后续多目标、重复命中、区域跳与引导子弹按冻结层数应用增伤/破甲，不再消费；新授予、刷新或转移不改变旧动作结果。纯消费允许0增伤/0破甲。contact缺省仍逐接触实时取样，原contact_status_consumed只在真正消费时投放一次，后续复用增强不冒充新消费。

原动作实体种类/ID/代次、动作ID及状态定义构成键，活跃计划保留快照，源退场/换代和计划排空清理；重连保留原生计划的快照。无动作身份不消费。max_contact_action_snapshots为1～1024（默认256，默认省略）；预算满后，已有快照继续，未取样的动作不消费且其依赖增强分类的终结拒绝，不把未知当成未增强。保护期持续至全部原生活跃计划排空，不缓存待发奖励或补发旧命中。缓存是有界结算状态，没有第二个动作/周期时钟。

```json
{
  "subject":"owner", "definition_id":"next-boost", "sampling":"action",
  "at_least":1, "consume_max":1,
  "damage_bonus_bps_per_stack":0, "armor_penetration_bps_per_stack":0
}
```

finish_other.rule可配置`consumed_status_bonus:{definition_id:"next-boost",hp_threshold_bps_per_stack:500}`。仅direct_hit准入，须有对应owner/action消费器；倍率1～10000。阈值为`min(10000, hp_threshold_bps + 冻结消费层数 × hp_threshold_bps_per_stack)`，宽整数交叉乘法及below/at_most原语义保持。只有相同定义的接触上下文可增强，未知分类拒绝，普通无对应上下文使用基础阈值。实际主伤害、护盾、消费后的有效属性/来源威力、限额替代及致死保护沿原路径。状态在主命中前消费，即使之后未达终结阈值、主伤害先击杀或保命挡住，也不退回增益；新命中附带状态不参加已经冻结的增强。

成功结果的finisher选择器可配置`enhanced:false`或true，必须引用带上述增强的finish_other；null拒绝。判定来自实际结算上下文，不从当前状态反推。缺省保持原core-24语义。典型组合为enhanced=false、counting=action授予下一次增益；同次未增强施法后续目标保持0快照并去重授予，增强施法的全部成功结果保持true，即使多目标击杀也不自刷新。其它按动作回灵/冷却等叶子继续沿既有预算、模式/提供者筛选、条件和私有路由。

[配置A](../guides/config-cookbook.md#combat-rules)enhancement-a.json)与[配置B](../guides/config-cookbook.md#combat-rules)enhancement-b.json)演示四人/八人：未增强终结获临时复合增益并重连→贯穿动作只消费一次，用5500/6000阈值击杀两目标→命中第一个后在途重连、第二个仍增强→增益不自刷新，回灵/冷却按动作一次→另一独立未增强施法再次获增益→原期限到期及一次结算。主弹道在提交时冻结450/460预算，消费后ATK降低不会重算在途主伤害；终结的威力替代仍按原消费后实时有效威力。客户端继续使用状态消费、终结、冷却与原快照，不新增权威计算或DTO。原5%/25%/30秒只是局部原生算例，完整功法威力、全属性派生、独尊阶段与所有投放另行验收。

周期载体可附带 `modifiers`，与 DOT/HOT 共用同一实例、来源、期限和槽位。数值会参加原属性管线；消耗、驱散或首次免控消耗会移除整个实例。非叠层 HOT 可附带 `control_immunity`、`damage_immunity`、`first_control_immunity`，仍走原生免疫门禁；不接受周期敌对控制或带免疫的叠层周期组合。

叠层周期显式配置 `duplicate_policy:"stack"`、`source_scoped:true`、`max_stacks:1..32` 和 `periodic_stack:{tick_policy:"preserve"|"restart"}`。`preserve` 延用下一跳，连续加层不推迟它；`restart` 从刷新时刻重新等待一个间隔。刷新更新本轮输入快照、期限及层数，并重置本轮跳数；部分消费保留时钟和输入，转移保留身份、来源、层数和时钟。冻结取值转移后仍使用原输入，实时取值按声明读取当前来源或新持有者。

周期比例在乘入当前层数后取整，例如 ATK99、700bps、2层得到13。快照 `periodic.per_stack:true` 表示 `raw_*` 是未乘层数的基础展示量，不能据此由客户端计算实际结算；实际 `periodic_*_tick` 会给出本次层数与最终原始预算。输入属性快照不下发。`first_tick_at_ms` 是本轮首跳锚：`next = first + ticks_done × interval`，总跳数为到期前的严格数量。缩短后的期限可能早于首跳，此时总跳数0、下一跳null，不补发收益。普通非叠层配置省略新增字段。

复合 `modifiers` 支持真正的 `movement_haste`（1..10000bps）和逐层 `movement_slow`。同类沿原移动规则取最强，叠层后减速保底1bps；加速封顶10000bps。它们不改敏捷或最大生命。旧内容按文字解释的裁定见[六项映射记录](../decisions/SDK-5-88-extra-movement-status-mapping.md)，功法条件与投放仍须独立绑定。

可运行样例：[四人成员的实时取值](../guides/config-cookbook.md#combat-rules)composed-a.json)与[八人成员的冻结取值](../guides/config-cookbook.md#combat-rules)composed-b.json)，目录别名为 `composed-four`、`composed-eight`。它们使用同一 Rust 构建，演示 NPC 加层、复合 HOT、持续伤害转移、重连与到期净化。拥有属性增益的 HOT 和普通状态都由同一个属性管线计算；“每层基数”只用于展示，实际每跳结果以服务端事件为准。

SDK 0.99.0 候选补充：严格单位提交、可信阶段上下界、双方状态原子组合、消费前资源倍率和只延长既有数值状态期限见 [conditional-activation-v1](conditional-activation-v1.md)。跨引用/模型范围及可达倍率由Rust冷装载最终核验。

SDK 0.100.0 候选：显式计数器支持可选zero_policy=keep/remove，缺省keep；remove从正层初始化，耗尽以Consumed移除。初次技能按实际已付灵力返还及耗尽后的有限结束状态见 [paid-resource-effects-v1](paid-resource-effects-v1.md)。

0.101.0 增加可信的 [最终施法拦截](cast-interception-v1.md)：在初次技能的最终提交点检查单次标记、控制免疫和目标战斗 profile，私有结果沿原发布通道传递。

## NPC 移动区域进入

SDK 0.119.0 在协议 6 的机制 105 提供 `combat_rules.npc_enter_areas`。键对应 NPC 的 `combat.projectile.id`。该技能使用原选敌、准备、打断、费用、冷却及 `on_hit_target_statuses`。绑定区域时将该技能的 `damage_bps` 设为 0，伤害改由区域配置的 `target_max_hp_bps` 计算。把样例的 `npc_ability`、`npc_enter_areas` 和 `status_definitions` 分别合入 NPC 施法配置、战斗规则和 PVE 状态表。区域占用 `members.area_capacity`，样例每实例预算 32 次，区域容量至多 4 份。 [配置样例](../guides/config-cookbook.md#combat-rules)moving-enter-area.json) · [配置 Schema](combat-rules-v1.schema.json)

`areas` 中的 `moving_enter` 发布圆心、半径、期限、接触预算和 `movement`。后者包含冻结的原点、单位方向、速度、经边界裁剪的路线长度及 `committed`。进入结算发布 `area_entered`，有效击退发布 `forced_displacement`，包含实际起点、终点和滑移倍率。伤害与状态沿用原事件。区域拥有自己的进入和再进入记录，持续重叠不会产生周期伤害。 [协议 Schema](publications-v6.schema.json)

SDK 0.120.0 起，准备中的区域随来源失效而回收。已释放区域配置 `end_on_source_invalid: false` 时，继续按原来源代次结算比例伤害和击退。可闪避的区域在释放时记录来源命中基线，失效后仍使用目标当前闪避。反击和需要两端活体身份的触发走原活体链。 [验收标准](../batches/SDK-5-88-detached-enter-area.md)

`retained_hit_statuses` 可声明来源隔离的简单定时减速，与技能的 `on_hit_target_statuses` 合计最多两项，ID 各自唯一。保留减速按共享规则施加，并随到期、净化或目标退场移除。本人状态快照用 `retained_source: true` 标记原代次。Rust 直接构造 `AreaPlanViewV4` 或 `StatusView` 时，为普通实例设置 `movement: None` 或 `retained_source: None`。 [协议 Schema](publications-v6.schema.json)

## NPC 固定地面领域

SDK 0.121.0、机制 105、协议 6 的 `combat_rules.npc_ground_fields` 按 NPC 技能 ID 配置。把样例的 `npc_ability` 合入 NPC 施法配置，`status_definitions` 合入 PvE 状态表，其余三个表合入战斗规则。技能的 `damage_bps` 为 0，伤害由领域的 `target_max_hp_bps` 决定。领域与其他技能共享 `members.area_capacity`，单个技能只能绑定一种区域机制。 [配置样例](../guides/config-cookbook.md#combat-rules)npc-ground-field.json) · [配置 Schema](combat-rules-v1.schema.json)

火海样例引用 `fire` 元素。宿主在 `combat_rules.elements` 注册该元素，并按本项目配表决定元素倍率。

`duration_ms` 为 `interval_ms` 的整数倍，间隔至少为一个 World 固定步。第一跳在激活后一个间隔，末跳在截止时刻。每跳按成员当前几何、目标状态和命中预算独立结算。原 `on_hit_target_statuses` 在有效来源下对每个目标首次入场尝试一次，状态沿用原有来源和清理规则。

`areas` 新增 `npc_ground_field`，客户端可通过 `logical_ms < activates_at_ms` 显示预警。该类型包含固定圆心、下一跳时刻、计数和接触历史，增量与同 World 重连使用同一契约。Rust 穷举 `AreaPlanKind` 的消费方需处理新分支，严格 Node/H5 消费方使用随版本更新的解码器。 [协议 Schema](publications-v6.schema.json)

## NPC 固定落点单次打击

SDK 0.122.0、机制 105、协议 6 的 `combat_rules.npc_ground_strikes` 按 NPC 技能 ID 配置 `radius`、`target_max_hp_bps` 和 `hit_status_chance_bps`。样例的 `npc_ability` 合入 NPC 施法配置，`status_definitions` 合入 PvE 状态表。技能的 `damage_bps` 为 0。单次打击和固定领域共享 `members.area_capacity`，两类绑定合计最多 16 个技能，单个技能只绑定一种区域机制。 [配置样例](../guides/config-cookbook.md#combat-rules)npc-ground-strike.json) · [配置 Schema](combat-rules-v1.schema.json)

提交时取固定落点内最近的合格成员，等距按成员 ID 排序。选中后沿用伤害免疫和闪避判定，一次打击最多命中一人。全盾吸收仍计成功接触，且两端存活时对 `on_hit_target_statuses` 整组判定一次。概率以基点表示，0 跳过，10000 必定尝试施加。免控与容量由状态链裁决。来源失效时撤销准备中的打击。

`areas` 的 `npc_ground_strike` 仅在预警期存在，`next_tick_at_ms` 与 `expires_at_ms` 均等于 `activates_at_ms`。结算发布一次 `area_tick` 和 `area_ended`，落空的 `targets` 为空数组。客户端复用 `area_started` 和快照绘制。Rust 穷举 `AreaPlanKind` 的消费方需处理新分支，Node/H5 使用更新后的严格解码器。 [协议 Schema](publications-v6.schema.json)

退场成员的冻结快照省略 NPC 区域。留场成员继续按原区域的时钟接收预警、命中和结束。

## 成员地面陷阱

SDK 0.124.0、机制 105、协议 6 支持 `combat_rules.member_ground_traps`。以不带蓄力或引导的 `ground_circle` 技能 ID 为键，配置 `duration_ms` 和 `contact_policy`。技能的 `warning_ms` 是提交后的启动延迟，`radius` 是固定圆半径。样例的技能、伤害规则及状态分别合入技能目录、`combat_rules.skills` 和 PvE 状态表。 [配置样例](../guides/config-cookbook.md#combat-rules)member-ground-trap.json) · [配置 Schema](combat-rules-v1.schema.json)

客户端继续提交 `cast_ground`。陷阱占用 `members.area_capacity`，提交成功时扣一次资源并开始冷却。启动后每个固定步检查进入圆内且无地图遮挡的敌对 NPC，按距离、ID 和代次排序。持续留在圆内只尝试一次，重复进入还须通过该目标代次的冷却和总预算。免疫或闪避保留次数，全盾吸收仍计成功接触。被当步上限跳过的目标需要重新进入才有新机会。

公共 `areas` 新增 `member_ground_trap`。预警用 `logical_ms < activates_at_ms` 判断，触发与剩余次数从 `area_entered` 和 `contact_policy` 读取。陷阱没有周期时钟，`next_tick_at_ms` 为 null，周期计数为 0。同 World 重连保留完整状态，退场和关局走既有区域回收。Rust 穷举 `AreaPlanKind` 的消费方需处理新分支。 [协议 Schema](publications-v6.schema.json)

## 成员施法锚点

SDK 0.125.0、机制 105、协议 6 支持 `combat_rules.member_cast_anchors`。键为零伤害、零提交后预警的普通 `ground_circle` 布置技能，值声明 `duration_ms`、`max_use_distance` 和 `consumer_skill_ids`。使用普通 `cast_ground` 布置。每个成员的同一布置技能保留一个点，替换时复用区域槽，并为新点生成新 ID。 [配置样例](../guides/config-cookbook.md#combat-rules)member-cast-anchor.json) · [配置 Schema](combat-rules-v1.schema.json)

对自己的锚点发送 `cast_at_anchor`，携带 `skill_id` 和 `anchor_id`。普通弹道还需非零 `direction`，圆形区域省略方向。Rust 在接受及提交校验归属、配置距离、视线和足迹。配置的锚点使用距离独立于弹道射程及常规地面施法距离。冷却、费用与伤害归属仍由原成员技能承担。提交生成的区域和弹道保持各自的物化位置与生命周期。

`areas` 发布 `member_cast_anchor`。点的 `radius` 为 0，`center` 是锚点坐标，`expires_at_ms` 是不可使用的起始时刻。重连恢复有效 ID。Rust 穷举 `Action` 或 `AreaPlanKind` 的消费方需处理新分支。Node/H5 使用随版本更新的解码器与动作入口。 [协议 Schema](publications-v6.schema.json)

### 锚点再施放

SDK 0.126.0 在布置技能的锚点配置中增加可选的 `recast`。例如，在样例的 `member_cast_anchors.anchor-place` 内加入以下配置。

```json
{"recast":{"window_ms":1800,"max_count":2,"spirit_cost":4}}
```

`window_ms` 从布置提交时开始，取固定步的正整数倍，至多为锚点寿命与 30000ms 中的较小值。时钟等于窗口截止即关闭。`max_count` 为 1～8，`spirit_cost` 受宿主资源上限约束。

从公共 `areas` 读取 `anchor_recast.action_id`、`expires_at_ms`、`max_count` 和 `used_count`。确认属于自己且可操作时，发送 `recast_anchor`，携带 `anchor_id`、原 `action_id`、`recast_index`（`used_count + 1`）和新 `position`。动作要求仍装备布置技能、未被控制、当前没有互斥动作或后摇。原点校验锚点距离与视线，新点沿用布置技能的范围、视线、阻挡和落点策略。

成功后公共事件 `anchor_relocated` 携带原动作、锚点 ID、来源代次、序号和实际坐标。费用只结算一次。锚点的 ID、寿命和原布置冷却保留，后续施法使用当前中心，已释放效果保留各自的落点。窗口耗尽或截止后仍可在锚点寿命内发射弹道或布置区域。重连以最新完整快照恢复中心与次数。穷举匹配 `EventV4` 的 Rust 消费方需处理 `AnchorRelocated`。 [协议 Schema](publications-v6.schema.json) · [验收标准](../batches/SDK-5-88-member-anchor-recast.md)

### 跟随锚点的领域

SDK 0.127.0 支持在锚点规则中配置 `following_field_skill_ids`。把样例的 `skill` 合入技能目录，将其 ID 加入同一锚点的 `consumer_skill_ids`，再配置 `following_field_skill_ids`。需要迁移锚点时，在同一规则中启用上一节的 `recast`。普通 `cast_ground` 和未启用跟随的 `cast_at_anchor` 沿用固定中心。 [配置样例](../guides/config-cookbook.md#combat-rules)anchor-following-field.json)

已启用的领域通过 `cast_at_anchor` 绑定当前锚点。迁移会同步更新领域中心，包括预警期间，并保留原跳数、命中预算和接触间隔。每跳用当前中心结算。锚点失效的固定步先回收绑定领域，再处理周期命中。锚点到期或替换时领域结束原因为 `anchor_invalid`。穷举匹配 `AreaEndReason` 的 Rust 消费方需处理 `AnchorInvalid`。

协议 6 在领域上发布 `attached_anchor_id`，同时发布该锚点和两者当前中心。客户端通过快照和增量恢复领域位置、阶段和历史。 [协议 Schema](publications-v6.schema.json) · [验收标准](../batches/SDK-5-88-anchor-following-fields.md)

### 返回与主动结束

SDK 0.128.0 可在 `recast` 中选择启用的行为。示例同时启用迁移、返回和结束，三个动作共享窗口、次数和每次费用。省略 `behaviors` 时默认只启用迁移。

```json
{"recast":{"window_ms":1800,"max_count":3,"spirit_cost":4,"behaviors":["relocate","return_to_source","end"]}}
```

`return_anchor` 和 `end_anchor` 携带 `anchor_id`、原 `action_id` 和 `recast_index`。两者沿用再施放的装备、状态、旧锚点距离和视线检查。返回取提交时的权威施法者位置，校验落点后发布 `anchor_relocated`。主动结束发布 `anchor_recast_ended`，随后按 `anchor_invalid` 回收绑定领域，并按 `recast_consumed` 结束锚点。已释放的固定落点效果沿用各自时钟。

`anchor_recast.behaviors` 只在显式配置时发布。增量与重连延续该配置、原动作和已用次数。穷举匹配 `Action`、`EventV4` 或 `AreaEndReason` 的 Rust 消费方需处理新增变体。 [协议 Schema](publications-v6.schema.json) · [验收标准](../batches/SDK-5-88-anchor-return-and-end.md)

## 状态易伤

SDK 0.129.0 的 `combat_rules.status_damage_taken_bps` 以有害数值或复合状态 ID 为键，配置 1～10000 基点，最多 16 项。结算时取目标最高有效易伤，在配置化物理与元素伤害的原取整前乘以对应倍率，护盾随后吸收实际伤害。多个来源各保留状态和期限。真伤沿用既有数值，固定终局与独立资源转移沿用各自路径。 [配置样例](../guides/config-cookbook.md#combat-rules)source-wushi-shu.json) · [配置 Schema](combat-rules-v1.schema.json)

## 受击反制窗口

机制 105、协议 6 支持可选的 `combat_rules.status_retaliations`。键是成员施加给自身的状态定义 ID。载体需要来源隔离，重复策略为刷新或忽略，且由 `status_policies` 标为不可转移的增益。反制和到期状态需要声明为有害，沿用普通状态的控制策略、拦截和容量。两类附加状态均为独立来源、非周期、无后续释放的状态。

| 字段 | 约束 |
|---|---|
| `chance_bps` | 0～10000，触发概率，10000 表示必定尝试施加反制状态。 |
| `source_cooldown_ms` | 至少一个 World 步长，至多 60000 ms。施加或刷新成功后开始计时。 |
| `max_sources` | 1～32。每实例最多保留这么多成功来源。记录满后，已记录来源仍按各自冷却反制。 |
| `reply_status_id` | 反制状态，持有者为来源，实际攻击者为目标。 |
| `dispel_beneficial` | 可选，0～16，默认 0。反制成功后按原驱散顺序移除目标的可驱散增益。 |
| `on_expired_status_id` | 可选。自然结束时施加给窗口内成功反制过的攻击者。 |

同场景最多配置 16 个定义。普通直接命中完成减伤、吸盾、扣血及保命后，若实际扣血或消耗护盾且持有者仍可战斗，才有反制机会。全盾吸收属于有效接触。零伤害且未吸盾、闪避、免伤、保护、DOT 和致死均跳过。窗口截止时已经失去新反制资格，只对已记录且仍符合条件的攻击者处理到期效果。记录按攻击者实体和代次隔离。刷新开启新窗口，忽略重复施加保留原记录。同 World 重连后仍沿用原窗口。自然到期重新校验目标敌对关系和代次，并与其他状态结束效果共用步级预算。宿主和客户端沿用现有状态发布协议，反制记录由 Rust 保存。

[配置样例](../guides/config-cookbook.md#combat-rules)source-xianwang-jiufeng.json) · [配置 Schema](combat-rules-v1.schema.json)

## 按熟练阶段组合友方治疗

协议 6 的 `combat_rules.ally_recovery_by_mastery` 按 `ally_heal` 技能 ID 配置 1～16 行，首行从阶段 0 起，后续行严格递增。技能沿用单一主目标、准备期、预约和后摇，组合表提供该技能的完整治疗配置。宿主未提供可信熟练记录时使用首行。

- `heal` 使用原生恢复公式，由固定量、来源威力比例、目标最大生命比例组成。
- `hot` 引用来源隔离的刷新型 HOT，并声明周期持续时间和每跳占即时原始治疗预算的比例。持续时间须为周期的整数倍，最多 128 跳。提交时冻结原始每跳值，执行时沿用当前受疗和资格。
- `splash` 以主目标的提交位置为圆心，选择半径内有效同局成员。次要目标的即时治疗和周期治疗分别乘以所配置比例，施法者在范围内时可作为次要目标。
- `overheal_shield` 按已有规则将即时治疗的实际溢出授予到同来源盾及共享盾池。处理单个目标时，先结算即时治疗，再尝试施加 HOT，最后处理溢出盾。
- `mark` 引用不可转移的来源隔离增益叠层状态，按 `owner` 选择 `caster` 或 `primary_target`，每次提交给该持有者增加 `stacks` 层并刷新期限。可选的 `at_cap_heal` 在本次达到上限时消费全部层数，对同局仍有效的成员逐个执行恢复公式。范围内其他治疗目标沿用原效果，叠层每次提交只执行一次。

组合 HOT 的发布携带 `periodic.tick_on_expiry: true`。最后一跳与截止时间相同，达到期限的首个 World 步结算该跳后移除。重连沿用原状态实例及剩余跳数。客户端需使用能识别该字段的协议 6 解码器。 [配置样例](../guides/config-cookbook.md#combat-rules)source-ally-recovery.json) · [配置 Schema](combat-rules-v1.schema.json)

`combat_rules.ally_self_target_skills` 可列出最多 64 个允许自选主目标的即时 `ally_heal` 技能。输入继续使用 `cast_unit` 和本人的显式 ID、代次。选本人时，私有 `pending_action` 同时携带 `ally_target` 和 `ally_target_is_self: true`。完整与增量解码器会校验该标记、主目标身份和同一代次。 [自选与叠纹样例](../guides/config-cookbook.md#combat-rules)source-ally-self-and-marks.json)

## 有界成员召唤

在机制 105、schema 7 和发布协议 6 下，宿主可通过 `combat_rules.member_summons` 为现有 `self_status` 技能绑定召唤。字段省略时保持原快照与施法行为。使用 `cast_self` 提交，客户端沿用现有费用、冷却、拒绝和重放协议。

| 字段 | 约束 |
|---|---|
| `max_active_per_member` | 每成员同时存活上限，1～4。 |
| `skills` | 以已配置自身技能 ID 为键，最多 8 项。召唤技能使用即时或固定前摇提交。 |
| `skills.<id>.template_id` | 引用同一配置中的已有 NPC 模板。模板提供实体属性、碰撞半径、近战范围与间隔。 |
| `skills.<id>.count` | 单次创建数量，至少为 1，且不超过成员上限。 |
| `skills.<id>.duration_ms` | 从提交起计算的 World 时间，至少两个固定步，最多 60,000 毫秒。到期时点不参与该步结算。 |

提交时以召唤模板的体积重验施法者当前位置和成员额度。全部单位同时生成。失败走原施法拒绝，保留费用和冷却。召唤体使用固定位置近战，按距离与实体 ID 选择近战范围内路径无遮挡的敌方 NPC。敌方 NPC 可以近战命中，原有敌方直线弹道也会接触并施加已配置的伤害或状态。击败敌方 NPC 归属召唤者。召唤体采用通用攻击结算，不继承模板的主动技能、掉落、死亡保护和角色构筑。

协议 6 在 `monsters[]` 同一实体列表中投影召唤体。其 `entity.kind` 沿用 `monster` 表示非成员实体，`summon.owner` 为完整成员实体引用，`summon.expires_at_ms` 为绝对 World 时点。启用时使用能识别 `summon` 的协议 6 解码器。客户端按该字段判定友方，完整帧和增量使用同一身份。敌方施加在召唤体上的状态只向召唤者投影。连接保护期间暂停交战，保留身份和到期时点。致死、到期、来源终止或换代后，运行时清除实体、关联状态和敌方锁定。

每个召唤绑定可选以下字段。宿主可按内容和成长阶段决定开放哪些字段。

| 字段 | 语义 |
|---|---|
| `inherit_attributes_bps` | 1000～10000。生成时采样召唤者的有效 ATK、DEF 和最大 HP，按比例向下取整，生命至少 1。召唤体拥有自己的生命和状态，采样值随该实体冻结。 |
| `skill_copy.skill_ids` | 1～8 个唯一且有序的授权 ID。支持普通方向弹道、锁定弹道、严格单位伤害和无预警单次圆形伤害。按当前装备、候选顺序、距离和目标 ID 选择，释放时重验。 |
| `skill_copy.shared_interval_ms` | 至少一个固定步且最多 60,000 毫秒。一个来源代次的召唤体共用释放间隔，每只召唤体另有每技能冷却和后摇。 |
| `skill_copy.damage_bps` / `max_circle_targets` | 伤害倍率 1～10000，圆形候选上限 1～8，并取技能自身单跳和总接触上限的更小值。复制伤害基于召唤体自身有效攻击及技能配置的伤害规则。 |
| `death_burst` | 可选 `radius` 0.1～128、`max_targets` 1～8 和 `damage_bps` 1～100000。倍率按生成时冻结的召唤技能威力计算。召唤体被实际致死时，从原位置对有限敌人结算一次。 |
| `owner_while_alive` | 可选 `damage_bonus_bps` 和 `damage_reduction_bps`，各 0～10000，至少一项大于零。结算直接物理或元素伤害时读取本体伤害增益。启用减伤需 `pve.status_rules`，与其他减伤共用原上限。只统计该成员代次自己存活且未到期的召唤。同属性取最高值，最后一具死亡或到期时立即撤掉该增益。 |

复制只释放授权技能的直接伤害。引导、蓄力、重施、陷阱、锚点和召唤技不可进入白名单。成员的费用、冷却、资源回报、装备乘区、命中附带状态和主动技能触发器不随复制转移。只有成功释放后推进共享和独立时钟，并发布 `summon_skill_copied`。弹道走同一容量与墙体判定，来源进入保护或退场时停止在途复制。终局、退出和到期不执行死亡爆破。

