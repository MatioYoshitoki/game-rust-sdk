# 直接攻击忽略护甲

机制 103 / schema 7 / 协议 6。成员 `basic`、六种伤害技能和 NPC `combat.projectile` 支持可选布尔字段 `ignore_armor`。

```json
{
  "mechanism": "projectile",
  "id": "armor-bypass",
  "ignore_armor": true,
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

省略与 `false` 使用原 DEF 减伤。`true` 在弹道或区域计划物化时冻结，实际直接伤害以零护甲项结算。技能 `projectile`、`unit_projectile`、`piercing_projectile`、`ground_circle`、`ground_field`、`line_field` 共用该策略。NPC 字段可配置在基础怪物和附加模板的 `combat.projectile` 中。

结算依次检查资格、保护、免疫与闪避，再计算基础伤害、护甲项、暴击和取整，随后应用受伤减伤、护盾和 HP 扣除及命中附带。穿透及区域重复接触使用已冻结的策略。

宿主通过 `kernel-host --sdk-info` 查询 `directIgnoreArmor`。客户端观察既有弹道、区域与伤害事实。 [范围与验收](../batches/SDK-5-86-direct-ignore-armor.md)
