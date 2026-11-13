# 施法阶段移动接入

SDK 0.31.0 的机制 46 在 schema 7、协议 6 下支持技能 `cast_movement`。在主技能或附加技能中加入完整对象即可启用。

```json
{
  "cast_movement": {
    "preparation": { "kind": "locked" },
    "recovery": { "kind": "slow", "speed_bps": 5000 }
  }
}
```

`free` 保持原速度。 `slow` 把原地形速度乘以 `speed_bps / 10000`，基点范围为 1～9999。 `locked` 暂停位移和导航路径推进。非自由策略要求对应阶段时长大于零。省略整个对象时使用自由移动。

宿主继续发送 Move、Navigate、Stop 和既有施法动作。Rust 接收锁定期间的最新意图。准备策略覆盖提交步的移动，随后从实际位置提交。后摇以 `recovery_until_ms` 为截止，时间相等时应用正常速度。取消准备后按最新意图移动。重连清除旧连接的移动意图和未提交准备，保留已提交后摇及费用。客户端从原有快照读取实际位置、准备和后摇时钟。

开发验证数据在 `examples/cast-movement-host`，交付数据在 `samples/cast-movement`。四人配置是准备锁定和后摇半速。八人配置是准备四分之一速和后摇锁定。可用已有生命成本验证台打开这些配置。 [范围与验收](../batches/SDK-5-29-cast-movement.md)
