# SDK-5-95 来源场景执行映射验收·资源与撤离（第二批）

SDK 0.176.0，机制 105，schema 7，协议 6。DELIVERY-06 / R6.2（I05 场景侧）第二批的资源/撤离族，接 [第一批](SDK-5-95-source-scenario-combat-2026-11-11.md)。 [范围](../batches/SDK-5-95-source-scenario-combat.md) · [逐条账本](../checklists/SDK-5-94-source-scenarios.json)

World 层适配器扩展：`given` 注入走真实路径（资源经 `resources` 权威容器、库存经拾取/`credit_stack` 双记账、遗物经 `world_loot` 配置），撤离读条用真实 `ExtractStart` 与绝对步推进（时间不回卷），成员死亡经 `terminal_intent` + `settlement()` 权威拆分。

| 验收路径 | 实测结果 |
|---|---|
| S18 竞争拾取 | 同刻双席拾取同一资源：先到者得 2、后到 InvalidTarget、资源消失、B 库存空。 |
| S19 满包不丢 | 单格满 + 不可堆叠：Capacity 拒绝，资源保留原地。 |
| S23 一次性不刷新 | 拾取后推进至终局时界（约 2 万步）：无重生事件、资源不再存在。 |
| S24 开放前拒绝 | 179980（边界内最后固定步）撤离 INVALID_STATE。 |
| S25 受伤中断 | 90ms 读条中途受 1 伤：`extraction_interrupted` 恰 1 次、读条关闭、成员保持 Active、不触发结算。撤离读条状态字段在 `extract_end`（广播视图仅 `extraction_lifecycle.broadcast_channels` 开启时发布）。 |
| S29 死亡拆分 | 真实拾取 5 → 死亡终局：结算行保留 3（kept），遗物恰 1 stack；掉落半份由遗物条目承载（内核记账口径，映射登记）。仓库余额在 World 外由结算回执侧持有。 |

登记映射：
- **S29 记账口径**：`death_relics` 模式下 `ItemSettlement.dropped` 恒 0，掉落数量记入遗物条目（数量守恒由遗物侧承载），源域把 2 记在结算行的 dropped。行为等价、字段口径不同。

带 `MATCH_SDK_TEST_DATA_DIR` 的 12 项场景测试全部通过（本批 +6）；合成套件 122、格式、Clippy、数据目录全量 169 过且失败集与基线一致。场景执行累计 **24/48**。剩余：S14/S20（需第三单位/包围几何）、S21/S22（遗物领取与隐私）、S26～S28（结算回执与绝对终局）为第三批；S16/S17 回链 M07；分歧累计六处待裁定。
