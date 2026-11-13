# 自身技能闪避率

机制 97 / schema 7 / 协议 6。将定义加入 `pve.status_definitions`，在自身技能的 `on_commit_self_statuses` 引用。

```json
{
  "id": "dodge-focus",
  "mechanism": "timed_attribute_modifier",
  "attribute": "dodge_chance",
  "modifier_bps": 2500,
  "duration_ms": 1300,
  "duplicate_policy": "refresh",
  "source_scoped": true
}
```

可配置 1～5000 bps。同源刷新，有效实例相加后封顶 5000 bps。NPC 弹道和近战在目标通过资格和保护检查后读取当前闪避率。0% 使用确定命中结果，其余沿用 Rust 对局随机流。闪避结束本次接触，伤害、近战暴击、护盾消耗和附带状态留给成功命中。NPC 近战在每次合法尝试后使用原攻击间隔。

宿主通过 `kernel-host --sdk-info` 查询 `memberDodgeChance`。客户端可观察防守成员的私有加成状态、弹道和现有 `damage` 事实。Rust 穷尽匹配需包含 `StatusAttribute::DodgeChance`。 [范围与验收](../batches/SDK-5-80-member-dodge-chance.md)
