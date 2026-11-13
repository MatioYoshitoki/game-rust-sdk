# 自身技能命中加成

机制 98 / schema 7 / 协议 6。将定义加入 `pve.status_definitions`，在自身技能的 `on_commit_self_statuses` 引用。

```json
{
  "id": "accuracy-focus",
  "mechanism": "timed_attribute_modifier",
  "attribute": "accuracy",
  "modifier_bps": 10000,
  "duration_ms": 1000,
  "duplicate_policy": "refresh",
  "source_scoped": true
}
```

可配置 1～10000 bps。同源刷新，有效实例相加后封顶 10000 bps。100 bps 对应原公式中 1 点命中。成员直接接触 NPC 时，Rust 使用 `min(0.5, max(0, dodge × 100 − penaltyBps) / (10000 + accuracyBps))` 计算闪避概率。使用示例配置时，原始闪避率 60%、无闪避压制的目标实际闪避率为 30%。零概率使用确定命中结果，其余概率沿用对局随机流。

宿主通过 `kernel-host --sdk-info` 查询 `memberAccuracy`。客户端可观察施法成员的私有加成状态、弹道和现有 `damage` 事实。Rust 穷尽匹配需包含 `StatusAttribute::Accuracy`。 [范围与验收](../batches/SDK-5-81-member-accuracy.md)
