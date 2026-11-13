# 方向蓄力接入

SDK 0.32.0，机制 47，schema 7，协议 6。技能采用普通或穿透方向弹道，配置 `windup_ms: 0` 和完整的 `charge` 对象。

```json
{"min_ms":450,"max_ms":1500,"min_damage_bps":4000}
```

客户端提交 `{"kind":"start_charge","skill_id":"arc-bolt","direction":[1,0]}`。从本人快照的 `self.pending_action.charge` 读取 `action_id`、`started_at_ms`、`min_release_at_ms` 和 `auto_commit_at_ms`。随后可发送 `{"kind":"release_charge","action_id":"charge-1"}`。输入身份沿原对局、成员、连接和序列门禁。提前释放取消准备，到上限自动提交，手动释放锁定实际持有时长。

Rust 在提交时使用施法者实际位置和开始时锁定的方向。原始伤害按 [CH02](../batches/SDK-5-30-direction-charge.md) 的线性公式缩放。本人 `action_committed.charge` 提供实际 `held_ms` 和 `damage_factor_bps`。普通施法的该字段省略。释放安排在下一固定步提交，期间 `commits_at_ms` 表示已冻结时点。UI 以准备清空和实际提交事件核对动作完成。

配置示例位于 `examples/direction-charge-host`，交付位于 `samples/direction-charge`。四人演示手动释放普通弹道，八人演示自动提交穿透弹道。 [范围与验收](../batches/SDK-5-30-direction-charge.md)
