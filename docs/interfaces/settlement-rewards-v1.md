# 多资产结算奖励

SDK 0.145.0 在机制 105、schema 7 和协议 6 支持可选顶层 `settlement_rewards`。宿主按资产 ID 声明至多 8 个奖励通道。配置该字段时 `cultivation_coefficient` 设为 0，由这些通道负责成长收益。资产 ID 使用 1～64 字节且无控制字符，可沿用原奖励资产 ID。入场费用和库存资产使用独立 ID。

| 字段 | 含义 |
|---|---|
| `kill_points` | 怪物模板 ID 到整数积分的映射。模板必须已登记，未登记奖励权重的模板贡献 0 分。单个权重上限为 100 万。 |
| `rate_bps` | 通道的通用倍率，SDK 0.155.0 起范围为 1～5000000。10000 代表 1 倍。 |
| `balance_cap` | 大于 0 的无符号 32 位余额上限，只限制本次发放。 |
| `by_result` | 为当前可达的每个结果 ID 配置 `bonus_points` 和 `multiplier_bps`。附加积分上限为 100 万，倍率范围为 0～100000。 |
| `multiply_by_entry_cost` | 可选，默认关闭。打开后在一次取整前乘入场费用因子。SDK 0.170.0 支持 `entry_cost_basis`，默认 `standard` 取配置标准费，`charged` 取本席实际扣费。 |
| `damage` | 可选，按实际扣血计分，可选择累计后取整或逐次取整。启用时同时声明 `settlement_progression`。见 [成长结算契约](settlement-progression-v1.md)。 |

每席独立按实际击杀模板计数计算基础分，再加本次结果附加分。请求发放量为 `floor((基础分 + 伤害积分) × 通道倍率 × 结果倍率 × 入场因子)`。倍率字段以基点表示，入场因子关闭时为 1。全部倍率用宽整数一次取整，装载时检查可能的最大请求量。实际发放量受余额余量限制。准入余额已经高于上限时，该通道的发放量为零。

`result_settlement.summary.rewards` 以资产 ID 排序。每行包含 `asset_id`、`base_points`、`requested`、`awarded`、`balance_before`、`balance_after`。余额基线来自可信 `assets_loaded` 事实，缺少资产行时视为 0。即使发放量为零，结算摘要仍带该通道。事务 `credits` 只包含正数发放量。

宿主以预期账户修订号一次提交库存、实例与全部 `credits`，并校验奖励行的事务前后余额。金额由 Rust 冻结。失败、未知回执与恢复沿用原操作 ID 和业务正文。Rust 会核对原准入余额、击杀与结果、发放量和完整资产行。

### 可信装备实例归属

SDK 0.166.0 起，奖励通道可设置 `equipped_weapon: true`。需要多人协议 6、`combat_rules.weapon_catalog` 与 `settlement_progression`。击杀、伤害和结果积分按席位累计，终局时用 Rust 已接受的最新可信装备实例确定奖励账户。无可信装备的席位省略该通道。局内临时武器档案可以参与本局计分倍率选择，奖励账户仍归可信实例。

实例奖励的 `asset_id` 为 `@scope/` 加小写 SHA-224 十六进制摘要，输入为通道 ID 的 UTF-8 字节、一个零字节和实例 ID 的 UTF-8 字节。Rust 提供 `protocol::equipped_weapon_reward_asset_id`，Node 参考适配器提供 `equippedWeaponRewardAssetId`。启用该能力的配置将 `@scope/` 保留给实例奖励。宿主在同一账户修订下提供可能装备实例的奖励余额，缺省余额按零处理。

核心结算的 `summary.progression.reward_weapon_instance_id` 冻结所选实例。已冻结的结算沿用原账户、金额与实例。财务恢复同时核对入场装备和可信换装记录。

### 事件计分倍率

SDK 0.167.0 起，通道可选用 `earning_scale`，并启用 `settlement_progression`。`default_asset_id` 指向默认倍率，`by_basic_profile` 可将已注册的普攻档案绑定到其他倍率资产。全部通道共用至多 16 个倍率资产。宿主在可信 `assets_loaded` 中为每个绑定显式提供 0～100000 基点，10000 代表 1 倍。倍率资产只读，使用独立 ID。

每次合格击杀或实际扣血读取当时生效的普攻档案。局内临时档案优先，其次是当前可信装备档案。没有匹配项时使用默认倍率。击杀按次向下取整。伤害先乘事件倍率，再按通道配置应用单次上限、取整策略和本局上限。倍率为零时事件贡献为零，结果附加分仍按原规则计算。终局继续应用通道倍率、结果倍率和可选入场因子。

启用后，`summary.progression.reward_earnings` 携带按资产 ID 排序的 `rates` 和按模板、档案排序的 `kills`。财务恢复核对原准入倍率、档案合法性、击杀总数以及冻结的结算金额。宿主在核心事务中按账户修订核对当前倍率事实。

### 限次奖励调整

SDK 0.169.0 起，多人奖励通道可配置 `adjustments`，需同时启用 `settlement_progression`。每条由 `uses_asset_id` 和 `effect` 组成。宿主通过可信 `assets_loaded` 提供剩余次数和暂存余额，缺省按零处理。各规则的状态资产独立于奖励、物品和其他成长资产。次数、暂存与事件倍率资产合计最多 16 个 ID。

| `effect.kind` | 配置与结算 |
|---|---|
| `scale` | `bps` 缩放当前请求金额。 |
| `add_original` | `bps` 乘本次通道的原始请求，再加到当前金额。 |
| `defer` | 以 `divert_bps` 从当前请求划入 `escrow_asset_id`，受 `cap` 限制。最后一次以 `release_bps` 返还累计暂存并清零。若提前命中 `forfeit_results` 中的结果，清零暂存及剩余次数。 |

原始请求大于零且该规则有剩余次数时，按声明顺序执行并扣一次。`divert_bps` 为 1～10000，其余倍率为 0～100000。每次乘算单独向下取整。没有暂存空间的金额留在当前请求。最后一次返还优先于同局没收。各暂存返还在全部调整后合入当前请求，再使用通道余额上限限制发放。暂存超过上限或次数已为零仍有暂存的入场事实会被拒绝。

有效调整写入对应奖励行的 `adjustment`，包括 `original_requested`、未裁剪的 `deferred_payout` 和按资产 ID 排序的 `changes`。`requested` 与 `awarded` 记录调整后的请求和实际发放。宿主应校验状态行的前值，与余额发放、库存和位置同笔执行。重启使用原入场事实和冻结事务，后续对局从宿主已提交的余额读取新次数及暂存。宿主负责局外激活与同类效果替换。
