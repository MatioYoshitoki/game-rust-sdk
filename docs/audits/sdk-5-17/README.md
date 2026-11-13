# SDK-5-17 原需求逐项映射

2026-11-05；基线0.19.0 / 机制34。本文和JSON由 `scripts/map_sdk_5_17.py` 离线生成。只完成路由审查，原场景/向量执行数均为0，498项没有因映射而签收。

每项JSON保留原位置/摘要、原审计记录、全部叶字段审查去向、子行为的P/M/I、现有能力切片与边界、缺失能力/工作包/门槛及引用闭包。原sourceAudit.not-started是历史原件，不作当前实现状态。P/M/I在子行为上可并存，I04始终开放。

20个已有能力族 + 18个剩余原生能力族是本次排程粒度，不是声称底层只有38个函数或18个开发批次。未知special归needs_spec，后续补规格可以细分族；25工作包及32类缺口总账保持。

历史证据例外：本轮采集基线摘要后，用户明确清理artifacts目录。能力表保留历史报告身份并标cleaned_by_user；源码/契约与原资料可核对，历史1146/22等只保留已报告口径，当前无法复核原日志。不得把清理前摘要还原为原报告或把文档核对算行为通过。

- [F-01～F-14原功能域与工作包映射](functional-domain-mapping.json)
- [完整498项机器映射](content-mapping.json)
- [48场景与122条原断言](scenario-mapping.json) / [50算术向量](numeric-mapping.json)
- [能力族输入/状态/输出/限制](capability-catalog.json) / [规则门槛与影响ID](rule-gates.json)
- [本批范围和验收](../../batches/SDK-5-17-requirements-mapping.md) / [下一批双成员闭环](../../batches/SDK-5-18-two-member-loop.md)
- [路线及25工作包](../../plans/sdk-delivery-roadmap.md) / [三类缺口](../../checklists/sdk-requirements-gap.md) / [本批校验](../../../artifacts/sdk-5-17/verification.json)

|目录|项数|逐项人读索引|
|---|---:|---|
|skill|86|[skill.md](skill.md)|
|cultivation|52|[cultivation.md](cultivation.md)|
|status|86|[status.md](status.md)|
|factor|177|[factor.md](factor.md)|
|weaponMorph|18|[weaponMorph.md](weaponMorph.md)|
|monster|45|[monster.md](monster.md)|
|mapPve|10|[mapPve.md](mapPve.md)|
|ruins|3|[ruins.md](ruins.md)|
|map|21|[map.md](map.md)|

|场景与向量|数量|人读索引|
|---|---:|---|
|场景|48|[scenarios.md](scenarios.md)|
|向量|50|[vectors.md](vectors.md)|

最先补N01/N02，并在有限双人切片提供N03必要的私有全量发布。两名参与者必须在同一World真实交互；现有“两个Runtime互不阻塞”的证据仅支持跨对局隔离。标准8席/各模式容量、delta/观礼、完整PvP和实例遗物仍在后续，M01/M14保持开放。
