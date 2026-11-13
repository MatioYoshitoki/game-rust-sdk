# 成员直接攻击不可闪避

机制 102 / schema 7 / 协议 6。成员 `basic` 和 `projectile`、`unit_projectile`、`piercing_projectile`、`ground_circle`、`ground_field`、`line_field` 伤害技能支持可选布尔字段 `unavoidable`。

```json
{
  "mechanism": "projectile",
  "id": "certain-strike",
  "unavoidable": true,
  "cost": 12,
  "windup_ms": 150,
  "recovery_ms": 150,
  "cooldown_ms": 1200,
  "damage_bps": 10000,
  "range": 500,
  "speed": 560,
  "radius": 8
}
```

省略与 `false` 沿用原闪避判定。`true` 在成员弹道或区域计划提交物化时冻结，后续接触跳过闪避与该次闪避随机抽取。资格、关系、身份、墙挡、伤害免疫、护盾、命中附带及接触预算继续走通用直接接触入口。穿透和区域重复接触都使用已冻结策略。普攻命中回灵及技能费用、冷却沿用原触发时机。

宿主通过 `kernel-host --sdk-info` 查询 `memberUnavoidable`。客户端观察既有弹道、区域与伤害事实。 [范围与验收](../batches/SDK-5-85-member-unavoidable.md)
