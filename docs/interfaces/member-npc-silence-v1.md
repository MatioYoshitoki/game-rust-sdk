# NPC 弹道沉默

机制 83 / schema 7 支持在 `pve.status_definitions` 注册来源分组的定时沉默，并在 NPC 弹道的 `on_hit_target_statuses` 引用。

```json
{
  "id": "npc-silence",
  "mechanism": "timed_control",
  "attribute": "silence",
  "duration_ms": 2800,
  "duplicate_policy": "refresh",
  "source_scoped": true
}
```

施加成功时取消未提交的技能和技能跟进排队，并按 `interrupted` 结束引导。费用、冷却、已发弹道及未被打断的普攻沿用原规则。控制期间主动技能、蓄力更新与释放、弹道再施放返回 `INVALID_STATE`。到期后按原资格恢复。

受影响成员通过私有 `statuses` 及 `status_applied`、`status_refreshed`、`status_removed` 观察实例。未提交技能被打断时收到 `action_interrupted`，引导结束沿用 `channel_ended`。Rust 穷尽匹配 `StatusAttribute` 时加入 `Silence` 分支。宿主通过 `kernel-host --sdk-info` 的 `memberNpcSilence` 查询能力。 [范围与验收](../batches/SDK-5-66-member-npc-silence.md)
