# 固定效果、显式层数计数器与同实例窗口 v1

适用 SDK 0.98.0 候选，schema 7 / mechanism 105 / protocol 6。数值、层数及触发策略从冻结配置初始化，Node/H5 只消费 Rust 的状态与事件。

## 同一实例中的两部分

`StatusDefinition.base_modifiers` 是固定数值，`modifiers` 继续表示每层数值。每个属性先计算 `固定 + 每层 × 当前层数`，再进入原有属性聚合、移动最强效果选择、取整和上限。两份列表各自不能重复属性，允许两份列表出现同一属性，合计最多 8 项。固定列表只能用于 timed_compound_modifier；已有配置省略此字段时保留原行为。

可选 `stack_counter: {initial: 0, maximum: 3}` 明确启用计数器，initial 可为 0，maximum 为 1～128；含 movement_slow 时最多 32。必须 source_scoped，载体只能是无控制/免疫标记的 timed_compound_modifier，重复策略只能 refresh/ignore，不能再配置 max_stacks、Stack、周期或护盾。Refresh 依既有策略更新期限/窗口并重置 initial；Ignore 保持计数和原实例。

公开 StatusView 使用 `stack_counter:true` 标记，配置对象不会进入快照；stacks 表示真实的 0～max_stacks。0 层仍占一个状态槽，保留固定效果、来源、绝对期限与伤害窗口。消费到 0 不移除该显式载体；普通 Stack 消费到 0 沿原规则移除。

## 只调整现有实例

~~~json
{"kind":"add_status_stacks","subject":"owner","status_id":"seed-carrier","amount":1}
~~~

该 CombatTrigger 效果只匹配触发归属者本人来源、指定 owner/other 持有者的活跃计数器，必须登记 status_policies。amount 为 1～128，结果钳制到 maximum。不存在、身份失效、不可行动或被状态政策封禁时失败，沿原预检不消费条件或 ICD；不会创建载体或借用他人来源。

缺省保留实例、原施加分类、applied_at_ms、expires_at_ms 及同实例记录。`refresh_duration:true` 才从本次时刻重建期限，并实时应用原有有害缩时；不重置计数、记录或窗口最大属性快照。到顶仍是合法的零增量尝试，StatusStacked.added_stacks=0，不生成跨越事实。

## 实际跨越门槛

~~~json
{
  "id":"full-seed-cleanse", "profiles":["member"],
  "when":"status_stacks_reached",
  "stack_change":{"definition_id":"seed-carrier","at_least":3},
  "include_periodic":false,"every":1,"cooldown_ms":0,"priority":1,
  "effect":{"kind":"dispel_statuses","subject":"owner","disposition":"harmful","max_count":1}
}
~~~

仅 add_status_stacks 的实际结果 `previous < at_least <= after` 产生此事实。初始创建、Refresh 重置、普通 Stack 和满层后的零增量不产生；消费下降后再次跨越可再次触发。事实使用增加层数的原生动作分类/身份，来源仍是载体的原施加者，amount 为实际增加量。

门槛在父触发列表结束后按捕获顺序执行；每个事实内按 priority/ID 稳定执行。执行前重验原来源、持有者和实例仍有效，条件读取当前状态；共享原有每步预算、概率、ICD、资格和消费预检。已经合法发生的主伤害与加层不因后置失败而回滚，满层后不补发失败的门槛。

仅支持 counting=contact、include_periodic=false，叶子限定 dispel_statuses、restore_spirit 或有限的 apply_self_status。自身状态不得再拥有计数器、结束释放、记录盾、窗口或同类派生；派生身份沿后续周期/转移保持。门槛净化产生的状态结束不再触发新结束链，不能用加层或伤害效果构造递归。

## 窗口、生命周期与消费

计数器可复用既有 status_windows，同一实例记录实际伤害并在配置允许的结束原因结算。普通加层和消费到 0 不清账、不新建计时器，也不触发 on_consumed 结算；驱散、自然到期、来源/持有者退场和关局沿原实例清理。退出可以发布零恢复的清理摘要，不产生恢复收益。合法完整转移保持来源、计数、固定效果、记录和截止，后续调整仍只接受原来源。

伤害先按当前吸血/减伤经过原生结算，再由 damage_dealt 增加层数，不能提前使用刚获得的层数。样例每次实际伤害 100，固定吸血 15% 和每层 3% 的七次恢复为 15、18、21、24、15、18、21；满层只净化一次，消费后重新跨越再净化。同一窗口记录 700，到期恢复 70，上限仍取配置中的最大 HP 20%。

`examples/combat-host/counter-catalog.json` / counter-host.json 提供四人、八人实测配置，分别使用来源表的 12/15 秒。Node 与真实 H5 验证 0 层、两次跨越、消费、成员隔离、重连、到期恢复和退出一次结算。Rust/JS full/delta 拒绝未标记的 0 层、false/null 标记及非法固定字段，失败不污染基线。

片段 Schema 为 status-counters-v1.schema.json。完整引用、列表合计、同属性唯一、计数上下界关系和生命周期仍由 Rust 及解码器验证。原始道种来源绑定见 src/verification/stack_window_vectors.json；完整成长阶段选择、真意和生产投放继续保留原工作包验收。

SDK 0.100.0 候选：显式计数器支持可选zero_policy=keep/remove，缺省keep；remove从正层初始化，耗尽以Consumed移除。初次技能按实际已付灵力返还及耗尽后的有限结束状态见 [paid-resource-effects-v1](paid-resource-effects-v1.md)。
