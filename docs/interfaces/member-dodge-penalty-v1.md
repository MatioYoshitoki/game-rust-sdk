# 自身技能闪避压制

机制 96 / schema 7 / 协议 6。将定义加入 `pve.status_definitions`，在自身技能的 `on_commit_self_statuses` 引用。

```json
{
  "id": "dodge-focus",
  "mechanism": "timed_attribute_modifier",
  "attribute": "dodge_penalty",
  "modifier_bps": 6000,
  "duration_ms": 1000,
  "duplicate_policy": "refresh",
  "source_scoped": true
}
```

可配置 1～10000 bps，合计封顶 10000。成员直接接触 NPC 时，Rust 先从目标闪避率扣除压制百分点，再将有效闪避率限制为 0%～50%。压制在接触时读取，0% 使用确定命中结果。资格、伤害、暴击、回灵和附带状态继续走对应流程。

宿主通过 `kernel-host --sdk-info` 查询 `memberDodgePenalty`。客户端可观察私有加成状态、弹道和现有 `damage` 事实。Rust 穷尽匹配需包含 `StatusAttribute::DodgePenalty`。 [范围与验收](../batches/SDK-5-79-member-dodge-penalty.md)
