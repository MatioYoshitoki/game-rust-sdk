# 恢复道具附带直接减伤接入

SDK 0.52.0、机制 67、schema 7、协议 6。`pve.status_definitions` 可定义 `timed_attribute_modifier` 状态，属性为 `damage_reduction`，重复策略为 `refresh`，`modifier_bps` 取 1～5000。恢复道具通过 `on_use_self_statuses` 引用它。不同品质可用 `on_use_self_status_overrides` 配置强度和时长。

```json
{
  "pve": {
    "status_definitions": [{"id":"pill-guard","mechanism":"timed_attribute_modifier","attribute":"damage_reduction","modifier_bps":2500,"duration_ms":15000,"duplicate_policy":"refresh","source_scoped":true}]
  },
  "member_item_use": {"pill_guard":{"heal_max_hp_bps":2000,"on_use_self_statuses":["pill-guard"]}}
}
```

对 NPC 命中成员的直接攻击，防御后的伤害乘以剩余伤害比例，再按已有种子随机源舍入，最后消耗护盾和生命。多个有效减伤状态相加，合计最多 50%。整数结果直接使用，带小数时以小数部分作为向上取整概率。周期伤害按原公式结算。

客户端从本人原有状态和快照读取 `damage_reduction`、强度和到期时间。能力可通过 `kernel-host --sdk-info` 的 `memberItemDamageReduction` 查询。 [范围与验收](../batches/SDK-5-50-member-item-damage-reduction.md)
