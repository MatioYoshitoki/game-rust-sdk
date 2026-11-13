# 自身技能冷却缩减

机制 92 / schema 7 / 协议 6。将定义加入 `pve.status_definitions`，在自身技能的 `on_commit_self_statuses` 引用。

```json
{
  "id": "cooldown-focus",
  "mechanism": "timed_attribute_modifier",
  "attribute": "cooldown_reduction",
  "modifier_bps": 2500,
  "duration_ms": 900,
  "duplicate_policy": "refresh",
  "source_scoped": true
}
```

有效实例相加，封顶 4000 bps。主动技能提交时读取当前缩减，再施加本次技能附带的自身状态。正冷却先乘剩余比例并向下取整到毫秒，下限为基础冷却与模拟步长的较小值。基础零冷却保持零。登记后以 `player_skills.ready_at_ms` 为准。

Rust 穷尽匹配需包含 `StatusAttribute::CooldownReduction`。宿主通过 `kernel-host --sdk-info` 查询 `memberCooldownReduction`。 [范围与验收](../batches/SDK-5-75-member-cooldown-reduction.md)
