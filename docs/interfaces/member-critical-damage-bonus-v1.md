# 自身技能暴伤加成

机制 95 / schema 7 / 协议 6。将定义加入 `pve.status_definitions`，在自身技能的 `on_commit_self_statuses` 引用。

```json
{
  "id": "critical-damage-focus",
  "mechanism": "timed_attribute_modifier",
  "attribute": "critical_damage_bonus",
  "modifier_bps": 7500,
  "duration_ms": 1000,
  "duplicate_policy": "refresh",
  "source_scoped": true
}
```

有效实例相加后封顶 15000 bps。成员直接命中 NPC 成功暴击时，在 `params.critical_multiplier` 上加 `有效 bps / 10000`。配置基础倍率 1.5、加成 7500 bps 时，实际倍率为 2.25。计算护甲减伤后乘暴击倍率，统一取整后处理护盾。

宿主通过 `kernel-host --sdk-info` 查询 `memberCriticalDamageBonus`。客户端观察私有状态与现有 `damage` 事实。Rust 穷尽匹配需包含 `StatusAttribute::CriticalDamageBonus`。 [范围与验收](../batches/SDK-5-78-member-critical-damage-bonus.md)
