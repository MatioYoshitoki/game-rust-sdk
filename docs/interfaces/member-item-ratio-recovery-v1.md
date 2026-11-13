# 按最大属性比例恢复接入

SDK 0.48.0、机制 63、schema 7、协议 6。`member_item_use` 可以按当前最大气血或最大灵力的万分比配置用药恢复。

```json
{"member_item_use":{"pill_mana":{"heal_max_hp_bps":2000,"restore_max_spirit_bps":2000,"cooldown_ms":5010}}}
```

两个比例字段各接受 1～10000 的整数。`heal_max_hp_bps` 与 `heal_hp` 互斥，`restore_max_spirit_bps` 与 `restore_spirit` 互斥。不同属性可选不同方式。Rust 计算 `floor(最大值 × 万分值 / 10000)` 后按缺口截断。例如最大气血 1003 的 20% 为 200，缺口为 57 时实际恢复 57。

客户端继续提交 `use_item` 与本人库存修订。成功时在本人的 `item_used` 中读取实际 `hp_restored` 和已配置时的 `spirit_restored`。两项实际值均为 0 时返回 `INVALID_STATE`，道具和冷却保持原值。能力可通过 `kernel-host --sdk-info` 的 `memberItemRatioRecovery` 查询。 [范围与验收](../batches/SDK-5-46-member-item-ratio-recovery.md)
