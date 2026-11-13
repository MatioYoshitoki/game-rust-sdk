# 固定方向引导接入

SDK 0.34.0，机制 49，schema 7，协议 6。同 World 1～8 成员 PVE 中，普通方向弹道技能可配置固定时长引导。

```json
{"channel":{"duration_ms":960,"interval_ms":240,"spirit_per_pulse":5}}
```

`channel` 可用于主槽或附加槽。时间按 `step_ms` 对齐，时长至少两个间隔，至多 30000 ms。实际脉冲最多 128 次，每次灵力是 0～1000000 的整数。示例在提交后 240、480、720 ms 发射，960 ms 结束。技能初始费用和冷却在提交时生效，后续费用在该发实际创建时扣除。

```json
{"kind":"cast","skill_id":"arc-bolt","direction":[1,0]}
{"kind":"cancel_action"}
```

本人快照 `self.active_channel` 包含 `action_id`、`skill_id`、单位 `direction`、`started_at_ms`、`ends_at_ms`、`next_pulse_at_ms` 和成功发射数 `pulses`。最后一发后 `next_pulse_at_ms` 为 `null`。引导结束时该对象从权威快照清除。接入方按逻辑时钟显示，按私有事件记录实际发射与费用。

```json
{"kind":"channel_pulse","action_id":"channel-1","pulse":1,"spirit_cost":5,"projectile_id":"shot-1"}
{"kind":"channel_ended","action_id":"channel-1","reason":"completed"}
```

两类事件通过原 `private` 封装发布给本人。结束原因包括 `completed`、`cancelled`、`interrupted`、`source_invalid`、`no_resource`、`capacity` 和 `invalid_origin`。活动期间会缓冲最新移动意图。结束进入配置后摇。主动取消、后台和失联保留已经发射的合法弹道，死亡和离场按成员终局清理。H5 的黄线展示权威方向，普通宿主可沿用现有 JSONL 和协议 6 发布接口。样例目录为 `examples/fixed-channel-host`，交付路径为 `samples/fixed-channel`。 [范围与验收](../batches/SDK-5-32-fixed-channel.md)
