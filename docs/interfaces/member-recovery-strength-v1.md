# 多人自然恢复强化

机制 72 / schema 7 可在 `pve.status_definitions` 定义恢复强化，再由成员技能的 `on_commit_self_statuses` 或道具的 `on_use_self_statuses` 引用定义 ID。

```json
{
  "id": "regen-strength",
  "mechanism": "timed_attribute_modifier",
  "attribute": "recovery_strength",
  "modifier_bps": 10000,
  "duration_ms": 3300,
  "duplicate_policy": "refresh",
  "source_scoped": true
}
```

每个自然恢复周期，Rust 按当前距离从 `params.regen` 取近敌或远敌比例，将有效状态的修正相加，倍率下限为零。每项回复按「当前属性上限 × 距离比例 × 恢复倍率」计算，向下取整后按缺口裁剪。示例的 10000 bps 表示在有效期内获得两倍自然回复。

成员的当前气血、灵力和恢复强化状态使用协议 6 的本人视图。能力可通过 `kernel-host --sdk-info` 的 `memberRecoveryStrength` 查询。 [范围与验收](../batches/SDK-5-55-member-recovery-strength.md)
