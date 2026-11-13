# 蓄力重瞄准接入

SDK 0.33.0，机制 48，schema 7，协议 6。普通和穿透方向弹道可在原有 `charge` 对象中配置 `aim_update`。

```json
{"min_ms":450,"max_ms":2100,"min_damage_bps":4000,"aim_update":{"min_interval_ms":90,"max_turn_deg_per_sec":720}}
```

开始动作仍发送 `start_charge`。本人快照的 `self.pending_action.charge.aim` 提供单位向量 `direction` 和逻辑毫秒 `updated_at_ms`，该时钟从接受蓄力开始计。按该准备动作的 `action_id` 发送更新或手动释放。

```json
{"kind":"update_charge_aim","action_id":"charge-1","direction":[1,1]}
{"kind":"release_charge","action_id":"charge-1","direction":[0,1]}
```

更新间隔至少 50 ms，转向速度上限 720 度每秒。两者从最近有效改向计算。相同方向幂等。过快、超角或状态已退出会被拒绝，客户端可依据后续权威快照继续操作。手动释放可省略最终方向。自动提交沿用最近有效方向。成功提交的本人 `action_committed.charge.direction` 提供实际冻结单位方向。

H5 在开始蓄力后可点选地图方向，再点击「更新蓄力方向」。黄线显示 Rust 已接受的方向。再次点选后手动释放会带上该次方向。样例在 `examples/charge-reaim-host`，交付路径为 `samples/charge-reaim`。 [范围与验收](../batches/SDK-5-31-charge-reaim.md)
