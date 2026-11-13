# 道具净化自身沉默

机制 87 / schema 7 / 协议 6。道具先注册到配置的物品目录，再配置本局掉落。以下为 `member_item_use` 的示例。

```json
{
  "cleanse-tonic": {
    "cleanse_self_statuses": ["npc-silence"],
    "cooldown_ms": 6000
  }
}
```

列表可填写 1～2 个已登记的沉默定义 ID，净化目标固定为当前成员。通过 `use_item` 提交物品 ID 和预期库存版本。成功移除所有匹配的有效实例，扣除一件本局拾取物品，推进库存版本并开启配置冷却。没有有效实例时返回 `INVALID_STATE`。

宿主使用目标私有 `status_removed` 的 `reason: "cleansed"` 和 `instance_ids` 更新状态展示，随后收到 `item_used`，其中 `hp_restored` 为 0，省略 `spirit_restored`。Rust 穷尽匹配需包含 `StatusRemovalReason::Cleansed`。通过 `kernel-host --sdk-info` 查询 `memberItemCleanse`。 [范围与验收](../batches/SDK-5-70-member-item-cleanse.md)
