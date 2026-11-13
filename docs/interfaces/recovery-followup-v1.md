# 后摇跟进施法接入

SDK 0.36.0，机制 51，schema 7，协议 6。可在多人 PVE 对局配置根节点声明以下字段。

```json
{"skill_followup":{"window_ms":150}}
```

窗口至少一个固定步、至多 1000 ms，并与 `step_ms` 对齐。前后技能均为普通方向弹道。前一技能成功提交后的末段后摇内，继续发送原 `cast`。Rust 使用下一可执行固定步判定窗口和目标技能冷却。成功接受返回原输入应答，费用在实际提交时才产生。再发一个有效输入会覆盖本人队列。

```json
{"kind":"cast","skill_id":"followup-latest","direction":[0,1]}
```

本人 `self.followup` 在可跟进的后摇阶段提供 `source_skill_id`、`promotes_at_ms` 和 `queued`。空队列为 `null`。排队对象包含 `action_id`、`skill_id`、单位 `direction`、`expires_at_ms`。释放成功后通过原 `pending_action` 观察准备，通过 `action_committed` 记录费用，通过 `action_rejected` 处理释放或提交失败。取消输入使用原 `cancel_action`。保护和重连会清除队列。

H5 可在后摇末段切换技能并点击「排队跟进」，有排队时可替换或点击「取消跟进」。样例目录为 `examples/recovery-followup-host`，交付路径为 `samples/recovery-followup`。 [批次范围](../batches/SDK-5-34-recovery-followup.md)
