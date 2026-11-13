# 通用指定单位效果与定向引导 v1

SDK 0.92.0，schema 7 / mechanism 105 / protocol 6。配置严格校验后按每局绑定版本执行；普通 Node/H5 只发送已有的 `cast_unit {skill_id,target:{id,generation}}`。所有目标重验、预算、费用、冷却、跳数和状态投放由 Rust 负责。

`unit_effect` 是不生成弹道的敌对单位效果，具有 `id/cost/windup_ms/recovery_ms/cooldown_ms/range/damage_bps`，可选 `unavoidable/ignore_armor/hp_cost/cast_movement/recovery_cancel/on_commit_self_statuses/on_hit_target_statuses`。不接受 `speed/radius/charge`。即时形式在提交点走已有直接接触结算，沿用免疫、命中、闪避、护盾、减伤、保命和触发规则；其它单位不会拦截效果，墙体视线、目标身份/代次、关系、有效状态和距离必须通过。准备阶段失效不扣费、不启动冷却。

定向引导通过同一技能的 `channel` 声明；敌方使用 `damage_distribution:"total"`，友方使用 `ally_heal` 配合 `channel.recovery`。均须明确 `pulse_count`，1～128 跳，`duration_ms=interval_ms*pulse_count`，间隔至少一个固定步且总长不超过30秒；不能蓄力。友方引导不能同时配置即时 heal 或 hot_status。HP/SP恢复复用已有 RecoveryFormula/SpiritFormula，按提交时来源攻击、目标最大属性冻结总量；敌方冻结 configured damage budget。先付提交费用、启动冷却；第一跳至少等一个完整间隔。后续按提交时刻加绝对间隔调度，固定步最多延后不足一步。每跳另付 `spirit_per_pulse`，不足则停止。敌方总量份额前N-1跳 floor(total/N)，末跳取余；恢复可声明 `remainder_policy:"final"`，省略时保留旧恢复机制的 earliest 行为。

例如总量8、3跳得到2、2、4。部分执行或中断只留下已执行前缀，未来预算不补发、不退款。实际伤害和恢复在执行时仍使用目标实时免疫、护盾、接收倍率、恢复锁及资源缺口；不会重新计算冻结的攻击或目标最大属性预算。

`channel.on_success` 最多4个门槛，每项 `{after_pulse,self_statuses?,target_statuses?}`，每组状态引用最多2项且唯一。至少一组非空，门槛为1～pulse_count；引用必须存在，自身为有益效果，目标效果与敌友关系一致且 source_scoped。每个门槛只在该动作第N个合法跳后尝试一次完整投放，层数和时长不拆分。合法跳指来源、目标、距离、视线及资源检查通过；伤害免疫、全盾吸收、满资源和实际零结果仍计数。投放仍受既有免疫、封禁和容量规则约束，失败不重试、不撤销主效果。

新定向计划在本固定步的成员移动、NPC动作、弹道/区域控制之后提交或执行，避免席位顺序使同刻目标移出范围仍受益。自身控制、有效新移动或显式取消沿已有阶段政策处理。失去目标、关系/保护失效、距离/视线失败、来源死亡/退出、技能撤销、关局均停止未来跳。目标在下一跳前失效也会在下一固定步收回计划。重连沿现有安全政策中断活跃动作，保留冷却和已付费用，重新就绪不恢复旧跳；进程恢复仍为财务关闭/补偿。

本人私有 `active_channel.target` 是包含 kind/id/generation 的完整 EntityRef；total_pulses 必须存在。本人收到 `unit_channel_pulse {action_id,target,pulse,spirit_cost,raw_damage,scheduled_at_ms,executed_at_ms}` 或原 `channel_recovery_pulse`；友方实际恢复通过 HealResolved 发给来源及目标。其它成员不能读取私有引导计划和门槛执行记录。完整/增量 Rust 与 JS 消费器校验关系、身份和时钟，非法增量不修改基线。

[严格片段 Schema](unit-effects-v1.schema.json)校验结构，冷装载还校验版本、关系和引用等跨字段约束。样例 units-four/units-eight 演示同一构建的不同总量、三跳、第二跳门槛、实际友方恢复、重连和目标退出。原来源归墟夺元的三跳/400ms/第二合法跳全量状态仅作为阶段投影；资源偷取、完整功法威力及其它培养阶段另有明确开放项。

SDK 0.99.0 候选补充：严格单位提交、可信阶段上下界、双方状态原子组合、消费前资源倍率和只延长既有数值状态期限见 [conditional-activation-v1](conditional-activation-v1.md)。跨引用/模型范围及可达倍率由Rust冷装载最终核验。
