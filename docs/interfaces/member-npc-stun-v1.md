# NPC 弹道眩晕

机制 89 / schema 7 / 协议 6。将定义加入 `pve.status_definitions`，在 NPC 弹道的 `on_hit_target_statuses` 中引用。

```json
{
  "id": "npc-stun",
  "mechanism": "timed_control",
  "attribute": "stun",
  "duration_ms": 2800,
  "duplicate_policy": "refresh",
  "source_scoped": true
}
```

眩晕中断准备、排队动作、追击、引导和撤离，清空移动方向和路径。眩晕有效期间，移动、寻路、主动技能、普攻、主动重定向弹道、开始撤离返回 `INVALID_STATE`。已提交的费用、冷却和在途弹道按原规则运行。到期或净化后可提交新指令。

定时免疫、首控免疫和时长减免沿用已有顺序，`cleanse_self_statuses` 可显式引用定义 ID。使用目标私有状态、移除与打断事件展示。Rust 穷尽匹配需加入 `StatusAttribute::Stun`，宿主通过 `kernel-host --sdk-info` 查询 `memberNpcStun`。 [范围与验收](../batches/SDK-5-72-member-npc-stun.md)
