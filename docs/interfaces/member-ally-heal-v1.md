# 多人指定友方即时治疗

机制 78 / schema 7 的 `ally_heal` 可放在主技能或附加技能中。治疗量用固定 `amount` 或目标最大气血万分比 `max_hp_bps` 配置。

```json
{
  "mechanism": "ally_heal",
  "id": "ally-restore",
  "cost": 30,
  "windup_ms": 120,
  "recovery_ms": 240,
  "cooldown_ms": 2000,
  "range": 260,
  "heal": { "max_hp_bps": 3000 }
}
```

用共享快照 `members[].entity` 中的 `id`、`generation` 提交指令。目标须为同一 World 的另一名成员。

```json
{"kind":"cast_unit","skill_id":"ally-restore","target":{"id":"P01","generation":1}}
```

接受和提交都验证目标就绪、存活、未受保护、距离和视线。重验失败由施法者收到 `action_rejected`，本次提交不发生。成功提交时施法者承担配置的费用和冷却，读取目标受治疗倍率及气血缺口结算一次。

协议 6 的 `heal_resolved` 中 `heal_kind` 为 `ally_skill`，带来源和目标实体身份。施法者和目标各收到一份，其他成员按共享快照更新气血。前摇的友方身份通过 `pending_action.ally_target` 读取。Rust 接入方穷尽匹配 `HealKind` 时加入 `AllySkill`，直接构造 `PlayerActionView` 时填写可选的 `ally_target`。宿主可通过 `kernel-host --sdk-info` 中的 `memberAllyHeal` 查询能力。 [范围与验收](../batches/SDK-5-61-member-ally-heal.md)
