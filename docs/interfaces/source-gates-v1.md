# 来源动作、资格和按动作配额 v1

适用 SDK 0.97.0 候选、schema 7 / mechanism 105 / protocol 6。新增项均为可选配置；缺省保留原有行为。普通客户端不能声明动作来源或修改成长资格，Node 不计算这些条件和收益。

## 动作筛选

`CombatTrigger.action_filter` 必填 `basic: bool` 与 `skills`。技能选择为封闭联合类型：`{mode:"any"}`、`{mode:"only",ids:[...]}` 或 `{mode:"except",ids:[...]}`。集合最多 64 个已注册、互不重复的技能 ID。only 空集合只在 basic=true 时允许，用于仅普攻；except 空集合表示所有技能。未知来源在配置了筛选时不匹配，不从当前槽位、按钮、伤害类型或技能布尔值猜测。

Rust 从实际提交动作保存来源。普通/贯穿弹道、区域多跳、引导子计划和附带状态保持原技能与动作身份。DamageTaken / ShieldAbsorbed 筛选来袭动作，触发资格仍检查被动归属者。SkillCommitted / AllySkillCommitted 对应实际提交技能。DOT/HOT 使用施加时保留的分类；转移和 Ignore 保持，真实 Refresh 更新为本次施加来源，没有已知来源的重施改为 unknown。状态离场时分类一并回收。

按动作汇总保留最初分类，不能被之后的全局动作覆盖；同一动作出现冲突分类则丢弃该汇总。派生递归阻断继续生效，动作筛选不会让派生、反射或周期冒充新的直接动作。

## 当前装备与成长资格

`CombatTrigger.owner_skill` 和 `ContactReactionRule.source_requirement` 共用：

~~~json
{
  "skill_id": "canonical-growth-skill",
  "equipped_any_of": ["base-variant", "advanced-variant"],
  "minimum_mastery": 1
}
~~~

skill_id 是已知的规范技能/成长记录键。equipped_any_of 可选，提供时为 1～8 个互不重复、已知技能，表示当前至少装备其中一个；未提供时检查 skill_id 本身。这样基础/进阶按钮可以共享一个成长来源，撤下基础按钮不等于丢失规范记录。

minimum_mastery 可选，为可信 PowerGrowth 的零起始 mastery 行索引（0～127）。提供时必须有该规范技能的可信成长记录且索引达到门槛；0 也要求记录存在。配置模型必须能表示该行，缺记录不能猜为 0 或借用其它技能。未提供时只要求当前装备。成员只读可信 Create seat 的 combat_build，不能从通用成员 combat profile 借用成长；NPC 读取其 profile 成长和实际安装的 projectile/heal 能力。

资格实时检查。撤下提供者后，已提交弹道/区域主伤害继续执行，依赖当前装备的被动收益停止；未提交技能和后续引导仍按既有撤销规则处理。接触反应检查原状态来源的装备和成长，不使用第三方引发者；不合格时保留状态，不抢用反应预算。

该索引是 SDK 已声明的初始化输入；原资料中的熟练度经验到阶段/行索引派生仍属于 R5.1。原冰封因子记录阈值 800，而阶段表圆满起点为 801，验收分别记录原值及给定阶段行的行为，不自行发明原比较器或宣称完整成长管线完成。

## 按熟练阶段选择自身状态

SDK 0.103.0 的 `combat_rules.self_statuses_by_mastery` 用已注册的自身技能 ID 索引阶段列表。例如 `{"self-skill":[{"minimum_mastery":2,"statuses":["strong-status"]}]}` 表示可信阶段索引达到 2 时，提交使用 `strong-status`。原 `on_commit_self_statuses` 是显式缺省，缺少可信记录或低于首个门槛时使用。

同一技能最多 16 行，门槛从 0 起并严格递增。取满足条件的最后一行。每行提供 1～2 个已注册且不同的状态，所选列表整体替换缺省。SDK 在预检和提交时校验所选状态的容量，使用原技能 ID、费用与冷却。阶段从同一成员、同一技能的可信 `CombatBuild.powers` 取得，客户端仍提交原有施法输入。可复用配置样例为 `examples/combat-host/mastery-stages.json`，交付包内位于 `samples/combat/mastery-stages.json`。

SDK 0.129.0 的 `combat_rules.unit_hit_statuses_by_mastery` 使用同样的表结构，适用于即时敌对 `unit_effect`。正式提交时读取原施法者的可信阶段，在合法命中后用选中的列表向实际目标施加状态。低于首行或缺可信记录时使用 `on_hit_target_statuses`。伤害免疫、闪避、状态拦截和容量沿用原命中规则。 [配置样例](../guides/config-cookbook.md#combat-rules)source-wushi-shu.json)

协议 6 的 `combat_rules.skill_cooldowns_by_mastery` 按成员技能 ID 配置 1～16 行递增的 `{minimum_mastery, reduction_bps}`，适用于基础冷却大于 0 的技能。提交时选择满足条件的最高阶段，低于首行或缺可信熟练记录时用基础冷却。阶段倍率与当时有效的状态冷却倍率相乘后向下取整，继续沿用已有最小冷却下限。同一技能的提交状态在设置本次截止时间之后施加。 [磐心守一样例](../guides/config-cookbook.md#combat-rules)source-lin-mi.json)

## 按动作尝试上限

`CombatTrigger.maximum_per_action` 缺省 1，范围 1～8。大于 1 仅适用于 counting=action；配额键包含被动归属者、规则、真实来源实体代次和动作，counter_scope=target 再区分目标代次。现有每步尝试预算和 256 个活跃动作缓存上限继续有效，活跃计划期间不淘汰以重领配额。

门槛或当前资格不满足时不消耗配额。合格事实消耗一次尝试；容量或概率失败不会消费状态、进入 ICD 或生成收益，但占用本次动作尝试，不能无限重试。已有 every、概率、ICD、条件消费和目标作用域保持独立。显式多次配额遇到无动作身份时失败关闭；缺省 1 保留旧内部事实入口的兼容行为。

## 消费与验收

不增加公开状态字段或新客户端输入。原 action_committed、combat_trigger_fired、contact_reaction_resolved、周期事件和原生状态/财务结果供 Node/H5 消费。内部 action_origin 不接受为公开状态/输入字段，Rust 与 JS full/delta 都应拒绝污染基线。

`examples/combat-host/source-catalog.json` 与 source-host.json 提供四/八成员配置，分别使用 2/3 次配额；独立清单保留原 32 版本容量上限。覆盖装备/熟练不同的四名成员、规范成长与进阶装备、排除自身两个释放技能、普攻、直接与周期来源、第三方碎裂、字段多跳、换槽后原主伤害、重连、来源退出和一次结算。

`src/verification/source_gate_vectors.json` 绑定原合锋阵排除项/配额、寒环术每施法配额及冰封碎裂倍率、阶段输入契约；完整原始熟练度、功法威力和生产投放继续开放。片段 Schema 为 source-gates-v1.schema.json；完整引用、模型行范围、事实身份和时序仍由 Rust 装载和运行时校验。


SDK0.99.0候选增加可选maximum_mastery，与minimum_mastery组成可信行索引闭区间。任一边界存在时都要求真实规范记录；上界0不等于允许缺记录。下界≤上界且模型能表示边界。详见[条件激活](conditional-activation-v1.md)。

SDK 0.99.0 候选补充：严格单位提交、可信阶段上下界、双方状态原子组合、消费前资源倍率和只延长既有数值状态期限见 [conditional-activation-v1](conditional-activation-v1.md)。跨引用/模型范围及可达倍率由Rust冷装载最终核验。
