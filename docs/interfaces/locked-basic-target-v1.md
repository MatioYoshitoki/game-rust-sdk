# 普攻锁定敌方单位接入

SDK 0.39.0，机制 54，schema 7，协议 6。多人 PVE 普攻可指定已发布的 NPC。

```json
{"kind":"attack_unit","target":{"id":"npc-1","generation":1}}
```

目标从共享快照的敌方 NPC 实体取得。Rust 在受理、排队释放和提交时重验同一身份、射程和视线。NPC 重置后，客户端使用新快照重新选择代次。超距或目标失效会拒绝动作。

`attack_unit` 与原 `attack` 共用普攻前摇和冷却。开启 `basic_followup` 时，当前冷却可接收下一击，最新有效请求替换本人的待执行槽。本人 `self.basic_followup.target` 给出排队身份。准备阶段的 `pending_action.unit_target` 可以与 `skill_id: null` 同时出现。失效且已不可见的代次会从准备投影省略，实际失败以 `action_rejected` 为准。

成功时本人 `action_committed` 的 `skill:false` 和 `unit_target` 提供实际提交目标。弹道使用提交方向和既有扫掠碰撞规则。H5 可用「普攻目标」选择身份，也可切回自动选择。样例位于 `examples/locked-basic-target-host`，交付路径为 `samples/locked-basic-target`。 [批次范围](../batches/SDK-5-37-locked-basic-target.md)
