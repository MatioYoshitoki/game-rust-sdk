# 成长计算与结算

SDK 0.155.0 提供机制 105、配置 schema 7、协议 6 的可选 `settlement_progression`。宿主提供资产标识、初始余额、阈值、保留率、仓库容量和结果目的地。配置使用既有 `asset_contract`、`result_contract` 和 `pve`，并将 `cultivation_coefficient` 设为 0。旧配置仍可省略这些成长扩展字段。

| 字段 | 规则 |
|---|---|
| `age_asset_id` | 独立的已用时间资产。资产余额与 `entry_cost_asset_id` 使用同一整数单位。宿主在 `assets_loaded.facts.assets` 提供该资产。入场同一笔事务按 `params.entry_years` 扣剩余、加已用并扣战备。剩余额刚好用尽可入场，余额不足或已用溢出拒绝。 |
| `storage_caps` | 至多 17 个已声明的堆叠物品资产及其仓库数量上限。先取准入后仓库余额，再以空余量接纳局内保留数量。可设为 0。仓库已有余额高于上限时，本次实存为 0。 |
| `instance_capacity` | 可选实例仓库容量，可设为 0。宿主提供 `assets_loaded.facts.instance_count`。保留实例按实例 ID 排序，先填空位，其余列为溢出。 |
| `warehouse_slots` | SDK 0.156.0 起支持共享格。`capacity` 为总格数，`stack_sizes` 为仓库单格堆叠数量，覆盖本局主物品及全部已声明堆叠物品，排除实例。单格规格范围为 1～10000，可与局内堆叠规格不同。 |
| `conversions` | 至多 8 条规则。每条 `id` 唯一，`when` 接受 `at_most` 或 `at_least` 及 `asset_id`、`quantity`。每步最多 8 行变更。每个资产可 `set` 为 `quantity`、按 `scale.bps` 保留 0～10000 基点并向下取整，或按 `add_capped` 增加 `quantity` 至 `cap`。既有余额高于递增上限时保留原余额。未出现的成长资产按 0 处理。 |
| `destinations` | 结果 ID 到目的地区域 ID。宿主提供 `assets_loaded.facts.location` 中的区域和位置修订。命中配置的结果时，Rust 冻结旧区域、旧修订、目的地和下一修订。 |

奖励通道可单独声明 `damage.rate_bps`、`damage.per_hit_cap_points` 和 `damage.match_cap_points`。每次成员直接对原生 NPC 结算的实际扣血乘计分率，夹在本次上限内后累计到成员该通道的本局上限。护盾吸收、超杀差额、周期、召唤、派生及成员互伤均排除。计分率为 1～100000 基点，本局上限为 1～1000000 整数分，单次上限不得超过本局上限。伤害积分以基点累加，和击杀、终局奖励合并后执行一次取整。 [奖励契约](settlement-rewards-v1.md)

SDK 0.165.0 增加 `damage.rounding`。默认 `accumulated` 保留每次计分的基点余数。`per_contact` 将每次合格扣血乘以计分率后向下取整为整数分，再累计到本局上限。两种策略都应用单次上限和本局上限。通道与结果倍率仍在终局按原规则取整。线上的 `points_bps` 继续用基点表达，启用逐次取整的恢复记录必须为 10000 的整数倍。

入场 `admission.clock` 冻结 `remaining`、`elapsed` 两行，各含资产 ID 及前后余额。补偿反转原已提交入场的两行，与既有战备归还同笔提交。终局沿用入场后的基线。

机制 105、协议 6 从 SDK 0.157.0 起可设 `params.entry_years: 0`。费用资产可从可信余额中省略，入场与补偿均省略 `clock`。同时配置 `carried_quantity: 0` 时，战备余额也可省略，入场 `debits` 与补偿 `credits` 为空。仍须校验账户修订、持久化稳定提交凭据，未知回执按原操作查询。按入场量乘算的奖励通道使用因子 0，其余通道按自身规则。

核心结算先以结果策略确定局内保留和损失，再计算实存、奖励及阈值变更。`result_settlement.summary.progression` 包含 `damage`、`storage`、`instance_storage`、`warehouse_slots`、`conversion` 和 `region` 中启用且适用的字段。库存行 `kept` 表示局内保留，仓库 `stored` 和 `overflow` 记录实际存入与容量溢出。`credits` 按实存数量及受余额上限约束的奖励发放量生成。实例持久化范围由 `stored_ids` 指定。

### 共享格

启用共享格时，宿主同时提供 `instance_count` 和 `warehouse_other_slots`。前者是本仓库全部既有实例数量，每件占一格。后者是配置布局之外的既有占用格，不含前者。堆叠物品的占用按同资产余额除以单格规格向上取整。宿主按同资产归并堆叠，并用账户修订保护整个库存基线。参考宿主从显式账户输入读取 `instances` 和 `warehouse_other_slots`。

Rust 先扣除入场战备，再从该占用出发，按物品 ID 的字节顺序填已有堆叠与空格，随后按实例 ID 使用剩余空格。每类单独上限同时生效。旧仓库超过总容量时仍可填已有堆叠，新增格数为零。共享模式的 `summary.progression.warehouse_slots.before/after` 记录入仓前后总占用，包含布局外占用。宿主核对占用并执行冻结的实存数量，事务恢复沿用原分配。

### 位置写入

宿主在同一个事务中校验账户修订、各行前余额、实例数量、启用的仓库占用和原位置修订，执行库存入仓、奖励、转换与位置迁移并保存幂等凭据。回执未知时查询原操作，证明未提交后使用原正文重试。附属事实在核心提交凭据产生后独立追加。

