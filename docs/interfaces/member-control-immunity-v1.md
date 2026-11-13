# 定时控制免疫

机制 84 / schema 7 支持在 `pve.status_definitions` 注册定时免疫，通过 `self_status.on_commit_self_statuses` 绑定。期限按 `duration_ms` 配置，来源分组并采用刷新策略。

```json
{
  "id": "control-guard",
  "mechanism": "timed_control",
  "attribute": "control_immunity",
  "duration_ms": 2600,
  "duplicate_policy": "refresh",
  "source_scoped": true
}
```

免疫有效时，NPC 命中的伤害和其它合法效果照常处理，被拒绝的沉默通过目标私有的 `status_rejected` 报告，`reason` 为 `IMMUNE`。已有沉默按原期限结束。免疫到期后，新沉默沿用机制 83 的打断与施法规则。

Rust 穷尽匹配时加入 `StatusAttribute::ControlImmunity` 和 `Error::Immune`。宿主可通过 `kernel-host --sdk-info` 的 `memberControlImmunity` 查询能力。 [范围与验收](../batches/SDK-5-67-member-control-immunity.md)
