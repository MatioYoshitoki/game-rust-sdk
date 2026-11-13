# 恢复道具状态品质配置接入

SDK 0.51.0、机制 66、schema 7、协议 6。不同品质使用独立物品 ID，绑定同一个 `refresh` 攻击或防御状态。`on_use_self_status_overrides` 以状态 ID 为键，值包含强度 `modifier_bps` 和时长 `duration_ms`。

```json
{
  "member_item_use": {
    "pill_berserk_low": {
      "heal_max_hp_bps": 2000,
      "on_use_self_statuses": ["pill-fury"],
      "on_use_self_status_overrides": {"pill-fury": {"modifier_bps": 1500, "duration_ms": 20000}}
    },
    "pill_berserk_high": {
      "heal_max_hp_bps": 2000,
      "on_use_self_statuses": ["pill-fury"],
      "on_use_self_status_overrides": {"pill-fury": {"modifier_bps": 3900, "duration_ms": 35000}}
    }
  }
}
```

`pill-fury` 仍需在 `pve.status_definitions` 注册。未列在覆盖表中的绑定使用原定义。换品质后，状态的 `instance_id` 保持，`modifier_bps`、`applied_at_ms` 和 `expires_at_ms` 按本次成功使用刷新。客户端继续从本人状态事件和快照读取实际结果。 [范围与验收](../batches/SDK-5-49-member-item-status-quality.md)
