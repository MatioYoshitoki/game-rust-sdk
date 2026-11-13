# 恢复道具附带自身状态接入

SDK 0.50.0、机制 65、schema 7、协议 6。恢复道具可在 `member_item_use` 引用 1～2 个已登记且互不重复的自身状态。状态定义继续放在 `pve.status_definitions`。

```json
{"member_item_use":{"pill_berserk":{"heal_max_hp_bps":2000,"on_use_self_statuses":["item-fury"]}}}
```

客户端沿用 `use_item` 和库存修订。Rust 校验实际恢复和整组状态容量后，一次提交用药。属性均无需恢复时返回 `INVALID_STATE`，状态容量不足时返回 `CAPACITY`。成功后在本人原有用药事件、状态事件和快照读取恢复量、状态及截止时间。重连保留同一局的截止时间。

状态强度与时长由引用定义控制，固定量、比例量、品质倍率和冷却均可独立配置。能力可通过 `kernel-host --sdk-info` 的 `memberItemSelfStatus` 查询。 [范围与验收](../batches/SDK-5-48-member-item-self-status.md)
