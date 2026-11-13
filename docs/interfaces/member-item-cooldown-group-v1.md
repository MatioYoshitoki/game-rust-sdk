# 恢复道具共享冷却接入

SDK 0.46.0，机制 61，schema 7，协议 6。在 `member_item_use` 的各条目配置 `cooldown_group`。同组条目使用相同的 `cooldown_ms`。

```json
{"member_item_use":{"pill_healing":{"heal_hp":40,"cooldown_ms":3000,"cooldown_group":"recovery"},"pill_ningqi":{"restore_spirit":40,"cooldown_ms":3000,"cooldown_group":"recovery"}}}
```

成功执行 `use_item` 后，本人 `item_cooldowns` 为同组各个道具发布相同的 `ready_at_ms`。客户端可照常按道具 ID 查找截止时间。冷却中返回 `INVALID_STATE`，库存修订和恢复属性保持原值。队友独立计时。

样例位于 `examples/member-item-cooldown-group-host`，交付路径为 `samples/member-item-cooldown-group`。 [范围与验收](../batches/SDK-5-44-member-item-cooldown-group.md)
