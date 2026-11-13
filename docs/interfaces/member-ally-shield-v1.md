# 指定友方吸收护盾

机制 80 / schema 7 可在主技能或附加技能注册 `ally_shield`，沿用 [严格友方单位施法](member-ally-heal-v1.md) 的输入和目标规则。

```json
{
  "mechanism": "ally_shield",
  "id": "ally-guard",
  "cost": 30,
  "windup_ms": 120,
  "recovery_ms": 240,
  "cooldown_ms": 2000,
  "range": 260,
  "shield_status": "ally-barrier"
}
```

在 `pve.status_definitions` 注册 `absorption_shield`，设置 `source_scoped: true`、`duplicate_policy: refresh`，配置 `shield_bps` 和 `duration_ms`。盾量按目标最大气血计算，并受目标共用的 `shield_cap_bps` 裁剪。相同来源刷新补足并保留实例 ID。护盾耗尽或到期按原规则移除，吸收后剩余的伤害才扣气血。

`shield_granted` 和状态生命周期事件发给来源与目标。只有目标的私有快照含实例与盾量，承伤目标收到 `shield_absorbed`。来源或目标退场时分别用 `source_invalid` 或 `target_invalid` 清理对应盾。宿主通过 `kernel-host --sdk-info` 的 `memberAllyShield` 查询能力。 [范围与验收](../batches/SDK-5-63-member-ally-shield.md)