Node SQLite 参考适配器提供可信宿主方法 `TwoMemberDatabase.savePosition(roomId, participantId, {region_id, revision, sequence, position})`。区域 ID 与位置修订必须匹配当前账户位置。每个位置修订使用正整数递增序号。相同序号和相同坐标的重试返回成功，旧序号或不同坐标的同序号返回失败。坐标保存使用独立序号，账户资产修订与位置修订保持原值。

核心事务递增位置修订并清除旧坐标和写入序号，包括目的地与原区域相同的迁移。已成功核心事务的重试返回原凭据，保留新版本已保存的坐标。生产接入应在所有坐标写入口执行相同条件，并将迁移、清理与核心结算放在同一存储事务中。

进程重启沿用 SDK 的财务恢复契约。恢复重放已冻结结算，尚未冻结终局的历史检查点按现有责任规则处理。运行中连接换代沿用同一笔入场。旧 `itemsGained` 的展示与计奖口径已于 2026-11-13 由 D-05 裁定（经手总量 = `kept + dropped` 合并，含战备携带；不引入入局基线差分，见 [D-05 裁定](../decisions/D-05-items-gained-semantics.md)）；G-03 的正式成长向量、具体转世选择与生产位置持久化仍由宿主验收。

### 当前阶段进度

SDK 0.166.0 支持可选的 `threshold_progress`。宿主配置互异的 `stage_asset_id`、`total_asset_id`、`progress_asset_id` 和从零开始严格递增的 `thresholds`。阶段索引从零开始，必须由可信余额显式提供。当前阶段与进度资产独立于入场扣款、奖励通道和阈值转换。

Rust 读取奖励和转换完成后的累计量，以 `max(0, 总量 - 当前阈值)` 更新进度余额。结算携带当前阶段、最终总量、当前阈值、可选下一阈值、金额是否达标以及进度资产的前后值。末档省略下一阈值。即使本次跨过多个阈值，阶段仍保持原索引。达标结果可用于宿主后续显式晋升事务。

宿主以同一账户修订核对阶段索引、累计量和进度前值，并把 Rust 冻结的进度后值与奖励同笔提交。恢复按原入场事实重验计划。

### 余额耗尽后的交接

宿主可通过 `conversion` 将余额耗尽表达为待处理资产变更，与本局奖励、进度和库存同笔提交。宿主在取得核心提交凭据后读取已提交的待处理状态和余额，用原核心操作 ID 去重后续通知。转世选择由宿主验证当前账户修订和耗尽条件，以独立事务提交角色重置和历史封存。

### 堆叠物资流向

SDK 0.168.0 起，多人配置可设置 `settlement_progression.stack_flows: true`。`result_settlement.summary.progression.stack_flows` 按 `item_id` 排序，每个实际拾取过的堆叠物品一行。未拾取时返回空数组。关闭时省略字段。

| 字段 | 含义 |
|---|---|
| `picked_up` | 实际拾取或从遗物取得的累计数量。重复拾取会再计一次。 |
| `consumed` | 成功使用和作为交互费用消耗的数量。 |
| `released` | 本人成功主动丢弃的数量。 |
| `relic_lost` | 本人终局时实际转入死亡遗物的数量。 |

同一物品的 `picked_up = looted + consumed + released + relic_lost`，其中 `looted` 来自 `summary.inventory`，缺行按零处理。`carried`、`kept`、普通终局 `dropped` 继续使用库存行。存在仓库容量限制时，从 `storage` 读取 `stored` 和 `overflow`，其余堆叠按 `kept` 实存。实例物品继续使用实例身份和 `instance_storage`。

流向随原核心结算操作冻结。恢复核对流向与库存行的一致性，并用各席净拾取和消耗总量核对世界供给。宿主可通过已提交的核心操作读取事实，用于统计、展示或独立业务。SDK 的既有发放规则继续使用配置声明的计分来源。

### 末次入场与顺序转换

SDK 0.170.0 起，`partial_entry_charge: true` 要求已配置 `age_asset_id` 和正数标准费用。收费入场要求余额大于零，扣款取余额与标准费用的较小值，并增加等量累计值。补偿精确逆转原预扣。结算的 `entry_charged` 冻结实际入场费，奖励通道可选择读取此值。采用耗尽标记的配置在终局提交标记，未激活的补偿沿用原账本。

转换可用 `result_ids` 过滤终局结果，并用 `and` 增加最多七项同格式余额条件。未列 `result_ids` 时适用于所有结果。`conversion_mode` 默认为 `first`，采用首条匹配规则并沿用 `summary.progression.conversion`。配置 `ordered` 时，每条规则按声明顺序最多运行一次，后续规则读取前一步的余额。`summary.progression.conversion_steps` 按执行顺序列出命中规则，无匹配时为空数组。

规则可用正数 `clock_cost` 按剩余量扣减，实际扣除量同时增加到 `age_asset_id`。两行计入每步八行上限。同一步的条件和变更都读取该步开始时的余额。启用终局扣减时，各规则对这两个资产均使用 `clock_cost`。入场校验扣减可能达到的累计量是否可表示。

宿主在核心事务中按顺序校验每步前值并应用冻结后值。最终阶段进度读取全部转换之后的总量。恢复按原入场事实核对实际费用、步骤顺序及最终进度。

首条匹配模式的 JSON 正文沿用原 `conversion` 对象。Rust 直接构造 `ProgressionSettlement` 时，`conversion` 使用 `Option<Box<GrowthConversion>>`。
