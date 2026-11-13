# SDK-5-95 来源场景执行映射验收·具名模式（第五批）

SDK 0.179.0，机制 105，schema 7，协议 6。DELIVERY-06 / R6.2（I05 场景侧）第五批，接 [第四批](SDK-5-95d-source-scenario-baselines-2026-11-11.md)。 [范围](../batches/SDK-5-95-source-scenario-combat.md) · [逐条账本](../checklists/SDK-5-94-source-scenarios.json)

具名模式经通用机制配置驱动：训练用 `members.training_targets`（两组预设），对抗用 `match_mode` 的 contest 阶段与 `disconnect_forfeit_ms`。S34 跨协调器层（连接管理属协调器）。

| 验收路径 | 实测结果 |
|---|---|
| S32 训练清理 | `ClearOwnedTargets` 恰释放本席目标组（释放目标即刻退出该席权威视图）；另一席的训练组与归属原样保留；清组零成长写入。 |
| S33 论剑超时平局 | 对抗阶段 180s 到期、双席剩余比例相等 → 双 Draw、无死亡遗物（库存保留）。 |
| S34 断线判负 | 判负期限调度（forfeit_at）+ 30s 到点退场（member_changed）+ 60s 内 2 代重连不推翻现状；B 保持参战。⚠️ 分歧登记：内核让 contest 阶段按自身时钟收尾，源域期望判负即出胜负裁决。 |

配置边界事实（账本同步）：训练模式要求 `entry_years=0` 且与 match_mode/战利品等互斥；模式时长须为 step 倍数且不超 terminal；`disconnect_forfeit_ms` 不得短于连接恢复宽限（源域 30s 判负 < 60s 恢复正是可配域）。

带 `MATCH_SDK_TEST_DATA_DIR` 的协调器 33 项 + World 层 17 项测试全部通过；合成 122、Node/H5 246、格式、Clippy、数据目录全量 181 过且失败集与基线一致。**场景执行累计 36/48**，分歧累计**八处**待裁定（新增：断线判负的即时裁决时序）。

剩余 12 个：S14/S20（几何）、S30/S31（加持驱动）、S35～S38（遗迹/灾变/风眼/火劫——机制在、编排较重）、S44（队列参数化）、S47（回放对比）；S16/S17 回链 M07。
