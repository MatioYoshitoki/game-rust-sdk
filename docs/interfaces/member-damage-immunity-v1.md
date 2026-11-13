# 定时伤害免疫

机制 100 / schema 7 / 协议 6。在 `pve.status_definitions` 注册定时免疫，通过 `self_status.on_commit_self_statuses` 绑定。持续时间由配置提供，采用同源刷新策略。

```json
{
  "id": "damage-guard",
  "mechanism": "timed_control",
  "attribute": "damage_immunity",
  "duration_ms": 1300,
  "duplicate_policy": "refresh",
  "source_scoped": true
}
```

NPC 弹道或近战在目标通过资格与保护检查后读取当前免疫状态。免疫有效时结束本次接触，保留 HP 和护盾，跳过闪避、近战暴击及命中附带状态。近战仍使用原攻击间隔。NPC 周期伤害在每跳实际执行时读取免疫状态，免疫期间该跳 `absorbed` 与 `hp_damage` 为 0，周期进度与状态期限继续推进，`outcome` 沿用目标资格结果。到期后恢复原伤害链。

宿主通过 `kernel-host --sdk-info` 查询 `memberDamageImmunity`。客户端观察受保护成员的私有免疫状态、弹道和既有伤害或周期事实。Rust 穷尽匹配需加入 `StatusAttribute::DamageImmunity`。 [范围与验收](../batches/SDK-5-83-member-damage-immunity.md)
