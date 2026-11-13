# 自身技能破甲

机制 93 / schema 7 / 协议 6。将定义加入 `pve.status_definitions`，在自身技能的 `on_commit_self_statuses` 引用。

```json
{
  "id": "armor-focus",
  "mechanism": "timed_attribute_modifier",
  "attribute": "armor_penetration",
  "modifier_bps": 5000,
  "duration_ms": 1000,
  "duplicate_policy": "refresh",
  "source_scoped": true
}
```

破甲可配置 1～10000 bps。成员直接命中 NPC 时读取攻击者当前有效实例，按比例减去目标有效 DEF，最低为零。保留小数护甲至原伤害公式取整，再进入护盾与扣血结算。目标原有效 DEF 可供后续其他攻击使用。

宿主通过 `kernel-host --sdk-info` 查询 `memberArmorPenetration`。客户端可使用攻击者私有状态与现有 `damage` 事实。Rust 穷尽匹配需包含 `StatusAttribute::ArmorPenetration`。 [范围与验收](../batches/SDK-5-76-member-armor-penetration.md)
