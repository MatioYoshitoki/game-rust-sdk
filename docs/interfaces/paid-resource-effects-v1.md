# 实际已付成本返还与耗尽载体 v1

适用 SDK 0.100.0 候选 / schema7 / mechanism105 / protocol6。新增字段仅是可信配置；普通客户端使用原动作和发布协议。

## 已付事实与比例返还

~~~json
{
  "id":"paid-return", "profiles":["member"],
  "when":"skill_cost_committed", "include_periodic":false,
  "every":1, "cooldown_ms":0, "priority":1,
  "action_filter":{"basic":false,"skills":{"mode":"only","ids":["paid-shot"]}},
  "conditions":{"status":{"subject":"owner","definition_id":"charges","at_least":1,"consume":1}},
  "effect":{"kind":"restore_paid_spirit","bps":3000}
}
~~~

skill_cost_committed只由成员技能的初次原子提交产生：完整验证并扣除本次HP/SP、建立冷却及处理既有提交状态/事实后，在建立主效果前投放。amount取实际已扣灵力，与费用预留口径相同。原SkillCommitted零量及自身上下文保持。未提交拒绝/取消、普攻、引导后续跳和再施放不补发初次提交事实；当前NPC没有新增该事实入口。

restore_paid_spirit只允许上述事实，bps为1～10000。请求恢复为floor(实际已付SP×bps/10000)，使用宽整数；不能用预计返还完成准入。实时缺口、有效上限、恢复锁和存活资格处理实际恢复及余量。它是游戏内资源恢复，不修改外部钱包或账本资产。

新事实仅接受该类型效果，禁用周期/action_total/target计数范围/other生命或状态条件。来源动作筛选、可信阶段/当前装备资格、条件层数消费、概率/ICD和动作/步预算复用；每个规则独立，不隐式合并多条配置。仅有本次实际初始扣费能提供事实金额，玩家输入不能报价。通过规则仍按声明消费次数，零费用或被锁的恢复可为0，不延后补发。

私有paid_spirit_restored包括source、trigger_id、可选action_id、paid_spirit、requested、restored、excess，且requested=restored+excess。它沿现有收件者通道发布；重连读取结果，不重放恢复。

## 显式计数器的耗尽策略

~~~json
{
  "id":"charges", "mechanism":"timed_compound_modifier",
  "base_modifiers":[{"attribute":"def","modifier_bps":0}],
  "stack_counter":{"initial":3,"maximum":3,"zero_policy":"remove"},
  "duration_ms":8000, "duplicate_policy":"refresh", "source_scoped":true
}
~~~

zero_policy可省略或取keep/remove。省略/keep延续既有0层载体行为；remove要求initial>=1，在最后一层被消费时以Consumed移除整个原实例。消费事件仍明确remaining_stacks=0；随后没有可刷新/转移的空壳。该策略随实例私有保存，克隆预检认识消费后槽位释放，转移保持原来源，实际移除时清理私有记录。快照仅携带真实层数及既有stack_counter标识，不公开或接受zero_policy。

可通过已有status_ends对Consumed投放最多两个有限状态，例如持续3秒的spirit_recovery_lock。本次已付事实的比例恢复先完成，结束叶子再按既有结束调度边界执行；之后恢复受锁限制。缺省递归阻断、结束预算及生命周期保持。Expired/Cleansed/死亡/退出不冒充Consumed；来源无效不创建迟到封锁。

## 接入与证据

paid-catalog.json / paid-host.json提供同一SDK下四/八成员的两套显式参数；原30%/三次/8秒/3秒由第一套与原生来源断言绑定，另一套用于证明数值可配置。真实Node重复同一序号请求，及H5的施法、重连、耗尽、锁内恢复和到期恢复均读取Rust结果。原境界技所有成长、普通功法清单及生产投放另按原工作包签收。

片段Schema见paid-resource-effects-v1.schema.json；跨引用、生命周期、资格与配置可达范围以Rust冷装载核验为准。
