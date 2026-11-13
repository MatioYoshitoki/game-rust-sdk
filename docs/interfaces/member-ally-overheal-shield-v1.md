# 友方即时治疗溢出转护盾

机制 81 / schema 7 的 `ally_heal` 可为即时 `heal` 显式配置 `overheal_shield`。目标继续通过 `cast_unit` 指定。盾定义须为按来源分组、允许刷新的 `absorption_shield`。

```json
{
  "mechanism": "ally_heal",
  "id": "ally-restore",
  "cost": 30,
  "windup_ms": 120,
  "recovery_ms": 240,
  "cooldown_ms": 2000,
  "range": 260,
  "heal": {
    "max_hp_bps": 3000,
    "overheal_shield": { "status_id": "overflow-shield", "conversion_bps": 5000 }
  }
}
```

Rust 先按目标有效受治疗倍率结算并填补气血缺口，再对剩余量乘转换比例并取整。新增盾量受自身实例和目标总量上限约束。实际新增时刷新该施法者的实例，保留已有余额。容量拒绝仅针对新增盾。

施法者和目标先收到 `heal_resolved`，再收到相应的 `shield_granted` 或 `status_rejected`。目标的私有快照含盾实例，双方退场清理仍由 Rust 处理。宿主通过 `kernel-host --sdk-info` 中的 `memberAllyOverhealShield` 查询能力。 [范围与验收](../batches/SDK-5-64-member-ally-overheal-shield.md)
