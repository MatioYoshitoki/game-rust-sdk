# 恢复道具品质倍率接入

SDK 0.49.0、机制 64、schema 7、协议 6。为各品质注册独立物品 ID，然后为每个 ID 配置 `member_item_use`。`quality_multiplier_bps` 取 1～40000 的整数，10000 为 1 倍，省略时采用 1 倍。

```json
{
  "member_item_use": {
    "pill_mana_low": {"heal_max_hp_bps":2000,"restore_max_spirit_bps":2000,"quality_multiplier_bps":7000},
    "pill_mana_high": {"heal_max_hp_bps":2000,"restore_max_spirit_bps":2000,"quality_multiplier_bps":15000}
  }
}
```

这两条配置使用相同的 20% 基础比例，倍率将它们分别调为 14% 和 30%。倍率也适用于固定量。Rust 在所有乘法完成后向下取整一次，再按各属性缺口截断。现有独立或共享冷却均可与倍率配合使用。

宿主沿用 `use_item` 和背包修订。发给本人的 `item_used` 带实际恢复量，库存与结算沿用对应品质的物品 ID。能力可通过 `kernel-host --sdk-info` 的 `memberItemQualityRecovery` 查询。 [范围与验收](../batches/SDK-5-47-member-item-quality-recovery.md)
