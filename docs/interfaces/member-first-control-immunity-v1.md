# 首控免疫

机制 86 / schema 7 / 协议 6。将以下定义加入 `pve.status_definitions`，在自身技能的 `on_commit_self_statuses` 引用 `first-guard`。

```json
{
  "id": "first-guard",
  "mechanism": "timed_control",
  "attribute": "first_control_immunity",
  "duration_ms": 10000,
  "duplicate_policy": "refresh",
  "source_scoped": true
}
```

NPC 弹道命中时先结算伤害，再按定时免疫、首控免疫、控制时长减免的顺序处理沉默。首控免疫抵挡该次命中全部沉默，消耗一个实例。多个实例依截止时间、施加时间、实例 ID 升序选取。同源刷新保留单次机会。目标收到 `status_rejected`（`IMMUNE`），随后收到对应实例的 `status_removed`（`depleted`）。未使用的实例沿用到期与退场清理。

宿主通过 `kernel-host --sdk-info` 查询 `memberFirstControlImmunity`。Rust 穷尽匹配需包含 `StatusAttribute::FirstControlImmunity`。 [范围与验收](../batches/SDK-5-69-member-first-control-immunity.md)
