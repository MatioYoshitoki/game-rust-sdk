# NPC 弹道减速

机制 90 / schema 7 / 协议 6。将定义加入 `pve.status_definitions`，在 NPC 弹道的 `on_hit_target_statuses` 中引用。

```json
{
  "id": "npc-slow",
  "mechanism": "timed_attribute_modifier",
  "attribute": "movement_slow",
  "modifier_bps": -2500,
  "duration_ms": 2800,
  "duplicate_policy": "refresh",
  "source_scoped": true
}
```

每个定义可配置 -9999～-1 bps。多个有效实例取最强一项。Rust 在基础速度曲线之后应用减速，再乘地形和适用的施法阶段倍率。方向移动、导航和普攻追击共用同一预算，沿用已有移动意图及动作。

定时免疫、首控免疫和时长减免按已有顺序处理。净化道具可通过 `cleanse_self_statuses` 显式引用定义 ID。目标使用私有状态和权威坐标展示效果。Rust 穷尽匹配需包含 `StatusAttribute::MovementSlow`，宿主可通过 `kernel-host --sdk-info` 查询 `memberNpcMovementSlow`。 [范围与验收](../batches/SDK-5-73-member-npc-movement-slow.md)
