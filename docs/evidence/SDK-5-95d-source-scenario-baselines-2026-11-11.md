# SDK-5-95 来源场景执行映射验收·发布基线与账本（第四批）

SDK 0.178.0，机制 105，schema 7，协议 6。DELIVERY-06 / R6.2（I05 场景侧）第四批，接 [第三批](SDK-5-95c-source-scenario-relics-settlement-2026-11-11.md)。 [范围](../batches/SDK-5-95-source-scenario-combat.md) · [逐条账本](../checklists/SDK-5-94-source-scenarios.json)

协调器层适配器扩展：逐席位传输回执控制（Sent/Failed，失败一次后重投成功）、帧种类流水（full/delta，`ReplicaV6` 消费序）、业务请求体日志（按 operation id 去重核对重复扣费）。S43 用两个独立 `TwoMembers` 实例。

| 验收路径 | 实测结果 |
|---|---|
| S40 只对发送者推进基线 | B 的投递失败后必须重建 full 基线才能继续收 delta（帧流水含第二个 full）；A 的已确认基线直接推进 delta。周期性全量属内核正常节奏。 |
| S41 旧代回执不推进新代 | 1 代连接的回执 `StaleConnection` 拒绝；重连后的新代从全新 full 起步，基线不受旧代影响。 |
| S42 未知扣费查证 | 查询证明成立后原操作裁决 committed；重试尝试保持挂起或经查询解决——**不重复扣款不变式**成立。分歧登记：内核在 unknown 后先以新 attempt id 重试再等查询，源域查证优先不发新尝试。 |
| S43 并行对局隔离 | m1 在 Preparing 挂起入场期间，m2 完成完整固定步（5.4）；两局互不扰动。 |

带 `MATCH_SDK_TEST_DATA_DIR` 的协调器层 30 项与 World 层 15 项测试全部通过；合成 122、Node/H5 246、格式、Clippy、数据目录全量 178 过且失败集与基线一致。**场景执行累计 33/48**，分歧累计**七处**待裁定（新增：unknown 回执后的重试与查证顺序）。

剩余待做：S14/S20（几何）、S30/S31（加持驱动）、S32～S38（具名模式）、S44/S47（参数化与回放）；S16/S17 回链 M07。
