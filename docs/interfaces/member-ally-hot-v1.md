# 指定友方持续治疗

机制 79 / schema 7 的 `ally_heal` 技能可选 `hot_status`。沿用 [严格友方单位施法](member-ally-heal-v1.md) 的目标输入和提交规则。

```json
{
  "mechanism": "ally_heal",
  "id": "ally-renew",
  "cost": 30,
  "windup_ms": 120,
  "recovery_ms": 240,
  "cooldown_ms": 2000,
  "range": 260,
  "hot_status": "ally-renewal"
}
```

在 `pve.status_definitions` 注册同名 `periodic_heal`，配置 `heal_bps`、`duration_ms`、`tick_interval_ms`，并设置 `source_scoped: true`、`duplicate_policy: refresh`。友方持续治疗配置仅用 `hot_status` 绑定效果。目标状态容量在技能提交前检查。

每个来源在同一目标上有独立实例和节拍。同来源刷新沿用实例 ID 并重置节拍。每跳按施加时冻结的目标最大气血比例和执行时有效的受治疗倍率结算。任一方处于保护时该跳记录为 `protected`，实际回复 0。施加后的移动沿用原节拍。

目标的 `self.statuses` 提供实例。来源和目标收到 `status_applied`、`status_refreshed`、`periodic_heal_tick` 和 `status_removed`，可按实例 ID 关联。来源离开对应 `source_invalid`，目标离开对应 `target_invalid`。宿主通过 `kernel-host --sdk-info` 中的 `memberAllyHot` 查询能力。 [范围与验收](../batches/SDK-5-62-member-ally-hot.md)
