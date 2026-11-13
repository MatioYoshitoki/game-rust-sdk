# 即时用药的受治疗倍率

schema 7 / 机制 70 支持在 `member_item_use` 的气血或双恢复条目设置 `apply_healing_received`。字段省略时采用原有气血恢复公式。

```json
{
  "member_item_use": {
    "pill_renewal": {
      "heal_hp": 43,
      "quality_multiplier_bps": 12500,
      "apply_healing_received": true,
      "cooldown_ms": 900
    }
  }
}
```

示例先将 43 乘道具品质倍率并向下取整为 53。用药时若已有 50% 受治疗增益，第二次取整得到 79，再按气血缺口截断。倍率来源可由自身技能或恢复道具施加，附带状态从本次用药提交后开始生效。双恢复道具的灵力公式独立，任一项实际回复大于零才消耗一份。

`item_used` 继续发布实际回复。能力可通过 `kernel-host --sdk-info` 的 `memberItemHealingReceived` 查询。 [范围与验收](../batches/SDK-5-53-member-item-healing-received.md)
