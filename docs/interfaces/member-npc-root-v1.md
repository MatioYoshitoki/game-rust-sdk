# NPC 弹道定身

机制 88 / schema 7 / 协议 6。将定义加入 `pve.status_definitions`，在 NPC 弹道的 `on_hit_target_statuses` 中引用。

```json
{
  "id": "npc-root",
  "mechanism": "timed_control",
  "attribute": "root",
  "duration_ms": 2800,
  "duplicate_policy": "refresh",
  "source_scoped": true
}
```

定身清空移动和导航，取消尚在移动的普攻追击。持续期间，`move`、`navigate` 和需要追击的普攻返回 `INVALID_STATE`。原地动作按原规则处理。到期或净化后由客户端提交新的移动指令。

命中沿用定时免疫、首控免疫、时长减免。通过 `cleanse_self_statuses` 显式引用定义 ID 即可净化。目标使用私有状态与移除事件展示，Rust 穷尽匹配需加入 `StatusAttribute::Root`。宿主通过 `kernel-host --sdk-info` 查询 `memberNpcRoot`。 [范围与验收](../batches/SDK-5-71-member-npc-root.md)
