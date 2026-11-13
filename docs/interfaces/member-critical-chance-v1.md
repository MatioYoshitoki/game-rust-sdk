# 自身技能暴击率

机制 94 / schema 7 / 协议 6。将定义加入 `pve.status_definitions`，在自身技能的 `on_commit_self_statuses` 引用。

```json
{
  "id": "critical-focus",
  "mechanism": "timed_attribute_modifier",
  "attribute": "critical_chance",
  "modifier_bps": 10000,
  "duration_ms": 1000,
  "duplicate_policy": "refresh",
  "source_scoped": true
}
```

暴击率可配置 1～10000 bps，有效实例相加后封顶 10000。成员直接命中 NPC 时，在资格与闪避判定后读取暴击率。成功暴击使用 `params.critical_multiplier`，在护甲减伤后乘算，统一取整后进入护盾和扣血。0% 和 100% 使用确定结果，其余概率由 Rust 对局随机流裁决。

宿主通过 `kernel-host --sdk-info` 查询 `memberCriticalChance`。客户端可观察私有加成状态和现有 `damage` 事实。Rust 穷尽匹配需包含 `StatusAttribute::CriticalChance`。 [范围与验收](../batches/SDK-5-77-member-critical-chance.md)
