# 锁定普攻短追击接入

SDK 0.40.0，机制 55，schema 7，协议 6。多人 PVE 对局可声明根配置。

```json
{"basic_pursuit":{"max_range":420,"duration_ms":1800}}
```

追击范围大于 `basic.range`，至多 10000。时长为 1 至 1800 ms 的固定步整数倍。发送原 `attack_unit` 指定 NPC 及代次。目标在实际射程内时沿原普攻逻辑。目标在实际射程外、追击范围内并通过准入时，Rust 沿直线向射程边缘推进。

本人 `self.basic_pursuit` 提供 `action_id`、`target`、`started_at_ms` 和 `expires_at_ms`。截止步结束追击。达到射程后用同一动作 ID 进入普通准备，之后通过实际 `action_committed` 观察发射和冷却。失效由 `action_rejected` 报告。追击及对应前摇期间保留最新手动移动意图，阶段退出后恢复。

`cancel_action` 可主动取消，后台和连接切换会清除旧动作。H5 在「普攻目标」选择 NPC 后点击普攻，可观察追击和取消按钮。样例在 `examples/basic-pursuit-host`，交付路径为 `samples/basic-pursuit`。 [批次范围](../batches/SDK-5-38-basic-pursuit.md)
