# 自动选敌短追击接入

SDK 0.41.0，机制 56，schema 7，协议 6。已有 `basic_pursuit` 的多人 PVE 对局可额外配置自动选敌范围。

```json
{"basic_pursuit":{"max_range":420,"auto_max_range":280,"duration_ms":1800}}
```

`basic.range < auto_max_range <= max_range`。`attack` 继续优先处理射程内的普攻和冷却缓冲。射程外由 Rust 选最近可见的敌方 NPC，等距按实体 ID 排序，经过既有路径与动作资格检查后接管移动。两种追击分别以自动和锁定范围限制每步推进，共用配置的时限。

Node 继续发送 `attack`、`attack_unit` 和 `cancel_action`。本人快照通过原 `self.basic_pursuit` 提供动作 ID、目标和期限。候选选定后保持身份，进入射程后沿原动作进入前摇。客户端可观察既有提交、中断和拒绝事件。

H5 的「自动选择」对应 `attack`，选中具体 NPC 对应 `attack_unit`。例子在 `examples/auto-basic-pursuit-host`，交付路径为 `samples/auto-basic-pursuit`。 [批次范围](../batches/SDK-5-39-auto-basic-pursuit.md)
