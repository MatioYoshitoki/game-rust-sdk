# D-05 裁定：itemsGained 采用经手总量语义（2026-11-13）

状态：**已裁定**（用户拍板，2026-11-13）。本记录对齐源项目 decisions.md 的 D-03/D-05 条目格式（编号 | 歧义 | 裁定 | 依据）。

| 编号 | 争议 | 裁定 | 依据 / 影响面 |
|---|---|---|---|
| D-05 | `itemsGained` 是经手总量还是净探索所得 | **经手总量语义**：表示本局结算涉及的全部物品，包含战备携带物。保持现行 `itemsKept + itemsDropped` 合并算法。产品文案按"本局结算涉及物品（含战备携带）"表述，不宣称"净获得"。`carriedIn/looted/consumed/kept/dropped` 分字段保留为可选增强，不得用不明字段影响奖励金额；`enterInventory` 维持兼容字段，结算不读取。**未采用**"净探索所得"口径（不引入入局基线差分） | 宿主冻结提交 `356ce494` 的 `combat_region_settlement.rs` 合并终局保留与掉落生成 `itemsGained`；SDK 侧拆分事实见 [results-v3](../interfaces/results-v3.md) 与 [settlement-progression-v1](../interfaces/settlement-progression-v1.md)；结算金额只读 `credits`/`summary` 冻结正文，`itemsGained` 不参与任何金额（B1 最小闭环同一原则） |

## 执行影响

- **SDK 运行时**：无代码变更。SDK 继续输出拆分事实（`carried/looted/consumed/kept/dropped`，见 [results-v3](../interfaces/results-v3.md) 的 `summary.inventory` 行）；`kept + dropped` 合并即 `itemsGained` 展示口径。
- **接口文档**：[results-v3](../interfaces/results-v3.md) 与 [settlement-progression-v1](../interfaces/settlement-progression-v1.md) 已标注本口径。
- **账本/奖励**：结算金额仍以冻结 `credits` 为唯一依据；统计口径变化不影响奖励金额，也不引入入局基线差分。
- **遗留表**：`sdk-requirements-gap.md` 的 D-05 行已更新为已裁定；M13（完整成长/附属事实）其余条目继续开放，与本裁定互不关闭。
