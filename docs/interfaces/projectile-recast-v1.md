# 方向弹道单次再施放接入

SDK 0.35.0，机制 50，schema 7，协议 6。同 World 1～8 成员 PVE 的普通方向弹道可声明一次限时转向。

```json
{"recast":{"window_ms":2100,"spirit_cost":5}}
```

窗口与 `step_ms` 对齐，介于一个固定步与普通玩家弹道 TTL 之间，工程上限为 30000 ms。初次仍发送 `cast`。初次成功提交后，本人快照的 `self.projectile_recast` 提供 `action_id`、`skill_id`、`projectile_id`、`expires_at_ms` 和 `next_index`。收到该快照后，用原动作 ID 和索引提交新方向。

```json
{"kind":"recast_projectile","action_id":"recast-1","recast_index":1,"direction":[0,1]}
```

成功后，本人 `private` 事件中的 `projectile_recast` 返回 `action_id`、`projectile_id`、`recast_index`、单位 `direction` 和实际 `spirit_cost`。原弹道位置和 ID 保留，后续沿新方向运动。初次生命费用、冷却、攻击和剩余射程、TTL 沿初次提交的结果。资源不足可在窗口内重试。成功、过期、弹道结束、取消或来源终局后，机会从快照中清除。重连可通过全量恢复在途机会的原截止时间。

H5 先点击施法，再选择目标点并点击「再施放 · 转向」。样例在 `examples/projectile-recast-host`，交付目录为 `samples/projectile-recast`。 [批次范围](../batches/SDK-5-33-projectile-recast.md)
