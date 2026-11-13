# 有界状态结束与延后伤害 v1

适用 schema 7 / mechanism 105 / protocol 6，SDK 0.96.0 候选。全部配置在启动时校验并冻结；Node 只传入输入、持久化和投递结果，H5 不调度状态时钟。

## 状态结束

`combat_rules.status_ends` 以原生状态定义 ID 为键。每项必填 `on`（expired / depleted / consumed / cleansed 的非空、不重复列表）及 `target_statuses`（1～2 个不重复的叶子引用）。载体必须来源隔离。只有真实实例的指定结束原因会执行；部分层数消费不结束实例，转移保留实例、来源、期限及结束政策，死亡、退出、代次失效及关局只清理。

叶子可以是数值、控制、普通 DOT/HOT、非记录护盾；必须来源隔离，不能叠层，不能附带结束派生、记录盾、窗口、净化计划、接触反应或余量转盾，也不能配置周期吸血。装载拒绝自引用、环和非叶子引用。记录盾的 `after_end_target_statuses` 和反击 `on_hit_target_statuses` 也允许这组叶子，仍按原有有益/有害用途约束。

`max_status_end_releases_per_step` 默认 32，范围 1～128。在启用结束计划的配置中，普通结束和记录盾结束共享此尝试预算，记录盾自己的反击/命中预算也继续生效。待处理结束最多 1024 个；超出部分按来源/持有者聚合计数。预算耗尽立即抑制，没有延后重试队列。每次结束先消耗尝试预算，再按真实身份、存活和接入状态校验；失败不会回滚已发生的消耗、破盾或到期。叶子组合的容量检查沿用原生批量状态投放，失败不创建半组叶子。

状态结束和记录盾创建的叶子带核内派生分类；此字段不会被客户端配置或投影。分类在后续 Tick、转移和 Ignore 重复策略下保持；一次真实的非派生 Refresh 改为新施加来源分类。派生 DOT 的后续伤害不能触发普通伤害被动、反射、记录或同类派生，即使击破的是另一面普通盾；盾吸收、减伤、保命、HP 扣除和实际击杀归属继续走原管线。

私有 `status_end_resolved` 包含实例、定义、原来源、当前持有者、真实结束原因、当前仍存在的叶子定义及可选 failure。`active_definition_ids` 表示结算后的活跃叶子，包含 Ignore 保留的既有实例，不表示新创建数量。反馈只发给相关成员。超预算反馈 `status_ends_suppressed {at_ms,count}` 只提供当前成员相关的数量，不泄漏其它成员内容。

## 一次延后伤害

`combat_rules.delayed_damage` 以 DOT 定义 ID 为键。载体必须来源隔离、不可叠层，配置 `duration_ms = tick_interval_ms + step_ms`，因此恰好一个原生周期截止。`periodic_sampling` 必须显式选择 source_snapshot 或 source_live。`status_power_sources` 可选择原来源的技能威力，未选时沿原来源攻击力。目标攻击力采样、另一套 base_power / fixed / cultivation 倍率和命中消费不允许与此公式同时配置。

公式必填 fixed、source_power_bps、target_missing_hp_bps、caps_by_target_profile。固定值 0～100 万，两类倍率 0～10 万，至少一个分量大于零。profile 上限倍率 0～10 万，键必须引用实际 combat profile。计算使用宽整数，一次向下取整：

~~~text
missing = max(0, 当前有效 maxHP - 当前 HP)
raw = floor((fixed * 10000 + 来源威力 * source_power_bps
             + missing * target_missing_hp_bps) / 10000)
存在目标 profile 上限时：raw = min(raw, floor(同一次来源威力 * 上限bps / 10000))
raw 饱和为 u32，随后经过原生伤害管线。
~~~

目标缺失生命始终在执行时读取，独立于来源快照/live；上限下降先归一 HP。伤害类型、减伤、护盾和保命沿 `periodic_damage` 原管线。**原生最低伤害规则仍生效，包括 raw=0 的尝试**；伤害免疫返回零实际伤害。`PeriodicView.raw_damage` 在施加时为 0，执行后表示上一次评估值；它不是提前承诺的最终伤害。

截止为绝对逻辑时刻，正常固定步迟到保留原 scheduled_at_ms；同刻伤害先于 HOT，致死后不复活。提前驱散、载体缩时至截止前、来源/目标失效和关闭都取消该计划；超出载体期限不追赶旧 Tick。H5 连接重建保留同一 World 的实例和截止，进程恢复仍只关闭并补偿财务，不恢复战斗计划。

层数门槛和创建计划可组合既有 CombatTrigger 条件。条件/来源层数/容量/政策预检通过才消费和进入 ICD；失败保留层数和 ICD。随后状态正常结束的失败遵循上面的“主效果已经发生”规则。

## 配置与验收样例

`examples/combat-host/derived-catalog.json` 提供四/八成员两套配置；独立目录清单保留原 catalog 的 32 版本容量上限。`derived-host.json` 配对可信账号数据。两套分别验证快照/live、普通到期 HOT/盾、记录盾到期反击 DOT、非递归、重连期限及来源退出取消。

资料包的派生非递归、护体自然到期与破盾区分，以及 delayed factor 参数表，绑定在 `src/verification/source_derived_vectors.json`。旧延后因子未发现实际静态投放参数，因此此处签收通用契约和声明形式；完整原功法阶段、成长和全部投放仍开放。严格片段 Schema 为 `status-ends-v1.schema.json`；跨引用、一次截止、共享预算和生命周期由 Rust 冷装载和运行时验证。
