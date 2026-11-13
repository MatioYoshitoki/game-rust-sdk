# 自身技能移速加成

机制 91 / schema 7 / 协议 6。将定义加入 `pve.status_definitions`，在自身技能的 `on_commit_self_statuses` 引用。

```json
{
  "id": "haste-guard",
  "mechanism": "timed_attribute_modifier",
  "attribute": "movement_haste",
  "modifier_bps": 3000,
  "duration_ms": 3000,
  "duplicate_policy": "refresh",
  "source_scoped": true
}
```

加成可配置 1～10000 bps。Rust 分别选取最强有效加速和减速，按 `1 + (加速 bps + 减速 bps) / 10000` 作用于基础速度。方向移动、导航和普攻追击使用同一预算，并保留地形、施法阶段和硬控制规则。

目标通过私有状态和权威坐标展示。Rust 穷尽匹配需包含 `StatusAttribute::MovementHaste`，宿主通过 `kernel-host --sdk-info` 查询 `memberMovementHaste`。 [范围与验收](../batches/SDK-5-74-member-movement-haste.md)
