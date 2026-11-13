# 自身持续治疗溢出转护盾

机制 76 / schema 7 允许在成员自身的 `periodic_heal` 定义上添加 `overheal_shield`。先注册一个 `refresh` 策略的吸收盾，再引用其 ID。

```json
{
  "id": "overflow-hot",
  "mechanism": "periodic_heal",
  "heal_bps": 1000,
  "tick_interval_ms": 600,
  "duration_ms": 2100,
  "duplicate_policy": "refresh",
  "source_scoped": true,
  "overheal_shield": {
    "status_id": "overflow-shield",
    "conversion_bps": 5000
  }
}
```

每跳先应用当前受治疗倍率并填补气血缺口。示例在满血且基础量为 100 时，申请 50 点盾。只有实际新增才刷新期限，已有余额保留。状态槽不足时，该跳气血回复仍结算，转换盾以容量不足拒绝。

本人 `periodic_heal_tick` 和 `shield_granted` 分别提供气血实回及护盾实授。可通过 `kernel-host --sdk-info` 的 `memberPeriodicOverhealShield` 查询能力。 [范围与验收](../batches/SDK-5-59-member-periodic-overheal-shield.md)
