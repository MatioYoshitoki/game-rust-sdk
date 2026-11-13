# 恢复道具溢出治疗转护盾

机制 75 / schema 7 的气血或双恢复道具可使用 `overheal_shield`。在 `pve.status_definitions` 注册一个 `refresh` 策略的吸收盾，再由道具引用其 ID。

```json
{
  "heal_hp": 200,
  "cooldown_ms": 900,
  "overheal_shield": {
    "status_id": "overflow-shield",
    "conversion_bps": 5000
  }
}
```

气血治疗先应用已配置的品质与受治疗倍率，裁剪气血缺口后取溢出量。示例在满血时用 200 点治疗申请 100 点盾。护盾新增量受状态自身上限和目标总上限裁剪。实际新增时复用同一实例的余额并刷新期限。

本入口使用单一状态转换。宿主从协议 6 的 `item_used` 读取实际气血、灵力回复，从 `shield_granted` 读取申请量和实授量。能力通过 `kernel-host --sdk-info` 的 `memberOverhealShield` 查询。 [范围与验收](../batches/SDK-5-58-member-overheal-shield.md)
