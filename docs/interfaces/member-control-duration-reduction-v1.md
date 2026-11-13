# 控制时长减免

机制 85 / schema 7 / 协议 6。将以下定义加入 `pve.status_definitions`，并在自身技能的 `on_commit_self_statuses` 引用 `duration-guard`。

```json
{
  "id": "duration-guard",
  "mechanism": "timed_attribute_modifier",
  "attribute": "control_duration_reduction",
  "duration_ms": 2200,
  "modifier_bps": 2500,
  "duplicate_policy": "refresh",
  "source_scoped": true
}
```

每个定义可配置 1～4000 bps。目标有效实例相加后封顶 4000 bps。NPC 弹道命中时先判定免疫，随后用当时的减免计算沉默期限。`duration_ms × (10000 − 有效减免) ÷ 10000` 向下取整到毫秒，最短 1ms。施加或刷新后，以权威 `expires_at_ms` 为准。

宿主可通过 `kernel-host --sdk-info` 查询 `memberControlDurationReduction`。Node 客户端使用目标私有状态与到期事件展示，Rust 穷尽匹配需包含 `StatusAttribute::ControlDurationReduction`。 [范围与验收](../batches/SDK-5-68-member-control-duration-reduction.md)
