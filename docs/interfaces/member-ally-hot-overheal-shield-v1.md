# 友方持续治疗溢出转护盾

机制 82 / schema 7 可在 `ally_heal.hot_status` 引用的 `periodic_heal` 上声明 `overheal_shield`。目标继续通过 `cast_unit` 指定。两个状态均须按来源分组并采用刷新策略。

```json
{
  "id": "ally-renewal",
  "mechanism": "periodic_heal",
  "heal_bps": 1000,
  "tick_interval_ms": 1000,
  "duration_ms": 3900,
  "duplicate_policy": "refresh",
  "source_scoped": true,
  "overheal_shield": { "status_id": "overflow-shield", "conversion_bps": 5000 }
}
```

每跳先读取目标当前受治疗倍率并回复气血，再将溢出按万分比向下取整。转换盾沿用单盾和总量上限，实际增加才刷新期限。容量不足只拒绝盾，保护期间沿用 HOT 跳过规则。

来源和目标按顺序收到 `periodic_heal_tick` 及相应的 `shield_granted` 或 `status_rejected`。目标私有快照包含 HOT 和盾，两者在施法者或目标退场时按实例清理。宿主通过 `kernel-host --sdk-info` 中的 `memberAllyHotOverhealShield` 查询能力。 [范围与验收](../batches/SDK-5-65-member-ally-hot-overheal-shield.md)
