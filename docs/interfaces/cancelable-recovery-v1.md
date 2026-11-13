# 可取消后摇接入

SDK 0.37.0，机制 52，schema 7，协议 6。多人 PVE 的普通方向技能可配置提交后何时开放取消。

```json
{"recovery_ms":2400,"recovery_cancel":{"after_ms":600}}
```

`after_ms` 是固定步的非负整数倍，距后摇结束至少一个固定步。本人快照的 `self.recovery_cancel` 提供来源 `skill_id` 和 `cancel_at_ms`。用 `logical_ms` 与 `self.recovery_until_ms` 判断当前是否在可取消区间。等待撤离完成或处于保护状态时该窗口不开放。

```json
{"kind":"cancel_action"}
{"kind":"cast","skill_id":"next-bolt","direction":[0,1]}
```

第一种结束后摇和本人的待执行跟进。第二种在另一普通方向技能通过当前冷却、资源、位置等准入门禁后，结束后摇并进入该技能准备。成功接管后观察原 `pending_action`，通过 `action_committed` 读取实际费用。原弹道、状态、费用和冷却继续有效。重连后的新连接完成就绪后，权威快照会恢复仍有效的取消窗口。

H5 开放时显示「取消后摇」和另一技能的「接管施法」按钮。样例位于 `examples/cancelable-recovery-host`，交付路径为 `samples/cancelable-recovery`。 [批次范围](../batches/SDK-5-35-cancelable-recovery.md)
