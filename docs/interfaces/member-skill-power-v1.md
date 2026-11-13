# 自身技能威力加成

机制 104 / schema 7 / 协议 6。在 `pve.status_definitions` 注册威力状态，在自身技能 `on_commit_self_statuses` 中引用。

```json
{
  "id": "power-focus",
  "mechanism": "timed_attribute_modifier",
  "attribute": "skill_power_bonus",
  "modifier_bps": 5000,
  "duration_ms": 1300,
  "duplicate_policy": "refresh",
  "source_scoped": true
}
```

`modifier_bps` 为 1～100000，同源刷新，有效实例的加成相加。伤害技能物化预算时，Rust 先计算 `floor(有效 ATK × (10000 + 加成合计) / 10000)`，再乘技能 `damage_bps` 并取整。中间量用 u64 计算并饱和到 u32。普通、单位、穿透弹道及圆形、持续圆形、线形区域共用该预算路径，引导在提交时冻结预算供后续子弹道使用。普攻使用其自身 ATK 预算。

宿主通过 `kernel-host --sdk-info` 查询 `memberSkillPower`。客户端观察施法成员私有状态和既有伤害事实。Rust 穷尽匹配需加入 `StatusAttribute::SkillPowerBonus`。 [范围与验收](../batches/SDK-5-87-member-skill-power.md)
