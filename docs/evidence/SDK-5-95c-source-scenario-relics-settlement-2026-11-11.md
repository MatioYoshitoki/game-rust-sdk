# SDK-5-95 来源场景执行映射验收·遗物、结算回执与绝对终局（第三批）

SDK 0.177.0，机制 105，schema 7，协议 6。DELIVERY-06 / R6.2（I05 场景侧）第三批，接 [第二批](SDK-5-95b-source-scenario-resources-2026-11-11.md)。 [范围](../batches/SDK-5-95-source-scenario-combat.md) · [逐条账本](../checklists/SDK-5-94-source-scenarios.json)

遗物场景经真实死亡掉落生成遗物（`terminal_intent` + `settlement()`），领取走 `OpenRelic`/`ClaimRelic` 公共输入；结算回执场景在协调器层驱动完整 business 回执循环（挂起、committed/not_committed/unknown、幂等重放）；绝对终局经真实 `extraction_lifecycle` 压力窗口。

| 验收路径 | 实测结果 |
|---|---|
| S21 遗物修订与重复 | 精确修订领取清空遗物、物品入包恰一次；同修订重放与另一席位请求均被拒。重放的拒绝码在修订检查之前由就绪/访问防线先行（NotReady/Unauthorized 族），账本登记。 |
| S22 遗物隐私 | `RelicOpened` 内容事件仅私有发往开启者（recipients 恰 [开启者]）；公开快照的遗物摘要不含内容字段。远距/无权开启由交互半径与访问防线拒绝。 |
| S26 结算失败不成功撤离 | `not_committed` 回执后成员不进入撤离终局、世界保留待重试。 |
| S27 核心成功附属失败 | `committed` 核心恰好应用一次、回执重放幂等；附属事实未决不影响已达成的终局（unknown 侧重试节奏属协调器 I/O 策略，与 S42 查询语义同构）。 |
| S28 绝对终局穿保护与无敌 | 真实 Background 保护 + `damage_immunity` 状态绑定的成员在绝对终局与普通成员一同死亡（压力窗口在单发 `DeadlineDamage` 兜底之前完成清场——死亡终局即 oracle，与源 deathTerminalCount 2 一致）。 |

带 `MATCH_SDK_TEST_DATA_DIR` 的 World 层 15 项与协调器层 24 项测试全部通过；合成 122、Node/H5 246、格式、Clippy、数据目录全量 174 过且失败集与基线一致。**场景执行累计 29/48**。剩余：S14/S20（第三单位/包围几何）、S30/S31（加持驱动）、S32～S38（具名模式）、S40～S43（发布与账本细节）、S44/S47（参数化与回放）；S16/S17 回链 M07；分歧累计六处待裁定。
