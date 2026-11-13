# 多人成员持续治疗倍率

schema 7 / 机制 69 可在 `pve.status_definitions` 定义受治疗状态。沿用自身技能的 `on_commit_self_statuses` 或恢复道具的 `on_use_self_statuses` 引用定义 ID。定义示例使用 `+50%`，修正值可配置为 −10000～100000 bps。

```json
{
  "id": "renewal-care",
  "mechanism": "timed_attribute_modifier",
  "attribute": "healing_received",
  "modifier_bps": 5000,
  "duration_ms": 3300,
  "duplicate_policy": "refresh",
  "source_scoped": true
}
```

成员自身的 HOT 每跳在 `executed_at_ms` 读取有效倍率。50% 增益下基础量 51 先向下取整为 76，再按气血缺口截断。已有 `periodic_heal_tick` 继续以 `raw_heal` 表示基础量、`hp_restored` 表示实际恢复。宿主读取 Rust 发布值。能力可通过 `kernel-host --sdk-info` 的 `memberPeriodicHealingReceived` 查询。 [范围与验收](../batches/SDK-5-52-member-periodic-healing-received.md)
