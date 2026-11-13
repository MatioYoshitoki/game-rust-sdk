# 局内战利品丢弃接入

SDK 0.42.0，机制 57，schema 7，协议 6。对局可配置 `"member_loot_drop":{"max_active":8}`。玩家丢弃实例配额为 1～50，并纳入原有 World 的地面资源预算。寿命沿用 `params.resource_ttl_ms`。

```json
{"kind":"drop_item","item_id":"xue-zhi","quantity":1,"expected_revision":2}
```

使用本人的最新 `self.inventory_revision`。Rust 只允许扣减本局净拾取份额，在权威位置建立公共资源后同步修改背包。资源可用原 `pickup` 拾取。双方继续收到既有 full/delta、`resource_spawned` 和 `resource_taken`。

错修订返回 `REVISION_CONFLICT`。地面配额耗尽返回 `CAPACITY`。超量、未知物品、无效落点和被阻挡拾取返回 `INVALID_TARGET`。越过拾取距离返回 `OUT_OF_RANGE`。通过连接重连保留已提交状态。

结算复用现有账本。`looted` 为该成员本局拾取扣除已用、主动丢弃和转入遗物后的净额，`carried` 为战前带入，`dropped` 为普通终局损失，`kept` 为结算保留。 [批次范围](../batches/SDK-5-40-member-loot-drop.md) · [堆叠物资流向](settlement-progression-v1.md#堆叠物资流向)
