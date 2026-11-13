# 局内恢复道具接入

SDK 0.43.0，机制 58，schema 7，协议 6。对局可配置最多 16 个已注册的恢复道具。

```json
{"member_item_use":{"pill_healing":{"heal_hp":40}}}
```

`heal_hp` 为 1～1000000 的整数。宿主提交本人背包的最新 `self.inventory_revision`。

```json
{"kind":"use_item","item_id":"pill_healing","expected_revision":2}
```

Rust 使用一份本局净拾取的道具，并按当前生命缺口恢复。本人 `item_used` 事件携带 `action_id`、`item_id`、`hp_restored`。生命和背包修订通过原 full/delta 更新。满血返回 `INVALID_STATE`，未知物品或不足一份返回 `INVALID_TARGET`，错修订返回 `REVISION_CONFLICT`。

重发原请求沿用原结果。结算按扣除使用与主动丢弃后的净拾取量入账。样例在 `examples/member-item-use-host`，交付路径为 `samples/member-item-use`。 [批次范围](../batches/SDK-5-41-member-item-use.md)
