# 有条件的单位激活、状态组合与资源倍率 v1

适用 SDK 0.99.0 候选，schema7 / mechanism105 / protocol6。配置冻结到对局；阶段、数值和状态引用由接入方初始化，Rust执行资格、消费、金额和生命周期。普通客户端不能提交资格、冻结预算或直接调用这些效果。

## 可信阶段闭区间

SkillRequirement新增可选`maximum_mastery`，与minimum_mastery共同定义闭区间，索引0～127。任一边界存在时均要求规范技能的可信成长记录，包括上界为0；缺记录不能当成零阶段。下界不能大于上界，配置模型须能表示边界行。当前装备候选和规范成长键沿source-gates-v1；成员不借用NPC/profile成长，NPC仍使用自己的既有资格入口。

这是可信初始化的行索引，原熟练经验到阶段的派生仍归R5.1，不由Node或按钮临时推导。

## 严格单位提交事实

`unit_skill_committed`只用于成员真正绑定敌对单位的提交，覆盖单位即时效果、单位弹道和敌对定向引导。它携带该目标的ID/代次、原动作分类及动作身份，不从最近NPC或后续命中猜测。

在提交目标/费用/容量检查通过、费用及本身提交状态处理完成后，原SkillCommitted事实继续使用自身上下文；随后产生新的单位事实，再冻结主效果/引导的威力预算。资源夺取的当前资源预算在这些提交触发之前冻结，以便按消费前层数报价。准备取消或拒绝不产生新事实；自己/方向/地面/友方技能不伪造此事实。新事实仅支持有界状态、恢复、处置等既定效果，不允许用它开启递归伤害链。当前NPC技能没有对应严格单位施法入口，不据此宣称新增该入口。

## 一次消费，两端状态预检

~~~json
{
  "kind":"apply_status_bundle",
  "owner_statuses":["source-power"],
  "other_statuses":["target-drained"]
}
~~~

两个列表均须提供，各0～2个不重复ID，合计至少1个。状态必须source_scoped，且为已有非周期数值/复合/控制定义；自己一端为明确的增益，敌对目标一端为明确的减益，各自登记status_policies。列表引用、政策和极性由Rust冷装载核验。

一个CombatTrigger只消费其条件一次。预检在消费后的状态副本中验证双方真实身份/敌对关系、自己一端政策和整体容量、对方政策/免控/控制替换及整体容量。两端都能执行才消费并开始ICD；失败不会先授予一端，已合法提交的主技能和费用继续执行。控制替换先合并同定义，不能凭两个原控制重复占槽。

状态使用原实例、重复策略、时长与事件；来源均为触发归属者。复用原概率、ICD、动作/目标计数和每步预算。支持单位提交及既有直接/普攻/非周期伤害事实；不加入结束、层数门槛或终结的派生叶子集合。不是任意脚本或回调列表。

## 资源倍率和消费前冻结

~~~json
{
  "hp_current_bps":1000,
  "spirit_current_bps":1000,
  "hp_policy":"direct_transfer",
  "status_bonus":{
    "mode":"multiply_base",
    "qualification":{"skill_id":"drain","minimum_mastery":1,"maximum_mastery":4},
    "subject":"owner","definition_id":"marks",
    "per_stack_bps":1000,"maximum_bonus_bps":3000
  }
}
~~~

status_bonus可选；提供时mode必填。subject选择owner或other上的状态，但始终要求该状态由本次来源本人施加、实例仍有效。qualification可选，使用同一可信技能资格规则；不合格或状态缺失/过期/属于别人时按零加成处理。显式计数器的0层按0，普通单份状态按1层。

加成`b = min(层数 × per_stack_bps, maximum_bonus_bps)`，两个配置值均为1～100000。设当前目标资源为v、基础比例为r：multiply_base使用`floor(v × r × (10000+b) / 100000000)`；add_current_bps使用`floor(v × (r+b) / 10000)`。宽整数合成后只取整一次，装载时验证最高可达倍率不超过当前资源100%。基础HP或SP比例为0时，该资源始终关闭，不因加法模式被开启。

技能在提交触发消费之前冻结预算，触发夺取在本次条件消费之前冻结预算。引导只拆分该冻结总量，后续加层、消费、资格/装备改变不重新报价；每跳仍按原目标、来源、资源扣除/返还、受疗、回灵锁和退出规则处理。配置对象借用使用，不为每个引导跳复制资格/状态字符串。

原“每层使本次偷取+10%”的乘区及HP夺取管线分别待产品裁定。两种模式与两种HP管线可由SDK显式配置，验证样例的选择不代替原内容签收。

## 只刷新已有数值状态的截止

~~~json
{"kind":"refresh_status_duration","subject":"owner","status_id":"marks","duration_ms":10000}
~~~

仅匹配本人来源、指定持有者上仍有效的既有非周期数值状态；必须登记status_policies。duration_ms可选，省略时取定义基准时长；显式值在固定步到本局终止时长/一小时之间。当前有害缩时在消费后的预览中采样，最短1ms。

只把expires_at_ms设为当前时刻加有效时长，保持实例、层数、原applied_at_ms、来源分类、记录及最大属性快照，不把时长加到旧截止上。重复/Ignore/Stack不因此新施加或增层。过期、缺失、封禁或身份失效时不创建、不启ICD、不消费条件。消费后普通Stack已移除则拒绝；显式计数器降到0仍可保持并刷新。

护盾、周期、控制/免疫标记载体及派生门槛/终结叶子不支持此操作。普通击杀可配置在已有状态上续期；来源退场仍完整清理，不能借续期延续失效来源。状态窗口继续使用原账，只有真实的新到期才结算一次，延长期不额外恢复当前资源。

## 接入与证据

无需新增玩家Action、快照属性或财务接口。使用已有action_committed.unit_target、combat_trigger_fired、状态应用/消费/刷新/移除、resource_stolen与unit_channel_pulse。原生full/delta保存结果，重连不重放收益；数值复合状态也拒绝资格/预算/来源等私有上下文注入。

`examples/combat-host/activation-catalog.json`与activation-host.json提供四/八成员样例，包含不同可信阶段、一次三层消费、两端五秒攻防状态、两种显式资源乘区、第二合法跳授予一层、真实击杀续期10秒、重连和退场停止引导。成员数值、配置和目标均来自Rust，Node/H5只发送输入及消费结果。

`src/verification/conditional_activation_vectors.json`绑定原参数行和设计的击杀续期行。原阶段配置尚未编码独立击杀续期因子，证据明确区分设计投影与已编码参数；完整成长、原HP/乘区选择及生产内容仍保留各自原工作包。片段Schema为conditional-activation-v1.schema.json，跨引用、模型范围、实际可达倍率和生命周期由Rust最终核验。
