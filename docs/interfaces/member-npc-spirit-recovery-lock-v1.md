# NPC 弹道施加灵力回复封锁

机制 74 / schema 7 可将已有的 `spirit_recovery_lock` 状态绑定到 NPC 普通弹道的 `on_hit_target_statuses`。在 `pve.status_definitions` 登记状态 ID，然后在 NPC 的 `combat.projectile` 使用以下配置片段。

```json
{
  "on_hit_target_statuses": ["spirit-seal"]
}
```

成功接触后先结算伤害。只要敌对成员仍存活，命中即可施加封锁，包括护盾完全吸收的情况。封锁沿用成员的四条回灵限制，并记录来源 NPC 的实体及代次。来源死亡或归位重置时移除其状态实例。

本人状态快照包含来源及截止时间。可通过 `kernel-host --sdk-info` 的 `memberNpcSpiritRecoveryLock` 查询能力。 [范围与验收](../batches/SDK-5-57-member-npc-spirit-recovery-lock.md)
