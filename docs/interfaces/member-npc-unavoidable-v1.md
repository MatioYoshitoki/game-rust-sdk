# NPC 弹道不可闪避

机制 101 / schema 7 / 协议 6。在 `pve.combat.projectile` 或 `pve.additional_templates.*.combat.projectile` 配置可选布尔字段 `unavoidable`。

```json
{
  "mechanism": "projectile",
  "id": "npc-bolt",
  "unavoidable": true,
  "cost": 10,
  "windup_ms": 300,
  "cooldown_ms": 700,
  "recovery_ms": 100,
  "range": 350,
  "speed": 560,
  "radius": 6,
  "ttl_ms": 1000,
  "damage_bps": 10000
}
```

省略和 `false` 使用原闪避判定。`true` 在弹道释放时冻结为该弹道的接触策略。接触时先检查来源与目标资格、碰撞和伤害免疫，再跳过闪避及其随机抽取，随后按原顺序结算伤害、护盾和命中附带状态。技能费用、冷却和弹道寿命沿用原流程。

宿主通过 `kernel-host --sdk-info` 查询 `memberNpcUnavoidable`。客户端观察成员私有闪避或免疫状态、弹道和既有伤害事实。 [范围与验收](../batches/SDK-5-84-member-npc-unavoidable.md)
