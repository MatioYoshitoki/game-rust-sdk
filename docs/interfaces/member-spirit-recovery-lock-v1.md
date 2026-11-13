# 多人灵力回复封锁

机制 73 / schema 7 可在 `pve.status_definitions` 定义以下状态，由成员技能的 `on_commit_self_statuses` 或道具的 `on_use_self_statuses` 引用。

```json
{
  "id": "spirit-seal",
  "mechanism": "timed_attribute_modifier",
  "attribute": "spirit_recovery_lock",
  "modifier_bps": -10000,
  "duration_ms": 4200,
  "duplicate_policy": "refresh",
  "source_scoped": true
}
```

`-10000` 表示完整封锁。只要有一个有效实例，成员自然恢复、普攻命中、光球和即时用药产生的灵力回复都归零。截止时刻恢复资格。光球仍可领取气血回复，双恢复道具在气血确有回复时可以使用。施加封锁的道具先结算本次回复，再添加状态。

协议 6 的本人状态可读取 `spirit_recovery_lock`。能力通过 `kernel-host --sdk-info` 的 `memberSpiritRecoveryLock` 查询。 [范围与验收](../batches/SDK-5-56-member-spirit-recovery-lock.md)
