# 局内双恢复道具接入

SDK 0.47.0、机制 62、schema 7、协议 6。对局配置可在同一条 `member_item_use` 定义中同时提供 `heal_hp` 和 `restore_spirit`。

```json
{"member_item_use":{"pill_mana":{"heal_hp":40,"restore_spirit":40,"cooldown_ms":3000}}}
```

客户端提交 `use_item` 时沿用本人背包修订。Rust 分别按本人当前缺口截断。任一项生效时扣除本局净拾取的一份并启动已有冷却。两项都为零时返回 `INVALID_STATE`。

双恢复成功时，本人的 `item_used` 同时包含实际 `hp_restored` 与 `spirit_restored`，已满的一项为 0。新版样例可查询 `kernel-host --sdk-info` 的 `memberItemDualRecovery`。交付包样例位于 `samples/member-item-dual-recovery`。 [范围与验收](../batches/SDK-5-45-member-item-dual-recovery.md)
