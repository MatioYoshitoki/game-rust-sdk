# 多人自身技能即时治疗

机制 77 / schema 7 的 `self_status` 技能可用 `self_heal` 绑定即时自身治疗。沿用 `CastSelf`，在主技能或附加技能中配置。治疗型技能单独绑定 `self_heal`，可选引用已配置为 `refresh` 的吸收盾。

```json
{
  "mechanism": "self_status",
  "id": "self-heal",
  "cost": 150,
  "windup_ms": 120,
  "recovery_ms": 240,
  "cooldown_ms": 5000,
  "self_heal": {
    "max_hp_bps": 3000,
    "overheal_shield": { "status_id": "overflow-shield", "conversion_bps": 5000 }
  }
}
```

固定量用 `"amount": 100` 替换 `max_hp_bps`。按最大气血比例时先取整得到基础量，再乘当前受治疗倍率向下取整。技能提交先扣费用、写入冷却，再按缺口结算。转盾按实际溢出量计算，容量不足只拒绝新增盾。

协议 6 将 `action_committed`、`heal_resolved` 和可选的授盾结果按顺序发布给本人。治疗结果的 `heal_kind` 为 `self_skill`，包含 `raw_heal`、`multiplier_bps`、`adjusted_heal` 和 `hp_restored`。宿主通过 `kernel-host --sdk-info` 的 `memberInstantSelfHeal` 查询能力。 [范围与验收](../batches/SDK-5-60-member-instant-self-heal.md)
