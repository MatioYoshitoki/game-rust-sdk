# 恢复道具冷却接入

SDK 0.44.0，机制 59，schema 7，协议 6。恢复道具可追加冷却时长。

```json
{"member_item_use":{"pill_healing":{"heal_hp":40,"cooldown_ms":3000}}}
```

`cooldown_ms` 为 1～60000 的固定步长整数倍。省略时可按原规则即时使用。宿主继续提交 `use_item` 和本人库存修订。

成功后，本人快照的 `item_cooldowns` 给出 `item_id` 和 `ready_at_ms`。客户端以 `logical_ms` 比较截止时间。冷却中返回 `INVALID_STATE`，不消费道具。到期条目从快照消失。不同成员、不同物品分别计时，同局重连沿用截止时间。成员离场时清除其冷却。

样例在 `examples/member-item-cooldown-host`，交付路径为 `samples/member-item-cooldown`。 [批次范围](../batches/SDK-5-42-member-item-cooldown.md)
