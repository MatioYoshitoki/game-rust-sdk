# 灵力恢复道具接入

SDK 0.45.0，机制 60，schema 7，协议 6。通过 `member_item_use` 配置道具的恢复属性。

```json
{"member_item_use":{"pill_ningqi":{"restore_spirit":40,"cooldown_ms":3000},"pill_healing":{"heal_hp":40}}}
```

每个道具配置一个属性，取值为 1～1000000。冷却沿用机制 59 的步长约束。使用时发送 `use_item`，携带物品 ID 和本人当前库存修订。可消费量来自本局净拾取份额。

灵力恢复成功时，本人的 `item_used` 事件包含实际 `spirit_restored` 和 `hp_restored: 0`。显示值以本人快照的 `spirit` 和 `max_spirit` 为准。满灵力或冷却中返回 `INVALID_STATE`，库存修订和冷却截止时间保持原值。

样例位于 `examples/member-item-spirit-host`，交付路径为 `samples/member-item-spirit`。 [范围与验收](../batches/SDK-5-43-member-item-spirit.md)
