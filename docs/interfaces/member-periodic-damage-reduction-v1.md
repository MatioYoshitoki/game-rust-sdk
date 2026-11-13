# 周期伤害减伤接入

SDK 0.53.0、机制 68、schema 7、协议 6。恢复道具继续在 `on_use_self_statuses` 引用 `damage_reduction` 状态，也可以同时绑定吸收盾。选择机制 68 后，NPC 对成员的物理持续伤害也按该减伤结算。

Rust 每跳依次处理防御、百分比减伤、护盾和生命。减伤和护盾按 `executed_at_ms` 的有效状态参与结算。`periodic_tick` 中的 `absorbed` 为实际消耗的护盾，`hp_damage` 为实际生命损失。保护跳过时这两个值均为 0。`raw_damage` 保留减伤前、过防御前的原始值。

能力可通过 `kernel-host --sdk-info` 的 `memberPeriodicDamageReduction` 查询。 [范围与验收](../batches/SDK-5-51-member-periodic-damage-reduction.md)
